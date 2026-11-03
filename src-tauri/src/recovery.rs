//! Restore from a copy (slice 3, PRD §10.4, §12.3): the "Database is corrupt" window.
//!
//! Two paths:
//! - at launch the database does not open (`StoreError::Corrupt`), or the "recovery needed" mark is set and a repeated
//!   synchronous check confirmed the corruption, or the restore of the previous launch is not finished, or the database file is missing but
//!   copies exist: the journal does not open, there is no tray or shortcuts, the window shows the copies; after the restore the app
//!   picks up the database right away;
//! - the background integrity check found corruption in the running database: a mark is set, the main window gets an event,
//!   "Restart" restarts the app, and it then starts by the first path. A running database must not be replaced
//!   from under the writer thread.
//!
//! A database newer than the app (`StoreError::TooNew`, §10.4) is a separate state: the journal does not open, copies are not offered
//! (they come from the same or a newer version), the window asks to update Kinige.

use kinige_domain::{ApiError, ErrorCode, RestoreOutcome, StartupState};
use kinige_store::{CorruptionHook, Now, RestoreReport, Store};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, Manager, Runtime, State};

/// An event for the main window: the integrity check found corruption in the running database. Only `main` listens.
pub const DATABASE_CORRUPT: &str = "database-corrupt";

type ApiResult<T> = Result<T, ApiError>;

/// The restore state: the database file, the "journal not open" flag and the default Markdown copy folder.
pub struct Recovery {
    db_path: PathBuf,
    active: AtomicBool,
    /// A restore is in progress right now (between `begin_restore` and the end): `startup_state` still counts the journal
    /// as not open, "Restart" is forbidden.
    restoring: AtomicBool,
    /// An integrity check or a command of this process (on any thread) found corruption of the database (set by `corruption_hook`). A process flag, not
    /// a marker file: with `Integrity::Unknown` there is no mark and no flag, while a file from a previous launch may remain.
    corruption: Arc<AtomicBool>,
    database_too_new: bool,
    mirror_folder: Option<PathBuf>,
}

impl Recovery {
    pub fn new(db_path: PathBuf, active: bool) -> Recovery {
        Recovery {
            db_path,
            active: AtomicBool::new(active),
            restoring: AtomicBool::new(false),
            corruption: Arc::new(AtomicBool::new(false)),
            database_too_new: false,
            mirror_folder: None,
        }
    }

    /// The "corruption found" flag set by the check hook (the store opens earlier than `Recovery` is created).
    pub fn with_corruption_flag(mut self, flag: Arc<AtomicBool>) -> Recovery {
        self.corruption = flag;
        self
    }

    pub fn corruption_flag(&self) -> Arc<AtomicBool> {
        self.corruption.clone()
    }

    /// The Markdown copy folder that the restore will rename.
    pub fn with_mirror(mut self, folder: Option<PathBuf>) -> Recovery {
        self.mirror_folder = folder;
        self
    }

    /// The database was created by a newer app version: the journal is not open, there is nothing to restore from and no point.
    pub fn database_too_new(db_path: PathBuf) -> Recovery {
        Recovery {
            database_too_new: true,
            ..Recovery::new(db_path, false)
        }
    }

    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::SeqCst)
    }

    /// Claims the restore: of two simultaneous requests (a double click, two windows) exactly one gets `true`.
    pub fn begin_restore(&self) -> bool {
        let began = self
            .active
            .compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok();
        if began {
            // A finding of a previous attempt (the store managed to open, the hook set the flag, and the services failed) does not belong to the new one:
            // the flag is reset BEFORE the database is opened. Almost always only the now-open database sets it afterwards; the exception is
            // the maintenance pass of the failed attempt, if it is still running: in theory it may set the flag after the reset.
            self.corruption.store(false, Ordering::SeqCst);
            self.restoring.store(true, Ordering::SeqCst);
        }
        began
    }

    /// The restore failed: it can be tried again.
    pub fn abort_restore(&self) {
        self.active.store(true, Ordering::SeqCst);
        self.restoring.store(false, Ordering::SeqCst);
    }

    /// The restore is finished: the journal is open.
    pub fn finish_restore(&self) {
        // The corruption flag is NOT reset: the restored database is already open, and its maintenance had time to check integrity
        // (`resume_after_restore` opens the store before this call). If the copy turned out to have damage that `quick_check`
        // does not see, the finding of this check must survive until the window. Findings of earlier attempts were reset in `begin_restore`.
        self.restoring.store(false, Ordering::SeqCst);
    }

    pub fn is_restoring(&self) -> bool {
        self.restoring.load(Ordering::SeqCst)
    }

    /// The "Database is corrupt" window must be shown (the journal is not open or a restore is in progress).
    pub fn needs_recovery(&self) -> bool {
        self.is_active() || self.is_restoring()
    }

    /// The background check found corruption in the running database, and nothing is being restored.
    pub fn corruption_found(&self) -> bool {
        self.corruption.load(Ordering::SeqCst) && !self.needs_recovery() && !self.database_too_new
    }

    pub fn db_path(&self) -> &Path {
        &self.db_path
    }
}

/// A corruption signal (the background integrity check or any command that read a broken page, on any thread): sets
/// the process flag and a mark (the next launch checks the database once more and, if the corruption is confirmed, offers copies) and
/// notifies the window.
pub fn corruption_hook<R: Runtime>(
    app: AppHandle<R>,
    db_path: PathBuf,
    found: Arc<AtomicBool>,
) -> CorruptionHook {
    CorruptionHook(Arc::new(move || {
        log::error!("integrity check found database corruption");
        found.store(true, Ordering::SeqCst);
        if let Err(error) = kinige_store::mark_needs_recovery(&db_path) {
            // Only the kind of I/O error: the OS error text may contain a path.
            log::error!("could not set the recovery mark: {:?}", error.kind());
        }
        let _ = app.emit_to(crate::capture::MAIN_LABEL, DATABASE_CORRUPT, ());
    }))
}

/// What to show at launch: the journal, the "Database is corrupt" window with a list of copies, or the "update the app" message.
#[tauri::command]
pub fn startup_state(recovery: State<'_, Recovery>) -> StartupState {
    let needs_recovery = recovery.needs_recovery();
    StartupState {
        needs_recovery,
        backups: if needs_recovery {
            kinige_store::backups(recovery.db_path())
        } else {
            Vec::new()
        },
        database_too_new: recovery.database_too_new,
        // The journal is open, and a check or a command found database corruption (the page may have missed the event).
        corruption_found: recovery.corruption_found(),
    }
}

/// "Restore from the copy of ...": only when the journal is not open, and only once at a time. The corrupt file
/// is set aside next to it, `device_id` is new; the previous Markdown copy folder is renamed; then the app opens the database and
/// starts everything that was not started because of the corruption (shortcuts, tray, capture window).
#[tauri::command(async)]
pub fn restore_backup<R: Runtime>(
    app: AppHandle<R>,
    recovery: State<'_, Recovery>,
    name: String,
) -> ApiResult<RestoreOutcome> {
    // The store is already open in this process (the journal works or a previous attempt managed to open it): the database under it
    // must not be replaced, even if the "journal not open" flag is set by mistake.
    if app.try_state::<Store>().is_some() || !recovery.begin_restore() {
        return Err(ApiError::from(ErrorCode::Internal));
    }
    let restored = kinige_store::restore_backup(
        recovery.db_path(),
        &name,
        Now::system().ms,
        recovery.mirror_folder.as_deref(),
    )
    .and_then(|report| crate::resume_after_restore(&app).map(|()| report));
    match restored {
        Ok(report) => {
            recovery.finish_restore();
            Ok(outcome_of(report))
        }
        Err(error) => {
            log::error!("restore from a copy failed: {}", error.category());
            recovery.abort_restore();
            Err(ApiError::from(error.code()))
        }
    }
}

/// "Restart" in the window that appeared on the background check event: only if corruption is really found
/// (found by the background check of this process). A request without a finding is rejected and sets no mark; while a
/// restore is in progress, a restart is forbidden (in the middle of copying the database file).
#[tauri::command]
pub fn restart_for_recovery<R: Runtime>(app: AppHandle<R>) -> ApiResult<()> {
    let allowed = app
        .try_state::<Recovery>()
        .is_some_and(|recovery| recovery.corruption_found());
    if !allowed {
        return Err(ApiError::from(ErrorCode::Internal));
    }
    app.restart();
}

/// The restore outcome for the window: where the copy folder from the backup database and the folder named in `settings.json` moved to.
fn outcome_of(report: RestoreReport) -> RestoreOutcome {
    let text = |folder: PathBuf| folder.to_string_lossy().into_owned();
    RestoreOutcome {
        previous_mirror: report.previous_mirror.map(text),
        settings_file_mirror: report.settings_file_mirror.map(text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_outcome_names_the_folder_moved_aside_for_the_settings_file_only_when_there_is_one() {
        let both = outcome_of(RestoreReport {
            previous_mirror: Some(PathBuf::from("/a/Kinige-before-restore-1")),
            settings_file_mirror: Some(PathBuf::from("/b/Kinige-B-before-restore-1")),
        });
        assert_eq!(
            both.previous_mirror.as_deref(),
            Some("/a/Kinige-before-restore-1")
        );
        assert_eq!(
            both.settings_file_mirror.as_deref(),
            Some("/b/Kinige-B-before-restore-1")
        );
        let same = outcome_of(RestoreReport {
            previous_mirror: Some(PathBuf::from("/a/Kinige-before-restore-1")),
            settings_file_mirror: None,
        });
        assert_eq!(same.settings_file_mirror, None);
    }

    #[test]
    #[allow(clippy::disallowed_methods)] // `thread::spawn` is forbidden in app code (clippy.toml); the test needs 8 threads
    fn exactly_one_of_two_simultaneous_restores_begins() {
        let recovery = Arc::new(Recovery::new(PathBuf::from("kinige.db"), true));
        let wins: usize = (0..8)
            .map(|_| {
                let recovery = recovery.clone();
                std::thread::spawn(move || recovery.begin_restore())
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|thread| usize::from(thread.join().unwrap()))
            .sum();
        assert_eq!(wins, 1);
        assert!(!recovery.is_active());
        recovery.abort_restore();
        assert!(
            recovery.begin_restore(),
            "after a failure it can be tried again"
        );
    }

    #[test]
    fn corruption_is_reported_only_for_a_flag_and_never_while_restoring() {
        let open = Recovery::new(PathBuf::from("x.db"), false);
        assert!(!open.corruption_found(), "no flag (including `Unknown`)");
        open.corruption_flag().store(true, Ordering::SeqCst);
        assert!(open.corruption_found());
        assert!(!open.needs_recovery());

        let closed = Recovery::new(PathBuf::from("x.db"), true);
        closed.corruption_flag().store(true, Ordering::SeqCst);
        assert!(
            !closed.corruption_found(),
            "the journal is not open: a different window"
        );
        assert!(closed.begin_restore());
        assert!(
            closed.needs_recovery() && !closed.corruption_found(),
            "while a restore is in progress the window stays, a restart is forbidden"
        );
        // The store is open, maintenance found damage in the copy (the hook) before the restore ended: the finding survives until the window.
        closed.corruption_flag().store(true, Ordering::SeqCst);
        closed.finish_restore();
        assert!(!closed.needs_recovery());
        assert!(
            closed.corruption_found(),
            "a finding between opening the database and `finish_restore` is not lost"
        );
    }

    #[test]
    fn a_finding_of_a_failed_attempt_does_not_leak_into_the_next_one() {
        let recovery = Recovery::new(PathBuf::from("x.db"), true);
        // Attempt 1: the store opened, the hook set the flag, the services failed.
        assert!(recovery.begin_restore());
        recovery.corruption_flag().store(true, Ordering::SeqCst);
        recovery.abort_restore();
        // Attempt 2 with a healthy copy: the flag is reset before the database opens, the hook is silent.
        assert!(recovery.begin_restore());
        recovery.finish_restore();
        assert!(
            !recovery.corruption_found(),
            "the earlier finding must not survive the new attempt"
        );
        // Attempt 3: the hook fired after the start -- the finding is real.
        recovery.abort_restore();
        assert!(recovery.begin_restore());
        recovery.corruption_flag().store(true, Ordering::SeqCst);
        recovery.finish_restore();
        assert!(recovery.corruption_found());
    }

    #[test]
    fn a_journal_that_is_open_or_too_new_never_begins_a_restore() {
        assert!(!Recovery::new(PathBuf::from("x.db"), false).begin_restore());
        let too_new = Recovery::database_too_new(PathBuf::from("x.db"));
        assert!(!too_new.begin_restore() && too_new.database_too_new);
    }

    #[test]
    fn a_leftover_marker_file_alone_never_allows_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("kinige.db");
        let recovery = Recovery::new(db.clone(), false);
        kinige_store::mark_needs_recovery(&db).unwrap();
        assert!(kinige_store::needs_recovery(&db));
        assert!(
            !recovery.corruption_found(),
            "only the process flag allows a restart"
        );
        kinige_store::clear_needs_recovery(&db);
        assert!(!kinige_store::needs_recovery(&db));
    }
}
