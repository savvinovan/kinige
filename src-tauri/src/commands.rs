//! Команды Tauri — тонкий слой над `kinige-store` и `kinige-domain` (ТЗ §3.2). Бизнес-правил здесь нет. Ошибки уходят
//! фронтенду кодами (`ApiError`), тексты — в файлах переводов.

use crate::hotkeys::Hotkeys;
use kinige_domain::link::{LinkError, validate_external_url};
use kinige_domain::{
    ApiError, AppInfo, ChatList, ErrorCode, FeedCursor, Language, MessagePage, MessageView,
    Settings, ShortcutKind, TextSize,
};
use kinige_store::{DEFAULT_PAGE, Now, PageAt, Store, StoreError};
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

/// Чаты сайдбара в порядке показа (закреплённые, чаты, архив) и размер корзины.
#[tauri::command(async)]
pub fn list_chats(store: State<'_, Store>) -> ApiResult<ChatList> {
    store.sidebar().map_err(api)
}

/// Страница ленты чата; `tag` оставляет только записи с этим тегом (щелчок по `#тегу`). Без `after` и `around` — самая
/// новая страница или более старая после `before`; `after` — более новая страница после курсора; `around` — страница
/// вокруг записи (переход из поиска). Указана одна из трёх позиций: `around`, затем `after`, затем `before`.
#[tauri::command(async)]
pub fn list_messages(
    store: State<'_, Store>,
    chat_id: String,
    before: Option<FeedCursor>,
    after: Option<FeedCursor>,
    around: Option<String>,
    limit: Option<usize>,
    tag: Option<String>,
) -> ApiResult<MessagePage> {
    let at = match (&around, &after, &before) {
        (Some(id), _, _) => PageAt::Around(id),
        (None, Some(cursor), _) => PageAt::After(cursor),
        (None, None, Some(cursor)) => PageAt::Before(cursor),
        (None, None, None) => PageAt::Latest,
    };
    store
        .list_messages_page(
            &chat_id,
            at,
            limit.unwrap_or(DEFAULT_PAGE),
            tag.as_deref(),
            &Now::system(),
        )
        .map_err(api)
}

#[tauri::command(async)]
pub fn send_message<R: Runtime>(
    app: AppHandle<R>,
    store: State<'_, Store>,
    chat_id: String,
    body: String,
) -> ApiResult<MessageView> {
    let message = store
        .send_message(&chat_id, &body, &Now::system())
        .map_err(api)?;
    crate::tray::refresh(&app);
    Ok(message)
}

/// Настройки: размер текста, состояние копии и сочетания клавиш с их состоянием «назначено / занято» от системы.
#[tauri::command(async)]
pub fn get_settings(store: State<'_, Store>, hotkeys: State<'_, Hotkeys>) -> ApiResult<Settings> {
    settings_with_shortcuts(&store, &hotkeys)
}

fn settings_with_shortcuts(store: &Store, hotkeys: &Hotkeys) -> ApiResult<Settings> {
    let mut settings = store.get_settings().map_err(api)?;
    hotkeys.overlay(&mut settings);
    Ok(settings)
}

/// Черновик поля ввода чата (§5): переживает перезапуск.
#[tauri::command(async)]
pub fn get_draft(store: State<'_, Store>, chat_id: String) -> ApiResult<String> {
    store.get_chat_draft(&chat_id).map_err(api)
}

#[tauri::command(async)]
pub fn save_draft(store: State<'_, Store>, chat_id: String, body: String) -> ApiResult<()> {
    store
        .set_chat_draft(&chat_id, &body, &Now::system())
        .map_err(api)
}

/// Смена сочетания клавиш: проверка до сохранения, регистрация в системе, сохранение. Отказ — код ошибки, прежнее
/// сочетание продолжает работать. Ответ — настройки целиком.
#[tauri::command(async)]
pub fn set_shortcut(
    store: State<'_, Store>,
    hotkeys: State<'_, Hotkeys>,
    kind: ShortcutKind,
    accelerator: String,
) -> ApiResult<Settings> {
    hotkeys
        .change(&store, kind, &accelerator)
        .map_err(ApiError::from)?;
    settings_with_shortcuts(&store, &hotkeys)
}

/// «Сбросить»: сочетание по умолчанию с теми же проверками.
#[tauri::command(async)]
pub fn reset_shortcut(
    store: State<'_, Store>,
    hotkeys: State<'_, Hotkeys>,
    kind: ShortcutKind,
) -> ApiResult<Settings> {
    hotkeys.reset(&store, kind).map_err(ApiError::from)?;
    settings_with_shortcuts(&store, &hotkeys)
}

/// В «Настройках» началась (`true`) или закончилась запись нового сочетания: на это время хоткеи не открывают окно захвата,
/// иначе нажать уже назначенное сочетание в поле записи было бы нельзя.
#[tauri::command]
pub fn shortcut_recording(hotkeys: State<'_, Hotkeys>, active: bool) {
    hotkeys.set_recording(active);
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
    hotkeys: State<'_, Hotkeys>,
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
    settings_with_shortcuts(&store, &hotkeys)
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
