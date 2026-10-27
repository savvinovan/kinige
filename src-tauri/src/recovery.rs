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
use kinige_store::{CorruptionHook, Now};
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
    database_too_new: bool,
    mirror_folder: Option<PathBuf>,
}

impl Recovery {
    pub fn new(db_path: PathBuf, active: bool) -> Recovery {
        Recovery {
            db_path,
            active: AtomicBool::new(active),
            database_too_new: false,
            mirror_folder: None,
        }
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
        self.active
            .compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    /// Восстановление не удалось: можно пробовать снова.
    pub fn abort_restore(&self) {
        self.active.store(true, Ordering::SeqCst);
    }

    pub fn db_path(&self) -> &Path {
        &self.db_path
    }
}

/// Сигнал фоновой проверки: ставит отметку (следующий запуск проверит базу ещё раз и, если повреждение подтвердится,
/// предложит копии) и сообщает окну.
pub fn corruption_hook<R: Runtime>(app: AppHandle<R>, db_path: PathBuf) -> CorruptionHook {
    CorruptionHook(Arc::new(move || {
        eprintln!("kinige: проверка целостности нашла повреждение базы");
        if let Err(error) = kinige_store::mark_needs_recovery(&db_path) {
            eprintln!("kinige: не удалось поставить отметку восстановления: {error}");
        }
        let _ = app.emit_to(crate::capture::MAIN_LABEL, DATABASE_CORRUPT, ());
    }))
}

/// Что показывать при запуске: журнал, окно «База повреждена» со списком копий или сообщение «обновите приложение».
#[tauri::command]
pub fn startup_state(recovery: State<'_, Recovery>) -> StartupState {
    let needs_recovery = recovery.is_active();
    StartupState {
        needs_recovery,
        backups: if needs_recovery {
            kinige_store::backups(recovery.db_path())
        } else {
            Vec::new()
        },
        database_too_new: recovery.database_too_new,
        // Журнал открыт и отметка стоит: проверка в фоне нашла повреждение работающей базы (событие страница могла пропустить).
        corruption_found: !needs_recovery
            && !recovery.database_too_new
            && kinige_store::restart_for_recovery_allowed(recovery.db_path()),
        mirror_folder: needs_recovery
            .then(|| recovery.mirror_folder.as_ref())
            .flatten()
            .map(|folder| folder.to_string_lossy().into_owned()),
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
    if !recovery.begin_restore() {
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
        Ok(report) => Ok(RestoreOutcome {
            previous_mirror: report
                .previous_mirror
                .map(|folder| folder.to_string_lossy().into_owned()),
        }),
        Err(error) => {
            eprintln!(
                "kinige: восстановление из копии не удалось: {}",
                error.category()
            );
            recovery.abort_restore();
            Err(ApiError::from(error.code()))
        }
    }
}

/// «Перезапустить» в окне, которое появилось по событию фоновой проверки: только если повреждение действительно найдено
/// (отметка стоит). Просьба без отметки отвергается и отметку не ставит.
#[tauri::command]
pub fn restart_for_recovery<R: Runtime>(app: AppHandle<R>) -> ApiResult<()> {
    let allowed = app
        .try_state::<Recovery>()
        .is_some_and(|recovery| kinige_store::restart_for_recovery_allowed(recovery.db_path()));
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
    fn a_journal_that_is_open_or_too_new_never_begins_a_restore() {
        assert!(!Recovery::new(PathBuf::from("x.db"), false).begin_restore());
        let too_new = Recovery::database_too_new(PathBuf::from("x.db"));
        assert!(!too_new.begin_restore() && too_new.database_too_new);
    }

    #[test]
    fn restart_needs_a_mark() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("kinige.db");
        assert!(!kinige_store::restart_for_recovery_allowed(&db));
        kinige_store::mark_needs_recovery(&db).unwrap();
        assert!(kinige_store::restart_for_recovery_allowed(&db));
    }
}
