//! Восстановление из копии (слайс 3, ТЗ §10.4, §12.3): окно «База повреждена».
//!
//! Два пути:
//! - при запуске база не открывается (`StoreError::Corrupt`) или стоит отметка «нужно восстановление»: журнал не
//!   открывается, трея и сочетаний нет, окно показывает копии; после восстановления приложение подхватывает базу тут же;
//! - проверка целостности в фоне нашла повреждение работающей базы: ставится отметка, главное окно получает событие,
//!   «Перезапустить» перезапускает приложение, и оно запускается уже по первому пути. Работающую базу из-под потока-писателя
//!   подменять нельзя.

use kinige_domain::{ApiError, ErrorCode, StartupState};
use kinige_store::{CorruptionHook, Now, StoreError};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, Manager, Runtime, State};

/// Событие главному окну: проверка целостности нашла повреждение работающей базы. Слушает только `main`.
pub const DATABASE_CORRUPT: &str = "database-corrupt";

type ApiResult<T> = Result<T, ApiError>;

/// Состояние восстановления: файл базы и признак «журнал не открыт».
pub struct Recovery {
    db_path: PathBuf,
    active: AtomicBool,
}

impl Recovery {
    pub fn new(db_path: PathBuf, active: bool) -> Recovery {
        Recovery {
            db_path,
            active: AtomicBool::new(active),
        }
    }

    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::SeqCst)
    }

    pub fn db_path(&self) -> &Path {
        &self.db_path
    }
}

/// Сигнал фоновой проверки: ставит отметку (следующий запуск не откроет базу, а предложит копии) и сообщает окну.
pub fn corruption_hook<R: Runtime>(app: AppHandle<R>, db_path: PathBuf) -> CorruptionHook {
    CorruptionHook(Arc::new(move || {
        eprintln!("kinige: проверка целостности нашла повреждение базы");
        if let Err(error) = kinige_store::mark_needs_recovery(&db_path) {
            eprintln!("kinige: не удалось поставить отметку восстановления: {error}");
        }
        let _ = app.emit_to(crate::capture::MAIN_LABEL, DATABASE_CORRUPT, ());
    }))
}

/// Что показывать при запуске: журнал или окно «База повреждена» со списком копий.
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
    }
}

/// «Восстановить из копии от …»: только когда журнал не открыт. Повреждённый файл откладывается рядом, `device_id` новый;
/// затем приложение открывает базу и запускает всё, что при повреждении не запускалось (сочетания, трей, окно захвата).
#[tauri::command(async)]
pub fn restore_backup<R: Runtime>(
    app: AppHandle<R>,
    recovery: State<'_, Recovery>,
    name: String,
) -> ApiResult<()> {
    if !recovery.is_active() {
        return Err(ApiError::from(ErrorCode::Internal));
    }
    kinige_store::restore_backup(recovery.db_path(), &name, Now::system().ms).map_err(|error| {
        eprintln!(
            "kinige: восстановление из копии не удалось: {}",
            error.category()
        );
        ApiError::from(error.code())
    })?;
    crate::resume_after_restore(&app).map_err(|error: StoreError| {
        eprintln!(
            "kinige: база после восстановления не открылась: {}",
            error.category()
        );
        ApiError::from(error.code())
    })?;
    recovery.active.store(false, Ordering::SeqCst);
    Ok(())
}

/// «Перезапустить» в окне, которое появилось по событию фоновой проверки: отметка уже стоит, запуск пойдёт по пути
/// восстановления.
#[tauri::command]
pub fn restart_for_recovery<R: Runtime>(app: AppHandle<R>) {
    if let Some(recovery) = app.try_state::<Recovery>() {
        let _ = kinige_store::mark_needs_recovery(recovery.db_path());
    }
    app.restart();
}
