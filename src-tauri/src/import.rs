//! Импорт из Telegram и поиск (слайс 4): команды главного окна. Бизнес-правил здесь нет (они в `kinige-domain` и
//! `kinige-store`). Окну захвата эти команды не выданы (§12.3).
//!
//! Путь к файлу выбирается системным диалогом, вызванным из Rust, и во фронтенд не приходит. Файл читается целиком (до
//! 1 ГиБ), разбирается и ждёт в состоянии приложения под номером сессии до запуска импорта или закрытия окна импорта.
//! Одновременно идёт один импорт; прогресс уходит событием `import-progress` только окну `main`.

use crate::capture::{MAIN_LABEL, notify_entries_changed};
use kinige_domain::telegram::{TgExport, TgParseError, parse_export};
use kinige_domain::{
    ApiError, ErrorCode, FeedCursor, ImportOpened, ImportProgress, ImportReport, SearchFilters,
    SearchPage,
};
use kinige_store::{Now, Store, StoreError};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use tauri::{AppHandle, Emitter, Runtime, State};
use tauri_plugin_dialog::DialogExt;

type ApiResult<T> = Result<T, ApiError>;

/// Событие главному окну: прогресс импорта (`ImportProgress`). Слушает только `main`.
pub const IMPORT_PROGRESS: &str = "import-progress";

/// Предел размера файла экспорта: 1 ГиБ (спецификация слайса 4, решение 11).
pub const MAX_IMPORT_FILE_BYTES: u64 = 1 << 30;

fn api(error: StoreError) -> ApiError {
    // В журнал процесса — только категория: текст внутренней ошибки может содержать куски данных.
    log::error!("ошибка хранилища: {}", error.category());
    error.code().into()
}

#[derive(Default)]
struct Inner {
    /// Номер следующей сессии.
    next_session: u64,
    /// Разобранный экспорт и его номер.
    session: Option<(u64, Arc<TgExport>)>,
    /// Импорт идёт.
    running: bool,
}

/// Состояние импорта: разобранный экспорт, флаг «идёт» и флаг остановки.
#[derive(Default)]
pub struct ImportState {
    inner: Mutex<Inner>,
    stop: Arc<AtomicBool>,
}

/// Пока жива, импорт считается идущим; при уничтожении снимает флаг.
pub struct RunGuard<'a> {
    state: &'a ImportState,
    pub export: Arc<TgExport>,
}

impl Drop for RunGuard<'_> {
    fn drop(&mut self) {
        self.state.lock().running = false;
    }
}

impl ImportState {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Запоминает разобранный экспорт, прежний забывается. Ответ — номер сессии.
    pub fn open(&self, export: TgExport) -> u64 {
        let mut inner = self.lock();
        inner.next_session += 1;
        let id = inner.next_session;
        inner.session = Some((id, Arc::new(export)));
        id
    }

    /// Начинает импорт сессии `session`: чужая или забытая сессия — `import_session_expired`, уже идущий импорт —
    /// `import_in_progress`.
    pub fn begin(&self, session: u64) -> Result<RunGuard<'_>, ErrorCode> {
        let mut inner = self.lock();
        if inner.running {
            return Err(ErrorCode::ImportInProgress);
        }
        let export = match &inner.session {
            Some((id, export)) if *id == session => export.clone(),
            _ => return Err(ErrorCode::ImportSessionExpired),
        };
        inner.running = true;
        self.stop.store(false, Ordering::SeqCst);
        Ok(RunGuard {
            state: self,
            export,
        })
    }

    /// Окно импорта закрыто: разобранный экспорт забывается (идущий импорт держит свою ссылку и не прерывается).
    pub fn close(&self) {
        self.lock().session = None;
    }

    /// «Остановить»: импорт закончит текущую пачку и остановится.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }

    pub fn is_running(&self) -> bool {
        self.lock().running
    }
}

/// Файл → экспорт. Чтение: не прочиталось — `import_file_unreadable`; больше 1 ГиБ — `import_file_too_large`; не UTF-8,
/// не JSON или не экспорт Telegram — `import_not_telegram`. Разбор — `kinige_domain::telegram`.
pub fn read_export(path: &std::path::Path) -> Result<TgExport, ErrorCode> {
    let size = std::fs::metadata(path)
        .map_err(|_| ErrorCode::ImportFileUnreadable)?
        .len();
    if size > MAX_IMPORT_FILE_BYTES {
        return Err(ErrorCode::ImportFileTooLarge);
    }
    let bytes = std::fs::read(path).map_err(|_| ErrorCode::ImportFileUnreadable)?;
    let text = String::from_utf8(bytes).map_err(|_| ErrorCode::ImportNotTelegram)?;
    parse_export(&text).map_err(|error: TgParseError| {
        // Подробность разбора может содержать кусок файла: в журнал — только вид ошибки.
        let kind = match error {
            TgParseError::NotJson(_) => "not_json",
            TgParseError::NotTelegramExport => "not_telegram",
            TgParseError::Malformed(_) => "malformed",
        };
        log::warn!("файл импорта отвергнут: {kind}");
        ErrorCode::ImportNotTelegram
    })
}

/// «Выбрать result.json…»: системный диалог (вызывается здесь, из Rust; у окна нет JS-разрешений на диалоги), чтение,
/// разбор, предпросмотр. Отмена диалога — `null`, ничего не меняется.
#[tauri::command(async)]
pub fn import_telegram_open<R: Runtime>(
    app: AppHandle<R>,
    store: State<'_, Store>,
    state: State<'_, ImportState>,
) -> ApiResult<Option<ImportOpened>> {
    let picked = app
        .dialog()
        .file()
        .add_filter("JSON", &["json"])
        .blocking_pick_file();
    let Some(picked) = picked else {
        return Ok(None);
    };
    let path = picked
        .into_path()
        .map_err(|_| ApiError::from(ErrorCode::ImportFileUnreadable))?;
    let export = read_export(&path).map_err(ApiError::from)?;
    open_with(&store, &state, export).map(Some)
}

/// Предпросмотр разобранного экспорта и запоминание сессии (общее для команды и тестов).
pub fn open_with(store: &Store, state: &ImportState, export: TgExport) -> ApiResult<ImportOpened> {
    let preview = store.import_preview(&export).map_err(api)?;
    let session = state.open(export);
    Ok(ImportOpened { session, preview })
}

/// «Импортировать»: выбранные чаты сессии. Возвращает итог; прогресс идёт событием `import-progress`.
#[tauri::command(async)]
pub fn import_telegram_run<R: Runtime>(
    app: AppHandle<R>,
    store: State<'_, Store>,
    state: State<'_, ImportState>,
    session: u64,
    chat_ids: Vec<String>,
) -> ApiResult<ImportReport> {
    let guard = state.begin(session).map_err(ApiError::from)?;
    let sink = app.clone();
    let result = store.import_telegram(
        guard.export.clone(),
        chat_ids,
        crate::commands::get_language(),
        &Now::system(),
        move |progress: ImportProgress| {
            let _ = sink.emit_to(MAIN_LABEL, IMPORT_PROGRESS, progress);
        },
        state.stop.clone(),
    );
    drop(guard);
    // Что бы ни случилось, записи из прошлых пачек уже в базе: окно обновляет ленту и счётчики.
    notify_entries_changed(&app);
    result.map_err(api)
}

/// «Остановить»: текущая пачка дописывается, следующая не начинается.
#[tauri::command]
pub fn import_telegram_stop(state: State<'_, ImportState>) {
    state.stop();
}

/// Окно импорта закрыто: разобранный экспорт забывается.
#[tauri::command]
pub fn import_telegram_close(state: State<'_, ImportState>) {
    state.close();
}

/// Поиск по записям: `query` — ввод как есть, правила словоформ и префикса решает Rust.
#[tauri::command(async)]
pub fn search_messages(
    store: State<'_, Store>,
    query: String,
    filters: Option<SearchFilters>,
    before: Option<FeedCursor>,
    limit: Option<usize>,
) -> ApiResult<SearchPage> {
    store
        .search(
            &query,
            &filters.unwrap_or_default(),
            before.as_ref(),
            limit.unwrap_or(DEFAULT_SEARCH_PAGE),
        )
        .map_err(api)
}

/// Записей на странице результатов поиска.
const DEFAULT_SEARCH_PAGE: usize = 50;
