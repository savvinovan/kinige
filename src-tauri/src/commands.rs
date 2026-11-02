//! Команды Tauri — тонкий слой над `kinige-store` и `kinige-domain` (ТЗ §3.2). Бизнес-правил здесь нет. Ошибки уходят
//! фронтенду кодами (`ApiError`), тексты — в файлах переводов.

use crate::hotkeys::Hotkeys;
use kinige_domain::link::{LinkError, validate_external_url};
use kinige_domain::{
    ApiError, AppInfo, ChatDraft, ChatList, ErrorCode, FeedCursor, FirstWeekday, ForestMode,
    ForestTarget, ForestView, ForestYear, Language, MessagePage, MessageView, OnThisDay, Settings,
    ShortcutKind, TextSize, WeekdaySource,
};
use kinige_store::{DEFAULT_PAGE, Now, PageAt, SettingsFile, Store, StoreError};
use tauri::{AppHandle, Manager, Runtime, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

type ApiResult<T> = Result<T, ApiError>;

fn api(error: StoreError) -> ApiError {
    // В журнал процесса — только категория: текст внутренней ошибки (SQLite, serde) может содержать куски данных.
    log::error!("ошибка хранилища: {}", error.category());
    error.code().into()
}

fn code(error: StoreError) -> ErrorCode {
    api(error).code
}

/// Правка настроек через поток файла (ADR-0010): он применяет внешнюю правку, если файл изменился, выполняет правку и
/// записывает файл. Остановка потока — `internal`.
pub(crate) fn edit<T: Send + 'static>(
    file: &SettingsFile,
    change: impl FnOnce(&Store) -> Result<T, ErrorCode> + Send + 'static,
) -> ApiResult<T> {
    file.edit(change)
        .unwrap_or(Err(ErrorCode::Internal))
        .map_err(ApiError::from)
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

/// Отправка; с `reply_to_id` (слайс 5) — ответ на запись этого же чата. Нет такой записи, она в корзине или id неверен —
/// `reply_target_not_found`, ничего не записано.
#[tauri::command(async)]
pub fn send_message<R: Runtime>(
    app: AppHandle<R>,
    store: State<'_, Store>,
    chat_id: String,
    body: String,
    reply_to_id: Option<String>,
) -> ApiResult<MessageView> {
    let message = store
        .send_reply(&chat_id, &body, reply_to_id.as_deref(), &Now::system())
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

/// Черновик поля ввода чата (§5): переживает перезапуск; несёт и запись, на которую пишется ответ (слайс 5).
#[tauri::command(async)]
pub fn get_draft(store: State<'_, Store>, chat_id: String) -> ApiResult<ChatDraft> {
    store.chat_draft(&chat_id, &Now::system()).map_err(api)
}

#[tauri::command(async)]
pub fn save_draft(
    store: State<'_, Store>,
    chat_id: String,
    body: String,
    reply_to_id: Option<String>,
) -> ApiResult<()> {
    store
        .save_chat_draft(&chat_id, &body, reply_to_id.as_deref(), &Now::system())
        .map_err(api)
}

/// Карточка «В этот день» Inbox (слайс 5); `null` — записей этого числа в прошлые годы нет или карточку скрыли на сегодня.
#[tauri::command(async)]
pub fn get_on_this_day(store: State<'_, Store>) -> ApiResult<Option<OnThisDay>> {
    store.get_on_this_day(&Now::system()).map_err(api)
}

/// «Скрыть до завтра»: до конца дня журнала `date` карточка не показывается. Состояние локальное (`app_setting`).
#[tauri::command(async)]
pub fn dismiss_on_this_day(store: State<'_, Store>, date: String) -> ApiResult<()> {
    store.dismiss_on_this_day(&date).map_err(api)
}

/// The table of one year for the forest (slice 6): every day with its cell, season, notes, stage and the start of its
/// first entry. `year` outside 1000..=9999 is `invalid_date`; an unknown `first_weekday` or `weekday_source` is rejected
/// while the arguments are parsed. `weekday_source` (sent once per session) only goes to the debug log: which of the
/// web view's locale APIs gave the first weekday, and what it was. Only the main window has this command (the excerpts
/// are entry text).
#[tauri::command(async)]
pub fn get_forest_year(
    store: State<'_, Store>,
    year: i32,
    first_weekday: FirstWeekday,
    weekday_source: Option<WeekdaySource>,
) -> ApiResult<ForestYear> {
    if let Some(source) = weekday_source {
        log::debug!("forest: first weekday {first_weekday:?} from {source:?}");
    }
    store
        .get_forest_year(year, first_weekday, &Now::system())
        .map_err(api)
}

/// The entry a click on a day opens (slice 6): the open chat if it has entries that day, otherwise the chat with the
/// most, the Inbox on a tie; `null` for an empty day. A bad `date` is `invalid_date`.
#[tauri::command(async)]
pub fn get_forest_day_target(
    store: State<'_, Store>,
    date: String,
    current_chat_id: Option<String>,
) -> ApiResult<Option<ForestTarget>> {
    store
        .forest_day_target(&date, current_chat_id.as_deref())
        .map_err(api)
}

/// Remembers the forest view (hidden, strip, expanded) and mode (board or calendar). Local, like the sidebar state.
#[tauri::command(async)]
pub fn set_forest_view(
    store: State<'_, Store>,
    view: ForestView,
    mode: ForestMode,
) -> ApiResult<()> {
    store.set_forest_view(view, mode).map_err(api)
}

/// Смена сочетания клавиш: проверка до сохранения, регистрация в системе, сохранение. Отказ — код ошибки, прежнее
/// сочетание продолжает работать. Ответ — настройки целиком.
#[tauri::command(async)]
pub fn set_shortcut<R: Runtime>(
    app: AppHandle<R>,
    store: State<'_, Store>,
    hotkeys: State<'_, Hotkeys>,
    file: State<'_, SettingsFile>,
    kind: ShortcutKind,
    accelerator: String,
) -> ApiResult<Settings> {
    // Правка идёт через поток файла: он применит внешнюю правку, если она есть, и запишет файл (ADR-0010).
    edit(&file, move |store| {
        let hotkeys = app.try_state::<Hotkeys>().ok_or(ErrorCode::Internal)?;
        hotkeys.change(store, kind, &accelerator)
    })?;
    settings_with_shortcuts(&store, &hotkeys)
}

/// «Сбросить»: сочетание по умолчанию с теми же проверками.
#[tauri::command(async)]
pub fn reset_shortcut<R: Runtime>(
    app: AppHandle<R>,
    store: State<'_, Store>,
    hotkeys: State<'_, Hotkeys>,
    file: State<'_, SettingsFile>,
    kind: ShortcutKind,
) -> ApiResult<Settings> {
    edit(&file, move |store| {
        let hotkeys = app.try_state::<Hotkeys>().ok_or(ErrorCode::Internal)?;
        hotkeys.reset(store, kind)
    })?;
    settings_with_shortcuts(&store, &hotkeys)
}

/// В «Настройках» началась (`true`) или закончилась запись нового сочетания: на это время хоткеи не открывают окно захвата,
/// иначе нажать уже назначенное сочетание в поле записи было бы нельзя.
#[tauri::command]
pub fn shortcut_recording(hotkeys: State<'_, Hotkeys>, active: bool) {
    hotkeys.set_recording(active);
}

#[tauri::command(async)]
pub fn set_text_size(file: State<'_, SettingsFile>, size: TextSize) -> ApiResult<()> {
    edit(&file, move |store| store.set_text_size(size).map_err(code))
}

/// Системный диалог выбора папки Markdown-копии. Диалог вызывается здесь, из Rust: у окна нет JS-разрешений на диалоги,
/// и путь не приходит из JS (§12.3). Отмена диалога ничего не меняет.
#[tauri::command(async)]
pub fn choose_mirror_folder<R: Runtime>(
    app: AppHandle<R>,
    store: State<'_, Store>,
    hotkeys: State<'_, Hotkeys>,
    file: State<'_, SettingsFile>,
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
        edit(&file, move |store| {
            store.set_mirror_dir(&path).map_err(code)
        })?;
    }
    settings_with_shortcuts(&store, &hotkeys)
}

/// Открывает ссылку из записи в системном браузере. Схема проверяется в `kinige-domain`: только `http`, `https`, `mailto`.
#[tauri::command(async)]
pub fn open_external_url<R: Runtime>(app: AppHandle<R>, url: String) -> ApiResult<()> {
    let checked = validate_external_url(&url).map_err(|error: LinkError| {
        log::warn!("ссылка отвергнута: {error}");
        ApiError::from(ErrorCode::LinkNotAllowed)
    })?;
    app.opener().open_url(checked, None::<&str>).map_err(|_| {
        // Текст ошибки opener не пишем: в нём может быть сам адрес.
        log::warn!("не удалось открыть ссылку в системном браузере");
        ApiError::from(ErrorCode::Internal)
    })
}
