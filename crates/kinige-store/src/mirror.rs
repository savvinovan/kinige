//! The background Markdown copy (PRD §2.1): after an entry is committed, a separate thread rewrites the file of the affected day in full,
//! `<folder>/<chat>/YYYY/YYYY-MM-DD.md` (a temporary file + rename). A day without entries -- the file is deleted.
//! The marks (`mirror_dirty`) live in the database, so they survive a process kill: the remaining ones are handled at startup.
//! The trash does not get into the copy (PRD §16).

use crate::StoreError;
use crate::mirror_day::write_day;
use crate::settings::effective_mirror_dir;
use crate::writer::Writer;
use kinige_domain::MirrorError;
use rusqlite::Connection;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::thread;
use std::time::Duration;

/// The state shared between the store and the background thread.
pub(crate) struct MirrorShared {
    wake: Mutex<bool>,
    signal: Condvar,
    stopped: AtomicBool,
    error: Mutex<Option<MirrorError>>,
    retry: Duration,
    /// Tests only: the next pass panics.
    panic_next: AtomicBool,
    /// Tests only: a step after reading a batch of marks and before the folder check (a test changes the folder in the middle of a batch here).
    batch_hook: Mutex<Option<BatchHook>>,
}

/// A test step inside a copy pass.
pub(crate) type BatchHook = Arc<dyn Fn() + Send + Sync>;

impl MirrorShared {
    pub(crate) fn new(retry: Duration) -> Arc<MirrorShared> {
        Arc::new(MirrorShared {
            wake: Mutex::new(true), // the first pass runs right at startup
            signal: Condvar::new(),
            stopped: AtomicBool::new(false),
            error: Mutex::new(None),
            retry,
            panic_next: AtomicBool::new(false),
            batch_hook: Mutex::new(None),
        })
    }

    pub(crate) fn set_batch_hook(&self, hook: Option<BatchHook>) {
        *self
            .batch_hook
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = hook;
    }

    fn run_batch_hook(&self) {
        let hook = self
            .batch_hook
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        if let Some(hook) = hook {
            hook();
        }
    }

    pub(crate) fn inject_panic(&self) {
        self.panic_next.store(true, Ordering::SeqCst);
    }

    pub(crate) fn wake(&self) {
        *self.wake.lock().unwrap_or_else(PoisonError::into_inner) = true;
        self.signal.notify_all();
    }

    pub(crate) fn stop(&self) {
        self.stopped.store(true, Ordering::SeqCst);
        self.wake();
    }

    pub(crate) fn error(&self) -> Option<MirrorError> {
        *self.error.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn clear_error(&self) {
        self.set_error(None);
    }

    fn set_error(&self, error: Option<MirrorError>) {
        *self.error.lock().unwrap_or_else(PoisonError::into_inner) = error;
    }

    /// Waits for a signal; after an error or with marks left over -- no longer than `retry`.
    fn wait(&self, retry_soon: bool) {
        let mut flag = self.wake.lock().unwrap_or_else(PoisonError::into_inner);
        if !*flag {
            let timeout = if retry_soon {
                self.retry
            } else {
                Duration::from_secs(3600)
            };
            flag = self
                .signal
                .wait_timeout(flag, timeout)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
        *flag = false;
    }
}

pub(crate) fn spawn(
    shared: Arc<MirrorShared>,
    reader: Connection,
    writer: Writer,
    default_dir: Option<PathBuf>,
) -> Result<(), StoreError> {
    thread::Builder::new()
        .name("kinige-mirror".into())
        .spawn(move || {
            loop {
                if shared.stopped.load(Ordering::SeqCst) {
                    return;
                }
                // A panic in a pass must not silently kill the copy thread: it is caught, the Settings status shows a write
                // error, a retry follows after `retry`, the loop goes on.
                let retry_soon = catch_unwind(AssertUnwindSafe(|| {
                    // Hook for the safety-net tests; absent in a release build (`debug_assertions` off).
                    assert!(
                        !(cfg!(debug_assertions)
                            && shared.panic_next.swap(false, Ordering::SeqCst)),
                        "check: panic in the copy pass"
                    );
                    pass(&shared, &reader, &writer, default_dir.as_deref())
                }))
                .unwrap_or_else(|_| {
                    log::error!("copy: panic in the pass, retry later");
                    shared.set_error(Some(MirrorError::WriteFailed));
                    true
                });
                if shared.stopped.load(Ordering::SeqCst) {
                    return;
                }
                shared.wait(retry_soon);
            }
        })
        .map(drop)
        .map_err(|_| StoreError::Internal("could not start the Markdown copy thread".into()))
}

/// One pass over the marks. Returns `true` if a retry is needed (an error or marks left over).
fn pass(
    shared: &MirrorShared,
    reader: &Connection,
    writer: &Writer,
    default_dir: Option<&Path>,
) -> bool {
    let dir = match effective_mirror_dir(reader, default_dir) {
        Ok((Some(dir), _)) => dir,
        // No folder (not chosen and there is no default): the copy is off, the marks wait.
        Ok((None, _)) => {
            shared.set_error(None);
            return false;
        }
        Err(_) => {
            log::warn!("copy: could not read the folder setting, retry later");
            return true;
        }
    };
    // While there is nothing to write, we do not touch the folder: on macOS the first access to Documents triggers an
    // access prompt, and it should appear together with the first entry, not at the launch of an empty app.
    match dirty_batch(reader) {
        Ok(batch) if batch.is_empty() => {
            shared.set_error(None);
            return false;
        }
        Ok(_) => {}
        Err(_) => {
            // Not "nothing to write": reading the marks failed, the marks are intact -- retry on the timer.
            log::warn!("copy: could not read the day marks, retry later");
            return true;
        }
    }
    if !prepare_root(&dir) {
        shared.set_error(Some(MirrorError::FolderUnavailable));
        return true;
    }
    loop {
        if shared.stopped.load(Ordering::SeqCst) {
            return false;
        }
        let Ok(batch) = dirty_batch(reader) else {
            log::warn!("copy: could not read the day marks, retry later");
            return true;
        };
        if batch.is_empty() {
            shared.set_error(None);
            return false;
        }
        shared.run_batch_hook();
        // The folder may have been changed during the pass (the window or `settings.json`): the change marks all days
        // again, and we must neither write the days marked by this pass to the old folder nor clear the marks -- the new
        // folder would stay empty. The pass starts over with the new folder; the marks are intact.
        if !matches!(effective_mirror_dir(reader, default_dir), Ok((Some(now), _)) if now == dir) {
            return true;
        }
        // Chat folders by their current names; the folders where the files lie now are reconciled with them before days are written.
        let folders = crate::chats::folder_map(reader).unwrap_or_default();
        let in_batch: HashSet<&str> = batch.iter().map(|(chat, _, _)| chat.as_str()).collect();
        if !reconcile_folders(reader, writer, &dir, &folders, &in_batch) {
            return true;
        }
        for (chat_id, date, seq) in batch {
            let folder = folders
                .get(&chat_id)
                .cloned()
                .unwrap_or_else(|| chat_id.clone());
            if let Err(error) = write_day(reader, &dir, &chat_id, &folder, &folders, &date) {
                shared.set_error(Some(error));
                return true;
            }
            // The mark is cleared only if it was not updated during the write: a new edit of the same day keeps the mark.
            let cleared = writer.run(move |conn| {
                conn.execute(
                    "DELETE FROM mirror_dirty WHERE chat_id = ?1 AND journal_date = ?2 AND seq = ?3",
                    (chat_id, date, seq),
                )
            });
            if !matches!(cleared, Ok(Ok(_))) {
                return true;
            }
        }
    }
}

fn dirty_batch(reader: &Connection) -> rusqlite::Result<Vec<(String, String, i64)>> {
    let mut statement = reader.prepare(
        "SELECT chat_id, journal_date, seq FROM mirror_dirty ORDER BY seq, chat_id LIMIT 50",
    )?;
    statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .collect()
}

/// The root folder exists (created if needed) and is a directory.
fn prepare_root(dir: &Path) -> bool {
    fs::create_dir_all(dir).is_ok() && dir.is_dir()
}

/// Chat folders in the copy match the folders where their files lie (the `mirror_chat` table): on a rename (or a change
/// of the owner of a plain name) the folder moves as a whole. The move is in two steps through temporary names, so a swap of
/// names between two chats and a case change (on macOS that is one folder) do not depend on the order. A rename error
/// does not stop the copy: the chat days are still written to the new folder. `false` -- the folder info could not be written, a retry is needed.
fn reconcile_folders(
    reader: &Connection,
    writer: &Writer,
    root: &Path,
    folders: &BTreeMap<String, String>,
    in_batch: &HashSet<&str>,
) -> bool {
    let Ok(known) = known_folders(reader) else {
        return false;
    };
    let moves: Vec<(&String, &String, &String)> = folders
        .iter()
        .filter_map(|(chat, wanted)| {
            let previous = known.get(chat)?;
            (previous != wanted).then_some((chat, previous, wanted))
        })
        .collect();
    // Step 1: leave the old places.
    let mut staged: Vec<(&String, &String, PathBuf)> = Vec::new();
    for (chat, previous, wanted) in &moves {
        let from = root.join(previous);
        let hop = root.join(format!(".moving-{chat}"));
        if from.is_dir() && fs::rename(&from, &hop).is_ok() {
            staged.push((previous, wanted, hop));
        }
    }
    // Step 2: arrive at the new places; if a place is taken, the folder goes back to where it was.
    for (previous, wanted, hop) in staged {
        let to = root.join(wanted);
        if to.exists() || fs::rename(&hop, &to).is_err() {
            let _ = fs::rename(&hop, root.join(previous));
        }
    }
    let wanted_now: Vec<(String, String)> = folders
        .iter()
        .filter(|(chat, wanted)| {
            known.get(*chat) != Some(*wanted)
                && (known.contains_key(*chat) || in_batch.contains(chat.as_str()))
        })
        .map(|(chat, wanted)| (chat.clone(), wanted.clone()))
        .collect();
    if wanted_now.is_empty() {
        return true;
    }
    matches!(
        writer.run(move |conn| {
            for (chat, folder) in &wanted_now {
                conn.execute(
                    "INSERT INTO mirror_chat (chat_id, folder) VALUES (?1, ?2) \
                     ON CONFLICT(chat_id) DO UPDATE SET folder = excluded.folder",
                    (chat, folder),
                )?;
            }
            Ok::<(), rusqlite::Error>(())
        }),
        Ok(Ok(()))
    )
}

fn known_folders(reader: &Connection) -> rusqlite::Result<BTreeMap<String, String>> {
    let mut statement = reader.prepare("SELECT chat_id, folder FROM mirror_chat")?;
    statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect()
}
