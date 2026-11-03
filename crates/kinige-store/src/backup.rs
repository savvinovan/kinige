//! Database file backup and integrity check (PRD §2.1, §10.4, §12.3, slice 3).
//!
//! A backup is taken through the SQLite backup API (`rusqlite::backup`) **from a separate read connection**: a plain
//! `fs::copy` under WAL gives an inconsistent file (without `-wal`), while the backup does not block the writer
//! connection. The backup is written to a temporary file, checked (`PRAGMA quick_check`), switched to the `DELETE`
//! journal mode (one file, no `-wal`), flushed to disk and renamed: a reader of the folder sees either a whole backup or
//! nothing. Names and rotation are in `kinige_domain::backup`.

use crate::StoreError;
use kinige_domain::backup::{self, BackupInfo};
use kinige_domain::{BackupError, BackupStatus};
use rusqlite::{Connection, OpenFlags, backup::Backup};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, UNIX_EPOCH};

/// The backups folder next to the database file.
pub fn backups_dir(db_path: &Path) -> PathBuf {
    db_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("backups")
}

/// The result of an integrity check. "Could not check" (database busy, I/O error, a missing volume) is not corruption:
/// a false "database is corrupted" would send the person to restore a healthy database.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Integrity {
    /// `PRAGMA integrity_check` returned `ok`.
    Ok,
    /// The verdict is not `ok`, or SQLite answers "database is corrupted" / "not a database".
    Corrupt,
    /// Only the search table `search_fts` is damaged (all verdict rows are `fts5:` ones about it, or it does not open):
    /// the database is intact, the index is a derived table, fixed by a rebuild, not by restoring from a backup.
    SearchIndex,
    /// The check did not happen for another reason: retry later, set no mark.
    Unknown,
}

/// The verdict of `PRAGMA integrity_check`. In SQLite 3.53 the check also goes into FTS5: damage to the search index
/// arrives as `fts5: …` rows (or as an error if the table does not open) -- this is [`Integrity::SearchIndex`], not `Corrupt`.
pub(crate) fn check_integrity(conn: &Connection) -> Integrity {
    let rows: rusqlite::Result<Vec<String>> =
        conn.prepare("PRAGMA integrity_check")
            .and_then(|mut statement| {
                statement
                    .query_map([], |row| row.get::<_, String>(0))?
                    .collect()
            });
    match rows {
        Ok(rows) if rows.first().is_some_and(|first| first == "ok") => Integrity::Ok,
        Ok(rows) if !rows.is_empty() && rows.iter().all(|row| is_search_index_row(row)) => {
            Integrity::SearchIndex
        }
        Ok(_) => Integrity::Corrupt,
        Err(error) if crate::error::is_search_index_error(&error) => Integrity::SearchIndex,
        Err(error) => match error.sqlite_error_code() {
            Some(rusqlite::ErrorCode::DatabaseCorrupt | rusqlite::ErrorCode::NotADatabase) => {
                Integrity::Corrupt
            }
            _ => Integrity::Unknown,
        },
    }
}

/// `PRAGMA quick_check` is fine: `ok` or only rows about the search table (or it does not open): for a backup and for
/// a restore this is enough -- the index is derived and will be rebuilt.
pub(crate) fn quick_check_fine(conn: &Connection) -> rusqlite::Result<bool> {
    let verdict: rusqlite::Result<Vec<String>> = conn
        .prepare("PRAGMA quick_check")
        .and_then(|mut statement| statement.query_map([], |row| row.get(0))?.collect());
    match verdict {
        Ok(rows) => Ok(rows.first().is_some_and(|first| first == "ok")
            || (!rows.is_empty() && rows.iter().all(|row| is_search_index_row(row)))),
        Err(error) if crate::error::is_search_index_error(&error) => Ok(true),
        Err(error) => Err(error),
    }
}

/// A verdict row about the search table: `fts5: … "search_fts"`.
fn is_search_index_row(row: &str) -> bool {
    row.starts_with("fts5:") && row.contains("\"search_fts\"")
}

/// Integrity check of the database file on a separate connection (at startup with the "restore needed" mark and before a migration).
pub fn verify_database(db_path: &Path) -> Integrity {
    // No file is not "the check failed": opening would create an empty database, and there is nothing to check.
    if !db_path.exists() {
        return Integrity::Corrupt;
    }
    // Read-only and without `CREATE`: the check must neither create the file nor change it (neither `-wal` nor the journal mode).
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let conn = match Connection::open_with_flags(db_path, flags) {
        Ok(conn) => conn,
        Err(error) => {
            return match crate::error::read_err(error) {
                StoreError::Corrupt(_) => Integrity::Corrupt,
                _ => Integrity::Unknown,
            };
        }
    };
    if conn.busy_timeout(Duration::from_secs(5)).is_err() {
        return Integrity::Unknown;
    }
    check_integrity(&conn)
}

/// Takes a backup of `source` into `dir/name`. An existing file with the same name is replaced entirely.
pub(crate) fn snapshot(source: &Connection, dir: &Path, name: &str) -> Result<(), BackupError> {
    fs::create_dir_all(dir).map_err(|_| BackupError::FolderUnavailable)?;
    let temp = dir.join(format!(".{name}.tmp"));
    let _ = fs::remove_file(&temp);
    let result = write_snapshot(source, &temp).and_then(|()| {
        fs::rename(&temp, dir.join(name)).map_err(|_| BackupError::WriteFailed)?;
        // The rename itself is flushed to disk too. On Windows a directory cannot be opened -- the error is ignored.
        if let Ok(directory) = fs::File::open(dir) {
            let _ = directory.sync_all();
        }
        Ok(())
    });
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

fn write_snapshot(source: &Connection, temp: &Path) -> Result<(), BackupError> {
    let failed = |_: rusqlite::Error| BackupError::WriteFailed;
    {
        let mut target = Connection::open(temp).map_err(failed)?;
        copy_all_pages(source, &mut target)?;
        // A backup is one file: the WAL journal mode would stay in its header and require a `-wal` next to it.
        target
            .pragma_update(None, "journal_mode", "DELETE")
            .map_err(failed)?;
        // A backup with a damaged search index is fine: the index is derived and will be rebuilt (on restoring from it or on
        // the first query). Refusing to copy here would leave the person without a backup over a table that can be recreated.
        if !quick_check_fine(&target).map_err(failed)? {
            return Err(BackupError::WriteFailed);
        }
    }
    // Opened for writing, not only for reading: on Windows `FlushFileBuffers` refuses a handle without write access.
    fs::OpenOptions::new()
        .write(true)
        .open(temp)
        .and_then(|file| file.sync_all())
        .map_err(|_| BackupError::WriteFailed)
}

/// All pages in one step. Page-by-page copying (`run_to_completion(256, …)`) under continuous writes to the database
/// would restart from the first page after every change and might never finish; one step holds a consistent snapshot
/// under a read lock (in WAL the writer does not wait meanwhile). On busy (`Busy`/`Locked`) -- a few retries.
fn copy_all_pages(source: &Connection, target: &mut Connection) -> Result<(), BackupError> {
    use rusqlite::backup::StepResult;
    let failed = |_: rusqlite::Error| BackupError::WriteFailed;
    let backup = Backup::new(source, target).map_err(failed)?;
    for _ in 0..50 {
        match backup.step(-1).map_err(failed)? {
            StepResult::Done => return Ok(()),
            _ => std::thread::sleep(Duration::from_millis(100)),
        }
    }
    Err(BackupError::WriteFailed)
}

/// Backups in the folder, newest first (by file modification time). Foreign and temporary files are not shown.
pub fn list_backups(dir: &Path) -> Vec<BackupInfo> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<BackupInfo> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let kind = backup::classify(&name)?;
            let modified = entry.metadata().ok()?.modified().ok()?;
            let created_at = modified.duration_since(UNIX_EPOCH).ok()?.as_millis();
            Some(BackupInfo {
                name,
                kind,
                created_at: i64::try_from(created_at).ok()?,
            })
        })
        .collect();
    found.sort_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then_with(|| b.name.cmp(&a.name))
    });
    found
}

/// Removes backups beyond the rotation (7 daily ones, 3 pre-migration ones); does not touch foreign files.
pub(crate) fn rotate(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let names: Vec<String> = entries
        .flatten()
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect();
    for stale in backup::expired(&names) {
        let _ = fs::remove_file(dir.join(stale));
    }
}

/// A backup before migrating an existing database (PRD §10.4): the name carries the schema version we migrate from. On
/// error the migration does not start.
pub(crate) fn before_migration(
    source: &Connection,
    db_path: &Path,
    from_version: i64,
    utc_stamp: &str,
) -> Result<(), BackupError> {
    let dir = backups_dir(db_path);
    snapshot(
        source,
        &dir,
        &backup::pre_migration_name(from_version, utc_stamp),
    )?;
    rotate(&dir);
    Ok(())
}

impl crate::Store {
    /// A backup before importing from Telegram (slice 4): `backups/pre-import-<UTC>.db`, the last 3 are kept. It is taken
    /// the same way as the daily backup: a separate read connection, a consistent snapshot while the writer is running.
    /// Returns the file name. An error (folder unavailable, write failed) is [`StoreError::ImportBackupFailed`]: the import does not start.
    pub fn backup_before_import(&self, now: &crate::Now) -> Result<String, StoreError> {
        let db_path = &self.inner.db_path;
        let reader =
            crate::open_connection(db_path, true).map_err(|_| StoreError::ImportBackupFailed)?;
        let dir = backups_dir(db_path);
        // The name has seconds: two imports within one second must not overwrite the first backup, so a taken name is
        // shifted a second forward (the stamp stays the order of backups for rotation).
        let name = (0..60)
            .map(|k| backup::pre_import_name(&crate::time::utc_stamp(now.ms + k * 1000)))
            .find(|name| !dir.join(name).exists())
            .ok_or(StoreError::ImportBackupFailed)?;
        snapshot(&reader, &dir, &name).map_err(|_| StoreError::ImportBackupFailed)?;
        rotate(&dir);
        Ok(name)
    }
}

/// The backups state for Settings and the last error (shared by the maintenance thread and the window).
#[derive(Default)]
pub(crate) struct BackupShared {
    error: Mutex<Option<BackupError>>,
    /// Trash auto-delete could not remove an entry: visible in Settings, but does not cancel the check and the backup.
    purge_failed: std::sync::atomic::AtomicBool,
    /// The local date of the last successful integrity check: the check runs once a day, not every hour.
    checked_on: Mutex<Option<String>>,
}

impl BackupShared {
    pub(crate) fn set_error(&self, error: Option<BackupError>) {
        *self.error.lock().unwrap_or_else(PoisonError::into_inner) = error;
    }

    /// The error for the window: a backup or check failure outranks an auto-delete failure.
    pub(crate) fn error(&self) -> Option<BackupError> {
        let main = *self.error.lock().unwrap_or_else(PoisonError::into_inner);
        main.or_else(|| {
            self.purge_failed
                .load(std::sync::atomic::Ordering::SeqCst)
                .then_some(BackupError::PurgeFailed)
        })
    }

    pub(crate) fn set_purge_failed(&self, failed: bool) {
        self.purge_failed
            .store(failed, std::sync::atomic::Ordering::SeqCst);
    }

    pub(crate) fn checked_on(&self) -> Option<String> {
        self.checked_on
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub(crate) fn set_checked_on(&self, date: Option<String>) {
        *self
            .checked_on
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = date;
    }

    pub(crate) fn status(&self, db_path: &Path) -> BackupStatus {
        let dir = backups_dir(db_path);
        let backups = list_backups(&dir);
        BackupStatus {
            folder: Some(dir.to_string_lossy().into_owned()),
            last_at: backups.first().map(|b| b.created_at),
            count: backups.len() as i64,
            error: self.error(),
        }
    }
}

/// The daily backup for this local date already exists: we do not take a second one the same day.
pub(crate) fn has_daily(dir: &Path, local_date: &str) -> bool {
    let name = backup::daily_name(local_date);
    dir.join(name).is_file()
}
