//! Восстановление из копии (слайс 3, ТЗ §10.4, §12.3): окно «База повреждена».
//!
//! Два пути:
//! - при запуске база не открывается (`StoreError::Corrupt`), либо стоит отметка «нужно восстановление» и повторная
//!   синхронная проверка подтвердила повреждение, либо восстановление прошлого запуска не закончено, либо файла базы нет, а
//!   копии есть: журнал не открывается, трея и сочетаний нет, окно показывает копии; после восстановления приложение
//!   подхватывает базу тут же;
//! - проверка целостности в фоне нашла повреждение работающей базы: ставится отметка, главное окно получает событие,
//!   «Перезапустить» перезапускает приложение, и оно запускается уже по первому пути. Работающую базу из-под потока-писателя
//!   подменять нельзя.
//!
//! База новее приложения (`StoreError::TooNew`, §10.4) — отдельное состояние: журнал не открывается, копии не предлагаются
//! (они из той же или более новой версии), окно просит обновить Kinige.

use kinige_domain::{ApiError, ErrorCode, RestoreOutcome, StartupState};
use kinige_store::{CorruptionHook, Now, Store};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, Manager, Runtime, State};

/// Событие главному окну: проверка целостности нашла повреждение работающей базы. Слушает только `main`.
pub const DATABASE_CORRUPT: &str = "database-corrupt";

type ApiResult<T> = Result<T, ApiError>;

/// Состояние восстановления: файл базы, признак «журнал не открыт» и папка Markdown-копии по умолчанию.
pub struct Recovery {
    db_path: PathBuf,
    active: AtomicBool,
    /// Восстановление идёт прямо сейчас (между `begin_restore` и концом): `startup_state` ещё считает журнал
    /// неоткрытым, «Перезапустить» запрещён.
    restoring: AtomicBool,
    /// Проверка целостности или команда этого процесса (с любого потока) нашла повреждение базы (ставит `corruption_hook`). Флаг процесса, а не
    /// файл-отметка: при `Integrity::Unknown` отметки нет и флага нет, а файл от прошлого запуска мог остаться.
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

    /// Флаг «повреждение найдено», который ставит хук проверки (хранилище открывается раньше, чем создаётся `Recovery`).
    pub fn with_corruption_flag(mut self, flag: Arc<AtomicBool>) -> Recovery {
        self.corruption = flag;
        self
    }

    pub fn corruption_flag(&self) -> Arc<AtomicBool> {
        self.corruption.clone()
    }

    /// Папка Markdown-копии, которую восстановление переименует.
    pub fn with_mirror(mut self, folder: Option<PathBuf>) -> Recovery {
        self.mirror_folder = folder;
        self
    }

    /// База создана более новой версией приложения: журнал не открыт, восстанавливать нечем и незачем.
    pub fn database_too_new(db_path: PathBuf) -> Recovery {
        Recovery {
            database_too_new: true,
            ..Recovery::new(db_path, false)
        }
    }

    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::SeqCst)
    }

    /// Занимает восстановление: из двух одновременных просьб (двойной щелчок, два окна) ровно одна получает `true`.
    pub fn begin_restore(&self) -> bool {
        let began = self
            .active
            .compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok();
        if began {
            // Находка прошлой попытки (хранилище успело открыться, хук поставил флаг, а сервисы упали) к новой не относится:
            // флаг сбрасывается ДО открытия базы. Почти всегда дальше его ставит только открытая теперь база; исключение —
            // проход обслуживания упавшей попытки, если он ещё идёт: теоретически он может поставить флаг после сброса.
            self.corruption.store(false, Ordering::SeqCst);
            self.restoring.store(true, Ordering::SeqCst);
        }
        began
    }

    /// Восстановление не удалось: можно пробовать снова.
    pub fn abort_restore(&self) {
        self.active.store(true, Ordering::SeqCst);
        self.restoring.store(false, Ordering::SeqCst);
    }

    /// Восстановление закончено: журнал открыт.
    pub fn finish_restore(&self) {
        // Флаг повреждения НЕ сбрасывается: восстановленная база уже открыта, и её обслуживание успело проверить целостность
        // (`resume_after_restore` открывает хранилище до этого вызова). Если копия оказалась с порчей, которую не видит
        // `quick_check`, находка этой проверки должна дожить до окна. Находки прошлых попыток сброшены в `begin_restore`.
        self.restoring.store(false, Ordering::SeqCst);
    }

    pub fn is_restoring(&self) -> bool {
        self.restoring.load(Ordering::SeqCst)
    }

    /// Окно «База повреждена» нужно показывать (журнал не открыт или восстановление идёт).
    pub fn needs_recovery(&self) -> bool {
        self.is_active() || self.is_restoring()
    }

    /// Фоновая проверка нашла повреждение работающей базы, и ничего не восстанавливается.
    pub fn corruption_found(&self) -> bool {
        self.corruption.load(Ordering::SeqCst) && !self.needs_recovery() && !self.database_too_new
    }

    pub fn db_path(&self) -> &Path {
        &self.db_path
    }
}

/// Сигнал о повреждении (фоновая проверка целостности или любая команда, прочитавшая битую страницу, с любого потока): ставит
/// флаг процесса и отметку (следующий запуск проверит базу ещё раз и, если повреждение подтвердится, предложит копии) и
/// сообщает окну.
pub fn corruption_hook<R: Runtime>(
    app: AppHandle<R>,
    db_path: PathBuf,
    found: Arc<AtomicBool>,
) -> CorruptionHook {
    CorruptionHook(Arc::new(move || {
        log::error!("проверка целостности нашла повреждение базы");
        found.store(true, Ordering::SeqCst);
        if let Err(error) = kinige_store::mark_needs_recovery(&db_path) {
            // Только вид ошибки ввода-вывода: текст ошибки ОС может содержать путь.
            log::error!(
                "не удалось поставить отметку восстановления: {:?}",
                error.kind()
            );
        }
        let _ = app.emit_to(crate::capture::MAIN_LABEL, DATABASE_CORRUPT, ());
    }))
}

/// Что показывать при запуске: журнал, окно «База повреждена» со списком копий или сообщение «обновите приложение».
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
        // Журнал открыт, и проверка или команда нашла повреждение базы (событие страница могла пропустить).
        corruption_found: recovery.corruption_found(),
    }
}

/// «Восстановить из копии от …»: только когда журнал не открыт, и только один раз за раз. Повреждённый файл
/// откладывается рядом, `device_id` новый; прежняя папка Markdown-копии переименовывается; затем приложение открывает базу и
/// запускает всё, что при повреждении не запускалось (сочетания, трей, окно захвата).
#[tauri::command(async)]
pub fn restore_backup<R: Runtime>(
    app: AppHandle<R>,
    recovery: State<'_, Recovery>,
    name: String,
) -> ApiResult<RestoreOutcome> {
    // Хранилище уже открыто в этом процессе (журнал работает или прошлая попытка успела его открыть): базу под ним
    // подменять нельзя, даже если флаг «журнал не открыт» ошибочно стоит.
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
            Ok(RestoreOutcome {
                previous_mirror: report
                    .previous_mirror
                    .map(|folder| folder.to_string_lossy().into_owned()),
            })
        }
        Err(error) => {
            log::error!("восстановление из копии не удалось: {}", error.category());
            recovery.abort_restore();
            Err(ApiError::from(error.code()))
        }
    }
}

/// «Перезапустить» в окне, которое появилось по событию фоновой проверки: только если повреждение действительно найдено
/// (повреждение нашла фоновая проверка этого процесса). Просьба без находки отвергается и отметку не ставит; пока идёт
/// восстановление, перезапуск запрещён (посреди копирования файла базы).
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
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
        assert!(recovery.begin_restore(), "после сбоя можно пробовать снова");
    }

    #[test]
    fn corruption_is_reported_only_for_a_flag_and_never_while_restoring() {
        let open = Recovery::new(PathBuf::from("x.db"), false);
        assert!(
            !open.corruption_found(),
            "флага нет (в том числе `Unknown`)"
        );
        open.corruption_flag().store(true, Ordering::SeqCst);
        assert!(open.corruption_found());
        assert!(!open.needs_recovery());

        let closed = Recovery::new(PathBuf::from("x.db"), true);
        closed.corruption_flag().store(true, Ordering::SeqCst);
        assert!(!closed.corruption_found(), "журнал не открыт: другое окно");
        assert!(closed.begin_restore());
        assert!(
            closed.needs_recovery() && !closed.corruption_found(),
            "пока идёт восстановление окно остаётся, перезапуск запрещён"
        );
        // Хранилище открыто, обслуживание нашло порчу в копии (хук) до конца восстановления: находка доживает до окна.
        closed.corruption_flag().store(true, Ordering::SeqCst);
        closed.finish_restore();
        assert!(!closed.needs_recovery());
        assert!(
            closed.corruption_found(),
            "находка между открытием базы и `finish_restore` не теряется"
        );
    }

    #[test]
    fn a_finding_of_a_failed_attempt_does_not_leak_into_the_next_one() {
        let recovery = Recovery::new(PathBuf::from("x.db"), true);
        // Попытка 1: хранилище открылось, хук поставил флаг, сервисы упали.
        assert!(recovery.begin_restore());
        recovery.corruption_flag().store(true, Ordering::SeqCst);
        recovery.abort_restore();
        // Попытка 2 со здоровой копией: флаг сброшен до открытия базы, хук молчит.
        assert!(recovery.begin_restore());
        recovery.finish_restore();
        assert!(
            !recovery.corruption_found(),
            "прежняя находка не должна пережить новую попытку"
        );
        // Попытка 3: хук сработал уже после начала — находка настоящая.
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
            "перезапуск разрешает только флаг процесса"
        );
        kinige_store::clear_needs_recovery(&db);
        assert!(!kinige_store::needs_recovery(&db));
    }
}
