//! Restore from a backup (slice 3, PRD §10.4, §12.3; ADR-0001 §1). Works with files while the database is not open in
//! this process: the corrupt file is not deleted but put aside next to it as `corrupt-<time>.db`, the backup takes its
//! place, and a new `device_id` is created -- the database is restored from a backup, so it is a new instance (otherwise
//! two instances with the same `device_id` would issue identical HLC stamps).
//!
//! The `needs-recovery` mark is set at the start of the restore and removed at the end: a failure between the two
//! `rename` calls would leave a folder without a database file at the next launch, and the app would silently create an
//! empty database. The Markdown copy folder is renamed to `<name>-before-restore-<stamp>` and all days are marked for
//! rewriting: otherwise the first edit of a day would overwrite the file with entries made after the backup snapshot.

use crate::backup::{backups_dir, list_backups};
use crate::error::{read_err, write_err};
use crate::schema::LATEST_VERSION;
use crate::send::mark_all_dirty;
use crate::settings::effective_mirror_dir;
use crate::settings_file;
use crate::{StoreError, time::utc_stamp};
use kinige_domain::BackupInfo;
use kinige_domain::backup::classify;
use rusqlite::{Connection, OpenFlags, TransactionBehavior};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use uuid::Uuid;

const MARKER: &str = "needs-recovery";

/// Restores in one process run one at a time: two simultaneous calls must not share intermediate files.
static RESTORE_LOCK: Mutex<()> = Mutex::new(());

fn marker_path(db_path: &Path) -> PathBuf {
    db_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(MARKER)
}

/// The integrity check found corruption in the running database: the next launch does not open it but offers backups.
pub fn mark_needs_recovery(db_path: &Path) -> std::io::Result<()> {
    fs::write(marker_path(db_path), b"")
}

/// Whether the "restore needed" mark is set (see [`mark_needs_recovery`]).
pub fn needs_recovery(db_path: &Path) -> bool {
    marker_path(db_path).exists()
}

/// Removes the mark (the database was checked and is intact, or the restore is finished).
pub fn clear_needs_recovery(db_path: &Path) {
    let _ = fs::remove_file(marker_path(db_path));
}

/// The restore started and is not finished: the mark says `restoring`.
const RESTORING: &[u8] = b"restoring";

fn restore_interrupted(db_path: &Path) -> bool {
    fs::read(marker_path(db_path)).is_ok_and(|content| content == RESTORING)
}

/// How to start the launch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupCheck {
    /// Open the database as usual.
    Normal,
    /// Do not open the journal, show the "Database is corrupt" window with backups.
    Recovery,
}

/// The decision at launch before the database is opened:
/// - the "restore not finished" mark -- always the window with backups (the database may have been left without a new
///   `device_id`);
/// - the "restore needed" mark from the background check -- a repeated synchronous check: `ok` -- the mark is removed and
///   the database opens as usual (the previous launch's check could have been wrong or the corruption was temporary);
///   a "corrupt" verdict or no file -- the window with backups; the check did not happen (busy, I/O) -- open as usual,
///   keep the mark;
/// - no mark, no database file, but backups exist -- the window with backups: silently creating an empty database next
///   to the backups is not allowed.
pub fn startup_check(db_path: &Path) -> StartupCheck {
    if restore_interrupted(db_path) {
        return StartupCheck::Recovery;
    }
    if needs_recovery(db_path) {
        return match crate::backup::verify_database(db_path) {
            // Corruption of only the search index is no reason to restore: the mark is removed, the first query or a
            // maintenance pass will heal the index.
            crate::backup::Integrity::Ok | crate::backup::Integrity::SearchIndex => {
                clear_needs_recovery(db_path);
                StartupCheck::Normal
            }
            crate::backup::Integrity::Corrupt => StartupCheck::Recovery,
            crate::backup::Integrity::Unknown => StartupCheck::Normal,
        };
    }
    if database_missing_but_backups_exist(db_path) {
        StartupCheck::Recovery
    } else {
        StartupCheck::Normal
    }
}

/// There is no database file, but there are backups in the folder: this is how an interrupted restore (or a lost file)
/// looks. Silently creating an empty database next to the backups is not allowed -- backups are offered.
pub fn database_missing_but_backups_exist(db_path: &Path) -> bool {
    !db_path.exists() && !list_backups(&backups_dir(db_path)).is_empty()
}

/// Backups that can be restored from, newest first.
pub fn backups(db_path: &Path) -> Vec<BackupInfo> {
    list_backups(&backups_dir(db_path))
}

/// The backup opens, passes `PRAGMA quick_check` and was made with a schema this app understands.
fn usable(path: &Path) -> bool {
    let Ok(conn) = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY) else {
        return false;
    };
    let version: Result<i64, _> = conn.pragma_query_value(None, "user_version", |row| row.get(0));
    // A backup with a corrupt search index is fine: it is derived and will be rebuilt on the first query or write.
    matches!(version, Ok(v) if (1..=LATEST_VERSION).contains(&v))
        && matches!(crate::backup::quick_check_fine(&conn), Ok(true))
}

/// What the restore did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreReport {
    /// Where the previous Markdown copy folder went; `None` means the copy is off or there was no folder.
    pub previous_mirror: Option<PathBuf>,
    /// Where the copy folder named in `settings.json` (ADR-0010) went, if it is a different one; `None` means there was no such folder.
    pub settings_file_mirror: Option<PathBuf>,
}

/// Restores the database from the backup `name` (a file name from [`backups`]). The corrupt database (if the file exists)
/// goes to `corrupt-<now_ms>.db` together with `-wal` and `-shm`; the backup takes its place; a new `device_id` is
/// created. The previous Markdown copy folder (`default_mirror` is the default folder, for when the user did not change
/// it) is renamed to `<name>-before-restore-<stamp>` (the name is unique) FIRST, before any move of the database; all days
/// are marked for rewriting. The "restore needed" mark is removed at the end. A backup that is not found or fails the
/// check -- `BackupNotFound`, a folder that could not be renamed -- `MirrorFolderBusy`; in both cases nothing is touched.
pub fn restore_backup(
    db_path: &Path,
    name: &str,
    now_ms: i64,
    default_mirror: Option<&Path>,
) -> Result<RestoreReport, StoreError> {
    restore_backup_with(
        db_path,
        name,
        now_ms,
        default_mirror,
        &|| Ok(()),
        &|| Ok(()),
    )
}

/// The same with check steps: `before_place` runs after copying and before the first `rename` of the database,
/// `after_first_rename` -- right after the first `rename` (tests simulate a copy, disk write or rename failure in them).
/// Not part of the app API.
#[doc(hidden)]
pub fn restore_backup_with(
    db_path: &Path,
    name: &str,
    now_ms: i64,
    default_mirror: Option<&Path>,
    before_place: &dyn Fn() -> Result<(), StoreError>,
    after_first_rename: &dyn Fn() -> Result<(), StoreError>,
) -> Result<RestoreReport, StoreError> {
    if classify(name).is_none() {
        return Err(StoreError::BackupNotFound);
    }
    let _guard = RESTORE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let source = backups_dir(db_path).join(name);
    if !source.is_file() || !usable(&source) {
        return Err(StoreError::BackupNotFound);
    }
    let io = |error: std::io::Error| StoreError::Write(error.to_string());
    let dir = db_path.parent().unwrap_or_else(|| Path::new("."));

    // 1. The Markdown copy folder is moved aside FIRST, while nothing else is touched: if it cannot be renamed (busy, no
    // permissions) -- `MirrorFolderBusy`, everything stays in place, the person closes the folder and retries. The path is
    // taken from the backup itself: its setting becomes the setting of the restored database.
    let mirror = mirror_folder_in(&source, default_mirror);
    // A retry after a failed restore (the `restoring` mark): the folder already moved in the previous attempt -- the
    // window must learn about `...-before-restore-...`, so the latest such folder next to it is taken.
    let earlier = if restore_interrupted(db_path) {
        mirror.as_deref().and_then(latest_before_restore)
    } else {
        None
    };
    // The folder from `settings.json` (ADR-0010) is also moved aside if it is a different one: the file will apply on top
    // of the restored database, set this folder as the copy and mark all days for rewriting, and the copy writer deletes
    // day files in the folder that are not in the database, that is, days that appeared after the snapshot. The file is
    // only read, with the same safety checks.
    let from_file = db_path
        .parent()
        .and_then(|data_dir| settings_file::mirror_folder_from_file(data_dir, default_mirror));
    let mut moved: Vec<(PathBuf, PathBuf)> = Vec::new();
    let undo_move = |moved: &[(PathBuf, PathBuf)]| {
        for (from, to) in moved.iter().rev() {
            let _ = fs::rename(to, from);
        }
    };
    for folder in mirror.iter().chain(from_file.iter()) {
        if !folder.is_dir() || moved.iter().any(|(from, _)| from == folder) {
            continue;
        }
        match move_aside(folder, now_ms) {
            Ok(target) => moved.push((folder.clone(), target)),
            Err(error) => {
                undo_move(&moved);
                return Err(error);
            }
        }
    }

    // 2. From this moment the database is in an undefined state: crashing is allowed, silently creating an empty database
    // is not. The previous mark (for example, "restore needed" from the background check) is remembered: if anything fails
    // before the first `rename`, the database is intact and the mark returns as it was.
    let prior_marker = fs::read(marker_path(db_path)).ok();
    let restore_marker = || match &prior_marker {
        Some(content) => {
            let _ = fs::write(marker_path(db_path), content);
        }
        None => clear_needs_recovery(db_path),
    };
    if let Err(error) = fs::write(marker_path(db_path), RESTORING) {
        restore_marker();
        undo_move(&moved);
        return Err(io(error));
    }

    // 3. The backup is first copied whole next to the database (the name is unique), and only then takes its place.
    let staged = dir.join(format!(".restore-{}.tmp", Uuid::new_v4()));
    let mut base_touched = false;
    let placed = (|| -> Result<(), StoreError> {
        // A fresh writable file, not `fs::copy`: `fs::copy` carries the permission bits (the read-only attribute) of the
        // source over, so a read-only backup would give a read-only database, and on Windows `FlushFileBuffers` refuses
        // a handle without write access.
        let mut copy = fs::File::create(&staged).map_err(io)?;
        let mut original = fs::File::open(&source).map_err(io)?;
        std::io::copy(&mut original, &mut copy).map_err(io)?;
        copy.sync_all().map_err(io)?;
        drop(copy);
        before_place()?;
        let corrupt = |suffix: &str| dir.join(format!("corrupt-{now_ms}.db{suffix}"));
        for (from, to) in [
            (db_path.to_path_buf(), corrupt("")),
            (sidecar(db_path, "-wal"), corrupt("-wal")),
            (sidecar(db_path, "-shm"), corrupt("-shm")),
        ] {
            if from.exists() {
                fs::rename(&from, &to).map_err(io)?;
                base_touched = true;
                after_first_rename()?;
            }
        }
        fs::rename(&staged, db_path).map_err(io)?;
        if let Ok(directory) = fs::File::open(dir) {
            let _ = directory.sync_all();
        }
        Ok(())
    })();
    if let Err(error) = placed {
        let _ = fs::remove_file(&staged);
        // A failure before the first `rename` (copying, flush to disk): the database and the folder are in place, everything
        // returns to how it was. After the first `rename` the `restoring` mark stays: at launch the window offers backups.
        if !base_touched {
            restore_marker();
            undo_move(&moved);
        }
        return Err(error);
    }

    let mut conn = Connection::open(db_path).map_err(write_err)?;
    // The new `device_id` is in one transaction with the "all days for rewriting" mark.
    {
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(write_err)?;
        tx.execute(
            "INSERT INTO sync_state(key, value) VALUES ('device_id', ?1) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [Uuid::new_v4().to_string()],
        )
        .map_err(write_err)?;
        mark_all_dirty(&tx).map_err(write_err)?;
        // Agent access is not carried over from the backup (ADR-0010): the result is exactly as in the settings file, and if
        // the file is unsafe or not parsed -- off. The backup has no hash of the file: the file will apply as an external edit.
        settings_file::reset_access_after_restore(&tx).map_err(write_err)?;
        tx.commit().map_err(write_err)?;
    }
    // Check: after the replacement the database can be read.
    conn.query_row("SELECT COUNT(*) FROM chat", [], |row| row.get::<_, i64>(0))
        .map_err(read_err)?;
    drop(conn);
    clear_needs_recovery(db_path);
    let target_of = |folder: &Option<PathBuf>| {
        let folder = folder.as_ref()?;
        moved
            .iter()
            .find(|(from, _)| from == folder)
            .map(|(_, target)| target.clone())
    };
    let previous_mirror = target_of(&mirror).or(earlier);
    let settings_file_mirror = if from_file == mirror {
        None
    } else {
        target_of(&from_file)
    };
    Ok(RestoreReport {
        previous_mirror,
        settings_file_mirror,
    })
}

/// The latest folder `<name>-before-restore-...` next to `folder` (names with a UTC stamp sort by time, the suffix `-2`
/// comes after the same moment). The `folder` itself may be absent: the previous attempt already renamed it.
fn latest_before_restore(folder: &Path) -> Option<PathBuf> {
    let name = folder.file_name()?.to_string_lossy().into_owned();
    let prefix = format!("{name}-before-restore-");
    let parent = folder.parent().unwrap_or_else(|| Path::new("."));
    fs::read_dir(parent)
        .ok()?
        .flatten()
        .filter(|entry| {
            entry.path().is_dir() && entry.file_name().to_string_lossy().starts_with(&prefix)
        })
        .map(|entry| entry.path())
        .max()
}

/// The Markdown copy folder per the backup's own setting (or the default, if the user did not change it); `None` means
/// the copy is off or the setting cannot be read (then nothing is moved).
fn mirror_folder_in(backup: &Path, default_mirror: Option<&Path>) -> Option<PathBuf> {
    let conn = Connection::open_with_flags(backup, OpenFlags::SQLITE_OPEN_READ_ONLY).ok()?;
    effective_mirror_dir(&conn, default_mirror).ok()?.0
}

/// The copy folder becomes `<name>-before-restore-<stamp>` next to it; if that name is taken (two restores in the same
/// second) -- `...-2`, `...-3`. If it could not be renamed -- [`StoreError::MirrorFolderBusy`].
fn move_aside(folder: &Path, now_ms: i64) -> Result<PathBuf, StoreError> {
    let name = folder
        .file_name()
        .ok_or(StoreError::MirrorFolderBusy)?
        .to_string_lossy()
        .into_owned();
    let stamp = utc_stamp(now_ms);
    let target = (1..=999)
        .map(|n| {
            let suffix = if n == 1 {
                String::new()
            } else {
                format!("-{n}")
            };
            folder.with_file_name(format!("{name}-before-restore-{stamp}{suffix}"))
        })
        .find(|candidate| !candidate.exists())
        .ok_or(StoreError::MirrorFolderBusy)?;
    fs::rename(folder, &target).map_err(|_| StoreError::MirrorFolderBusy)?;
    Ok(target)
}

fn sidecar(db_path: &Path, suffix: &str) -> PathBuf {
    let mut name = db_path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}
