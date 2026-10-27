//! Обслуживание в фоне (слайс 3): раз в сутки и при старте — автоудаление корзины (30 дней), проверка целостности и копия
//! дня. Идёт после показа окна, чтобы не ломать холодный старт (< 1,5 с); работает на отдельных соединениях чтения.
//!
//! Копия дня снимается **только после успешной проверки**: иначе испорченные копии вытеснили бы хорошие (7 последних).
//! Проверка нашла повреждение — копия не снимается, ошибка видна в «Настройках», вызывающему слою приходит сигнал
//! (`StoreOptions::on_corruption`): он предложит восстановление из копии.

use crate::backup::{backups_dir, has_daily, integrity_ok, rotate, snapshot};
use crate::{Inner, Now, Store, StoreError, open_connection};
use kinige_domain::BackupError;
use kinige_domain::backup::daily_name;
use kinige_domain::journal::journal_date;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, Weak};
use std::thread;
use std::time::Duration;

/// Как часто поток просыпается проверить, не наступили ли новые сутки.
const TICK: Duration = Duration::from_secs(3600);

/// Что сделал проход обслуживания.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaintenanceReport {
    /// Сколько записей удалено из корзины навсегда.
    pub purged: usize,
    /// `PRAGMA integrity_check` дал `ok`.
    pub integrity_ok: bool,
    /// Снята копия дня.
    pub backup_made: bool,
}

pub(crate) struct MaintenanceShared {
    wake: Mutex<bool>,
    signal: Condvar,
    stopped: AtomicBool,
    /// Сигнал о повреждении уже подан: повторять его каждый час незачем.
    reported: AtomicBool,
}

impl MaintenanceShared {
    pub(crate) fn new() -> Arc<MaintenanceShared> {
        Arc::new(MaintenanceShared {
            wake: Mutex::new(true), // первый проход — сразу при старте
            signal: Condvar::new(),
            stopped: AtomicBool::new(false),
            reported: AtomicBool::new(false),
        })
    }

    pub(crate) fn stop(&self) {
        self.stopped.store(true, Ordering::SeqCst);
        if let Ok(mut flag) = self.wake.lock() {
            *flag = true;
            self.signal.notify_all();
        }
    }

    fn wait(&self) {
        let Ok(mut flag) = self.wake.lock() else {
            return;
        };
        if !*flag {
            flag = match self.signal.wait_timeout(flag, TICK) {
                Ok((flag, _)) => flag,
                Err(_) => return,
            };
        }
        *flag = false;
    }
}

pub(crate) fn spawn(shared: Arc<MaintenanceShared>, store: Weak<Inner>) {
    thread::Builder::new()
        .name("kinige-maintenance".into())
        .spawn(move || {
            loop {
                if shared.stopped.load(Ordering::SeqCst) {
                    return;
                }
                // Хранилище берётся только на время прохода: поток не должен удерживать его живым.
                let Some(inner) = store.upgrade() else {
                    return;
                };
                let store = Store { inner };
                if let Err(error) = store.run_maintenance(&Now::system()) {
                    eprintln!("kinige: обслуживание: {}", error.category());
                }
                drop(store);
                if shared.stopped.load(Ordering::SeqCst) {
                    return;
                }
                shared.wait();
            }
        })
        .expect("не удалось запустить поток обслуживания");
}

impl Store {
    /// Один проход обслуживания (поток вызывает его при старте и раз в сутки; тесты — с заданным «сейчас»).
    pub fn run_maintenance(&self, now: &Now) -> Result<MaintenanceReport, StoreError> {
        let purged = self.purge_expired(now)?;
        let db_path = &self.inner.db_path;
        let reader = open_connection(db_path, true)?;
        let mut report = MaintenanceReport {
            purged,
            integrity_ok: true,
            backup_made: false,
        };
        if !integrity_ok(&reader) {
            report.integrity_ok = false;
            self.inner
                .backup
                .set_error(Some(BackupError::IntegrityFailed));
            if !self.inner.maintenance.reported.swap(true, Ordering::SeqCst) {
                if let Some(hook) = &self.inner.on_corruption {
                    (hook.0)();
                }
            }
            return Ok(report);
        }
        let dir = backups_dir(db_path);
        let today = journal_date(now.ms, now.utc_offset_secs, 0);
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
