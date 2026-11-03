//! Background maintenance (slice 3): once a day and at startup -- auto-delete of the trash (30 days), the integrity check
//! and the daily backup, and (slice 7) the attachment sweep. In the app the first pass waits for the main window's page to
//! load (`StoreOptions::defer_maintenance`, [`Store::begin_maintenance`]), so none of it is on the cold start path (< 1.5 s); it
//! works on separate read connections.
//!
//! The daily backup is taken **only after a successful check**: otherwise corrupted backups would push out good ones (the last 7).
//! If the check finds corruption, no backup is taken, the error is visible in Settings, and the calling layer gets a signal
//! (`StoreOptions::on_corruption`): it will offer a restore from a backup.

use crate::backup::{Integrity, backups_dir, check_integrity, has_daily, rotate, snapshot};
use crate::{Inner, Now, Store, StoreError, open_connection};
use kinige_domain::BackupError;
use kinige_domain::backup::daily_name;
use kinige_domain::journal::journal_date;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError, Weak};
use std::thread;
use std::time::Duration;

/// How often the thread wakes up to check whether a new day has started.
const TICK: Duration = Duration::from_secs(3600);

/// With a deferred start (`StoreOptions::defer_maintenance`) the first pass waits for [`Store::begin_maintenance`] (the main window
/// has loaded its page); if the signal never comes, it starts after this long anyway.
const STARTUP_GATE: Duration = Duration::from_secs(20);

/// What a maintenance pass did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaintenanceReport {
    /// How many entries were deleted from the trash for good.
    pub purged: usize,
    /// `PRAGMA integrity_check` returned `ok`.
    pub integrity_ok: bool,
    /// The daily backup was taken.
    pub backup_made: bool,
}

pub(crate) struct MaintenanceShared {
    wake: Mutex<bool>,
    signal: Condvar,
    stopped: AtomicBool,
    /// The corruption signal was already sent: no need to repeat it every hour.
    reported: AtomicBool,
    /// Tests only: the next pass panics.
    panic_next: AtomicBool,
    /// The first pass is still waiting for the signal that the window has loaded.
    gated: AtomicBool,
}

impl MaintenanceShared {
    /// `deferred`: the first pass waits for [`MaintenanceShared::begin`] (or [`STARTUP_GATE`]) instead of running at once.
    pub(crate) fn new(deferred: bool) -> Arc<MaintenanceShared> {
        Arc::new(MaintenanceShared {
            wake: Mutex::new(!deferred),
            signal: Condvar::new(),
            stopped: AtomicBool::new(false),
            reported: AtomicBool::new(false),
            panic_next: AtomicBool::new(false),
            gated: AtomicBool::new(deferred),
        })
    }

    /// The window has loaded: let a deferred first pass start. Only the first call does anything.
    pub(crate) fn begin(&self) {
        if self.gated.swap(false, Ordering::SeqCst) {
            self.wake_now();
        }
    }

    /// Waits at the gate of a deferred start, for the signal or [`STARTUP_GATE`].
    fn wait_at_gate(&self) {
        if !self.gated.load(Ordering::SeqCst) {
            return;
        }
        let flag = self.wake.lock().unwrap_or_else(PoisonError::into_inner);
        let mut flag = self
            .signal
            .wait_timeout_while(flag, STARTUP_GATE, |woken| {
                !*woken && !self.stopped.load(Ordering::SeqCst)
            })
            .unwrap_or_else(PoisonError::into_inner)
            .0;
        *flag = false;
        drop(flag);
        self.gated.store(false, Ordering::SeqCst);
    }

    pub(crate) fn inject_panic(&self) {
        self.panic_next.store(true, Ordering::SeqCst);
    }

    /// Wake the thread now (test): the next pass runs without waiting an hour.
    pub(crate) fn wake_now(&self) {
        *self.wake.lock().unwrap_or_else(PoisonError::into_inner) = true;
        self.signal.notify_all();
    }

    pub(crate) fn stop(&self) {
        self.stopped.store(true, Ordering::SeqCst);
        *self.wake.lock().unwrap_or_else(PoisonError::into_inner) = true;
        self.signal.notify_all();
    }

    fn wait(&self) {
        let mut flag = self.wake.lock().unwrap_or_else(PoisonError::into_inner);
        if !*flag {
            flag = self
                .signal
                .wait_timeout(flag, TICK)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
        *flag = false;
    }
}

pub(crate) fn spawn(shared: Arc<MaintenanceShared>, store: Weak<Inner>) -> Result<(), StoreError> {
    thread::Builder::new()
        .name("kinige-maintenance".into())
        .spawn(move || {
            shared.wait_at_gate();
            loop {
                if shared.stopped.load(Ordering::SeqCst) {
                    return;
                }
                // The store is held only for the duration of a pass: the thread must not keep it alive.
                let Some(inner) = store.upgrade() else {
                    return;
                };
                let store = Store { inner };
                // A panic in a pass must not silently kill the maintenance thread (integrity check, trash, backups, log
                // rotation): it is caught, the backup status becomes "could not write", the next pass runs in an hour.
                let outcome = catch_unwind(AssertUnwindSafe(|| {
                    // Hook for the safety-net tests; absent in a release build (`debug_assertions` off).
                    assert!(
                        !(cfg!(debug_assertions)
                            && shared.panic_next.swap(false, Ordering::SeqCst)),
                        "check: panic in the maintenance pass"
                    );
                    store.run_maintenance(&Now::system())
                }));
                match outcome {
                    Ok(Ok(_)) => {}
                    Ok(Err(error)) => log::warn!("maintenance: {}", error.category()),
                    Err(_) => {
                        log::error!("maintenance: panic in the pass, retry later");
                        store.inner.backup.set_error(Some(BackupError::WriteFailed));
                    }
                }
                drop(store);
                if shared.stopped.load(Ordering::SeqCst) {
                    return;
                }
                shared.wait();
            }
        })
        .map(drop)
        .map_err(|_| StoreError::Internal("could not start the maintenance thread".into()))
}

impl Store {
    /// Corruption found (by the integrity check or by a command that read a broken page): the signal is sent once per
    /// launch, after it nothing more is written to the database (`run_maintenance`).
    pub(crate) fn report_corruption(&self) {
        // The line in Settings matches the "Database is corrupted" window, also when the finding came from a command.
        self.inner
            .backup
            .set_error(Some(BackupError::IntegrityFailed));
        let first_time = !self.inner.maintenance.reported.swap(true, Ordering::SeqCst);
        if let (true, Some(hook)) = (first_time, &self.inner.on_corruption) {
            (hook.0)();
        }
    }

    /// One maintenance pass (the thread calls it at startup and once a day; tests call it with a given "now").
    pub fn run_maintenance(&self, now: &Now) -> Result<MaintenanceReport, StoreError> {
        let db_path = &self.inner.db_path;
        let mut report = MaintenanceReport {
            purged: 0,
            integrity_ok: true,
            backup_made: false,
        };
        // Corruption was already found: nothing more is written to such a database (no auto-delete, no backup) until the
        // person restores it from a backup.
        if self.inner.maintenance.reported.load(Ordering::SeqCst) {
            report.integrity_ok = false;
            return Ok(report);
        }
        let reader = open_connection(db_path, true)?;
        let dir = backups_dir(db_path);
        let today = journal_date(now.ms, now.utc_offset_secs, 0);
        // The integrity check runs once per local day (not every hour) if it has already passed.
        if self.inner.backup.checked_on().as_deref() != Some(today.as_str()) {
            let seen = self.search_generation();
            let mut verdict = check_integrity(&reader);
            if verdict == Integrity::SearchIndex {
                // The database is intact, only the search index is damaged: rebuild, then check again (real corruption
                // will not be hidden, and a rebuild in a corrupted database reports `Corrupt` by itself).
                self.heal_search_index(seen)?;
                verdict = check_integrity(&open_connection(db_path, true)?);
            }
            match verdict {
                Integrity::Ok => self.inner.backup.set_checked_on(Some(today.clone())),
                Integrity::Corrupt => {
                    report.integrity_ok = false;
                    self.inner
                        .backup
                        .set_error(Some(BackupError::IntegrityFailed));
                    self.report_corruption();
                    return Ok(report);
                }
                // The check did not happen (database busy, I/O error): this is not corruption. We set no mark and
                // take no backup without a verdict; the next pass will retry.
                Integrity::Unknown | Integrity::SearchIndex => {
                    log::warn!("integrity check did not complete, retry later");
                    return Ok(report);
                }
            }
        }
        // Auto-delete runs only after an "intact" verdict (or after a successful check today): we must not write to a
        // database that has not been checked yet. One damaged entry in the trash must not deprive the database of backups
        // forever: the auto-delete error is recorded (visible in Settings) and the pass goes on.
        let (purged, purge_error) = self.purge_expired_report(now);
        if let Some(error) = &purge_error {
            log::warn!("trash auto-delete: {}", error.category());
        }
        self.inner.backup.set_purge_failed(purge_error.is_some());
        report.purged = purged;
        // The agent access log: 90 days and at most 20,000 rows; a rotation error does not cancel the daily backup.
        if let Err(error) = self.rotate_agent_log(now) {
            log::warn!("agent log rotation: {}", error.category());
        }
        // Attachment files: leftovers of crashes, purges that did not finish, files whose row is gone (ADR-0014). A writer job.
        if let Err(error) = self.sweep_attachments(now) {
            log::warn!("attachment sweep: {}", error.category());
        }
        if has_daily(&dir, &today) {
            self.inner.backup.set_error(None);
            return Ok(report);
        }
        match snapshot(&reader, &dir, &daily_name(&today)) {
            Ok(()) => {
                rotate(&dir);
                self.inner.backup.set_error(None);
                report.backup_made = true;
            }
            Err(error) => self.inner.backup.set_error(Some(error)),
        }
        Ok(report)
    }
}
