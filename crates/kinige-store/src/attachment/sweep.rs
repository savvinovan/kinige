//! The attachment sweep (ADR-0014): runs in the background after the first frame, **as a writer job**, so it cannot see a file
//! that a live attach has renamed but not yet written a row for. It touches only what no live attach owns: `.tmp/` and `open/`
//! directories of other processes, files without a row, thumbnails without a row, and the state of rows whose file is gone.

use super::{Dirs, delete_stored_files, is_stored_name, make_dir, stored_id};
use crate::error::write_err;
use crate::{Now, Store, StoreError};
use kinige_domain::history::DAY_MS;
use rusqlite::{Connection, OptionalExtension};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::path::Path;
use std::time::{Duration, UNIX_EPOCH};

/// How long a file without a row waits in `.orphans/` before it is deleted.
const ORPHAN_DAYS: i64 = 30;

/// What a sweep did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SweepReport {
    /// Directories of other processes removed (`.tmp/<process>`, `open/<process>`).
    pub stale_dirs: usize,
    /// Files without a row that were moved to `.orphans/`.
    pub orphaned: usize,
    /// Files without a row deleted at once because their id has a tombstone (a purge whose deletion did not finish).
    pub deleted: usize,
    /// Files brought back from `.orphans/` for a row that wanted them.
    pub restored: usize,
    /// Rows whose file is gone: now `missing`.
    pub missing: usize,
    /// Thumbnails without an attachment, deleted.
    pub thumbs_removed: usize,
    /// Files in `.orphans/` older than 30 days, deleted.
    pub orphans_expired: usize,
}

impl Store {
    /// Cleans up after crashes and restores (see the module doc). Safe to run at any time and concurrently with attaching: both are
    /// writer jobs.
    pub fn sweep_attachments(&self, now: &Now) -> Result<SweepReport, StoreError> {
        let (dirs, faults) = (self.dirs(), self.inner.faults.clone());
        let now_ms = now.ms;
        self.write(move |conn| sweep(conn, &dirs, &faults, now_ms))
    }
}

fn sweep(
    conn: &mut Connection,
    dirs: &Dirs,
    faults: &super::Faults,
    now_ms: i64,
) -> Result<SweepReport, StoreError> {
    let mut report = SweepReport {
        stale_dirs: remove_other_processes(&dirs.tmp_root(), dirs.process())
            + remove_other_processes(&dirs.open_root(), dirs.process()),
        // Expiry comes first: a file moved to `.orphans/` in this pass is never a candidate in the same pass.
        orphans_expired: expire_orphans(&dirs.orphans(), now_ms),
        ..SweepReport::default()
    };
    let locals = load_locals(conn).map_err(write_err)?;
    let drafts = load_drafts(conn).map_err(write_err)?;
    let mut present = sort_files(conn, dirs, faults, &locals, &drafts, now_ms, &mut report)?;
    let tx = conn.transaction().map_err(write_err)?;
    for (id, (file_name, state)) in &locals {
        if present.contains(file_name) {
            if state == "missing" {
                set_state(&tx, id, "local")?;
            }
        } else if restore(dirs, file_name) {
            present.insert(file_name.clone());
            set_state(&tx, id, "local")?;
            report.restored += 1;
        } else if state != "missing" {
            set_state(&tx, id, "missing")?;
            report.missing += 1;
        }
    }
    for (id, file_name) in &drafts {
        if !present.contains(file_name) && !restore(dirs, file_name) {
            // A tile whose file is gone cannot be sent.
            tx.execute(
                "DELETE FROM draft_attachment WHERE attachment_id = ?1",
                [id],
            )
            .map_err(write_err)?;
        }
    }
    tx.commit().map_err(write_err)?;
    report.thumbs_removed = remove_orphan_thumbs(conn, dirs).map_err(write_err)?;
    Ok(report)
}

/// Files of `attachments/` that no row owns: deleted if the id has a tombstone, otherwise moved to `.orphans/`. Returns the
/// names of the files that stay (the ones some row owns).
fn sort_files(
    conn: &Connection,
    dirs: &Dirs,
    faults: &super::Faults,
    locals: &HashMap<String, (String, String)>,
    drafts: &HashMap<String, String>,
    now_ms: i64,
    report: &mut SweepReport,
) -> Result<HashSet<String>, StoreError> {
    let owned: HashSet<&str> = locals
        .values()
        .map(|(name, _)| name.as_str())
        .chain(drafts.values().map(String::as_str))
        .collect();
    let mut present = HashSet::new();
    let Ok(entries) = fs::read_dir(dirs.attachments()) else {
        return Ok(present);
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        // Only `<uuid>.<ext>` files are ours to move; `.tmp`, `.orphans` and anything else stay where they are.
        let (Some(id), true) = (stored_id(&name), entry.path().is_file()) else {
            continue;
        };
        if owned.contains(name.as_str()) {
            present.insert(name);
            continue;
        }
        let purged: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM tombstone WHERE entity = 'attachment' AND id = ?1)",
                [id],
                |row| row.get(0),
            )
            .map_err(write_err)?;
        if purged {
            delete_stored_files(dirs, faults, &[(id.to_string(), Some(name))]);
            report.deleted += 1;
        } else if move_to_orphans(dirs, &name, now_ms) {
            report.orphaned += 1;
        }
    }
    Ok(present)
}

/// Moves a file aside and stamps it with the time of the move: the 30 days count from here, not from when the file was written
/// (a rename keeps the modification time, so a file older than 30 days would otherwise be expired at once).
fn move_to_orphans(dirs: &Dirs, file_name: &str, now_ms: i64) -> bool {
    let Some(from) = dirs.stored(file_name) else {
        return false;
    };
    let to = dirs.orphans().join(file_name);
    if make_dir(&dirs.orphans()).is_err() || fs::rename(from, &to).is_err() {
        return false;
    }
    // If stamping fails the file keeps its old time and may expire early; with the sweep running at startup that is a rare case
    // of a failed metadata write, and the file is still moved aside rather than deleted.
    let moved_at = UNIX_EPOCH + Duration::from_millis(u64::try_from(now_ms).unwrap_or(0));
    let _ = File::options()
        .write(true)
        .open(&to)
        .and_then(|file| file.set_modified(moved_at));
    true
}

/// A file of a row that lies in `.orphans/` goes back (restoring an older backup and then a newer one).
fn restore(dirs: &Dirs, file_name: &str) -> bool {
    let Some(to) = dirs.stored(file_name) else {
        return false;
    };
    let from = dirs.orphans().join(file_name);
    from.is_file() && fs::rename(from, to).is_ok()
}

fn set_state(tx: &rusqlite::Transaction<'_>, id: &str, state: &str) -> Result<(), StoreError> {
    tx.execute(
        "UPDATE attachment_local SET state = ?2 WHERE attachment_id = ?1",
        (id, state),
    )
    .map_err(write_err)?;
    Ok(())
}

/// `id -> (file_name, state)`; a name that is not `<uuid>.<ext>` is left out, so no path is built from it.
fn load_locals(conn: &Connection) -> rusqlite::Result<HashMap<String, (String, String)>> {
    let mut statement =
        conn.prepare("SELECT attachment_id, file_name, state FROM attachment_local")?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            (row.get::<_, String>(1)?, row.get::<_, String>(2)?),
        ))
    })?;
    Ok(rows
        .filter_map(Result::ok)
        .filter(|(_, (name, _))| is_stored_name(name))
        .collect())
}

fn load_drafts(conn: &Connection) -> rusqlite::Result<HashMap<String, String>> {
    let mut statement = conn.prepare("SELECT attachment_id, file_name FROM draft_attachment")?;
    let rows = statement.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    Ok(rows
        .filter_map(Result::ok)
        .filter(|(_, name)| is_stored_name(name))
        .collect())
}

/// Directories of `root` that belong to a process other than ours; they are left from a crash.
fn remove_other_processes(root: &Path, own: &str) -> usize {
    let Ok(entries) = fs::read_dir(root) else {
        return 0;
    };
    entries
        .flatten()
        .filter(|entry| entry.file_name().to_string_lossy() != own)
        .filter(|entry| {
            let path = entry.path();
            if path.is_dir() {
                fs::remove_dir_all(path).is_ok()
            } else {
                fs::remove_file(path).is_ok()
            }
        })
        .count()
}

/// Thumbnails `thumbs/<uuid>.jpg|png` whose attachment has no row.
fn remove_orphan_thumbs(conn: &Connection, dirs: &Dirs) -> rusqlite::Result<usize> {
    let Ok(entries) = fs::read_dir(dirs.thumbs()) else {
        return Ok(0);
    };
    let mut removed = 0;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        // A temporary name left by a crash.
        if name.ends_with(".tmp") {
            if fs::remove_file(entry.path()).is_ok() {
                removed += 1;
            }
            continue;
        }
        let Some((id, ext)) = name.split_once('.') else {
            continue;
        };
        if !kinige_domain::hlc::is_uuid_v4(id) || !matches!(ext, "jpg" | "png") {
            continue;
        }
        // A tile above the input field has a thumbnail too (the same id as the row it becomes on sending).
        let known: Option<i64> = conn
            .query_row(
                "SELECT 1 FROM attachment WHERE id = ?1 \
                 UNION ALL SELECT 1 FROM draft_attachment WHERE attachment_id = ?1",
                [id],
                |row| row.get(0),
            )
            .optional()?;
        if known.is_none() && fs::remove_file(entry.path()).is_ok() {
            removed += 1;
        }
    }
    Ok(removed)
}

/// Files of `.orphans/` older than 30 days are deleted for good.
fn expire_orphans(dir: &Path, now_ms: i64) -> usize {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    let border = u64::try_from(now_ms.saturating_sub(ORPHAN_DAYS * DAY_MS)).unwrap_or(0);
    entries
        .flatten()
        .filter(|entry| {
            let modified = entry
                .metadata()
                .and_then(|meta| meta.modified())
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map(|age| u64::try_from(age.as_millis()).unwrap_or(u64::MAX));
            modified.is_some_and(|ms| ms <= border) && fs::remove_file(entry.path()).is_ok()
        })
        .count()
}
