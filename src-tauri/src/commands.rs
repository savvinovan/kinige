//! Команды Tauri — тонкий слой над `kinige-store` и `kinige-domain` (ТЗ §3.2). Бизнес-правил здесь нет. Ошибки уходят
//! фронтенду кодами (`ApiError`), тексты — в файлах переводов.

use kinige_domain::link::{LinkError, validate_external_url};
use kinige_domain::{
    ApiError, AppInfo, ChatSummary, ErrorCode, FeedCursor, Language, MessagePage, MessageView,
    Settings, TextSize,
};
use kinige_store::{DEFAULT_PAGE, Now, Store, StoreError};
use tauri::{AppHandle, Runtime, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

type ApiResult<T> = Result<T, ApiError>;

fn api(error: StoreError) -> ApiError {
    // В журнал процесса — только категория: текст внутренней ошибки (SQLite, serde) может содержать куски данных.
    eprintln!("kinige: ошибка хранилища: {}", error.category());
    error.code().into()
}

/// Сведения о приложении для `KinigeApi.appInfo()`.
#[tauri::command]
pub fn app_info() -> AppInfo {
    kinige_domain::app_info(env!("CARGO_PKG_VERSION"))
}

/// Язык интерфейса по предпочитаемым языкам системы (ADR-0005).
#[tauri::command]
pub fn get_language() -> Language {
    let preferred: Vec<String> = sys_locale::get_locales().collect();
    kinige_domain::lang::pick_language(&preferred)
}

#[tauri::command(async)]
pub fn list_chats(store: State<'_, Store>) -> ApiResult<Vec<ChatSummary>> {
    store.list_chats().map_err(api)
}

#[tauri::command(async)]
pub fn list_messages(
    store: State<'_, Store>,
    chat_id: String,
    before: Option<FeedCursor>,
    limit: Option<usize>,
) -> ApiResult<MessagePage> {
    store
        .list_messages(
            &chat_id,
            before.as_ref(),
            limit.unwrap_or(DEFAULT_PAGE),
            &Now::system(),
        )
        .map_err(api)
}

#[tauri::command(async)]
pub fn send_message(
    store: State<'_, Store>,
    chat_id: String,
    body: String,
) -> ApiResult<MessageView> {
    store
        .send_message(&chat_id, &body, &Now::system())
        .map_err(api)
}

#[tauri::command(async)]
pub fn get_settings(store: State<'_, Store>) -> ApiResult<Settings> {
    store.get_settings().map_err(api)
}

#[tauri::command(async)]
pub fn set_text_size(store: State<'_, Store>, size: TextSize) -> ApiResult<()> {
    store.set_text_size(size).map_err(api)
}

/// Системный диалог выбора папки Markdown-копии. Диалог вызывается здесь, из Rust: у окна нет JS-разрешений на диалоги,
/// и путь не приходит из JS (§12.3). Отмена диалога ничего не меняет.
#[tauri::command(async)]
pub fn choose_mirror_folder<R: Runtime>(
    app: AppHandle<R>,
    store: State<'_, Store>,
) -> ApiResult<Settings> {
    let mut dialog = app.dialog().file();
    if let Some(current) = store.get_settings().map_err(api)?.mirror.folder {
        let current = std::path::PathBuf::from(current);
        if current.is_dir() {
            dialog = dialog.set_directory(current);
        }
    }
    // Блокирующий вызов допустим: команда выполняется в пуле потоков (`async`), а не на главном потоке.
    if let Some(picked) = dialog.blocking_pick_folder() {
        let path = picked
            .into_path()
            .map_err(|_| ApiError::from(ErrorCode::FolderPickFailed))?;
        if !path.is_dir() {
            return Err(ErrorCode::FolderPickFailed.into());
        }
        store.set_mirror_dir(&path).map_err(api)?;
    }
    store.get_settings().map_err(api)
}

/// Открывает ссылку из записи в системном браузере. Схема проверяется в `kinige-domain`: только `http`, `https`, `mailto`.
#[tauri::command(async)]
pub fn open_external_url<R: Runtime>(app: AppHandle<R>, url: String) -> ApiResult<()> {
    let checked = validate_external_url(&url).map_err(|error: LinkError| {
        eprintln!("kinige: ссылка отвергнута: {error}");
        ApiError::from(ErrorCode::LinkNotAllowed)
    })?;
    app.opener().open_url(checked, None::<&str>).map_err(|_| {
        // Текст ошибки opener не пишем: в нём может быть сам адрес.
        eprintln!("kinige: не удалось открыть ссылку в системном браузере");
        ApiError::from(ErrorCode::Internal)
    })
}
