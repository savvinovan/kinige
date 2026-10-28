//! Обслуживание в фоне (слайс 3): раз в сутки и при старте — автоудаление корзины (30 дней), проверка целостности и копия
//! дня. Идёт после показа окна, чтобы не ломать холодный старт (< 1,5 с); работает на отдельных соединениях чтения.
//!
//! Копия дня снимается **только после успешной проверки**: иначе испорченные копии вытеснили бы хорошие (7 последних).
//! Проверка нашла повреждение — копия не снимается, ошибка видна в «Настройках», вызывающему слою приходит сигнал
//! (`StoreOptions::on_corruption`): он предложит восстановление из копии.

use crate::backup::{Integrity, backups_dir, check_integrity, has_daily, rotate, snapshot};
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
                    log::warn!("обслуживание: {}", error.category());
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
    /// Повреждение найдено (проверкой целостности или командой, прочитавшей битую страницу): сигнал подаётся один раз за
    /// запуск, после него в базу больше ничего не пишется (`run_maintenance`).
    pub(crate) fn report_corruption(&self) {
        // Строка в «Настройках» совпадает с окном «База повреждена», в том числе когда находка пришла от команды.
        self.inner
            .backup
            .set_error(Some(BackupError::IntegrityFailed));
        let first_time = !self.inner.maintenance.reported.swap(true, Ordering::SeqCst);
        if let (true, Some(hook)) = (first_time, &self.inner.on_corruption) {
            (hook.0)();
        }
    }

    /// Один проход обслуживания (поток вызывает его при старте и раз в сутки; тесты — с заданным «сейчас»).
    pub fn run_maintenance(&self, now: &Now) -> Result<MaintenanceReport, StoreError> {
        let db_path = &self.inner.db_path;
        let mut report = MaintenanceReport {
            purged: 0,
            integrity_ok: true,
            backup_made: false,
        };
        // Повреждение уже найдено: в такую базу больше ничего не пишется (ни автоудаление, ни копия), пока человек не
        // восстановит её из копии.
        if self.inner.maintenance.reported.load(Ordering::SeqCst) {
            report.integrity_ok = false;
            return Ok(report);
        }
        let reader = open_connection(db_path, true)?;
        let dir = backups_dir(db_path);
        let today = journal_date(now.ms, now.utc_offset_secs, 0);
        // Проверка целостности — раз в локальные сутки (не каждый час), если она уже прошла успешно.
        if self.inner.backup.checked_on().as_deref() != Some(today.as_str()) {
            let seen = self.search_generation();
            let mut verdict = check_integrity(&reader);
            if verdict == Integrity::SearchIndex {
                // Цела база, повреждён только индекс поиска: перестройка, затем повторная проверка (настоящая порча не
                // скроется, а перестройка в повреждённой базе сама сообщит `Corrupt`).
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
                // Проверка не состоялась (база занята, ошибка ввода-вывода): это не повреждение. Отметку не ставим,
                // копию не снимаем без вердикта, следующий проход повторит.
                Integrity::Unknown | Integrity::SearchIndex => {
                    log::warn!("проверка целостности не состоялась, повтор позже");
                    return Ok(report);
                }
            }
        }
        // Автоудаление идёт только после вердикта «цела» (или после успешной проверки сегодня): писать в базу, которую ещё
        // не проверили, нельзя. Одна повреждённая запись в корзине не должна навсегда лишать базу копии: ошибка
        // автоудаления записывается (видна в «Настройках») и проход идёт дальше.
        let (purged, purge_error) = self.purge_expired_report(now);
        if let Some(error) = &purge_error {
            log::warn!("автоудаление корзины: {}", error.category());
        }
        self.inner.backup.set_purge_failed(purge_error.is_some());
        report.purged = purged;
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
