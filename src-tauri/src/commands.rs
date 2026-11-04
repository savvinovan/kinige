//! Tauri commands -- a thin layer over `kinige-store` and `kinige-domain` (PRD §3.2). There are no business rules here. Errors go
//! to the frontend as codes (`ApiError`), the texts are in the translation files.

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
    // Only the category goes to the process log: the text of an internal error (SQLite, serde) may contain pieces of data.
    log::error!("store error: {}", error.category());
    error.code().into()
}

fn code(error: StoreError) -> ErrorCode {
    api(error).code
}

/// Editing settings through the file thread (ADR-0010): it applies an external edit if the file changed, performs the edit and
/// writes the file. A stopped thread gives `internal`.
pub(crate) fn edit<T: Send + 'static>(
    file: &SettingsFile,
    change: impl FnOnce(&Store) -> Result<T, ErrorCode> + Send + 'static,
) -> ApiResult<T> {
    file.edit(change)
        .unwrap_or(Err(ErrorCode::Internal))
        .map_err(ApiError::from)
}

/// App information for `KinigeApi.appInfo()`.
#[tauri::command]
pub fn app_info() -> AppInfo {
    kinige_domain::app_info(env!("CARGO_PKG_VERSION"))
}

/// The interface language from the system's preferred languages (ADR-0005).
#[tauri::command]
pub fn get_language() -> Language {
    let preferred: Vec<String> = sys_locale::get_locales().collect();
    kinige_domain::lang::pick_language(&preferred)
}

/// Sidebar chats in display order (pinned, chats, archive) and the trash size.
#[tauri::command(async)]
pub fn list_chats(store: State<'_, Store>) -> ApiResult<ChatList> {
    store.sidebar().map_err(api)
}

/// A page of the chat feed; `tag` keeps only entries with this tag (a click on `#tag`). Without `after` and `around` -- the newest
/// page or an older one after `before`; `after` -- a newer page after the cursor; `around` -- a page
/// around an entry (a jump from search). If several positions are given, the order is `around`, then `after`, then `before`.
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

/// Send; with `reply_to_id` (slice 5) it is a reply to an entry of the same chat. If there is no such entry, it is in the trash or the id is wrong,
/// the result is `reply_target_not_found` and nothing is written. With `attachment_ids` (slice 7) the chat's own draft attachments
/// are sent with the entry, in the order given; the text may then be empty. A foreign, unknown or repeated id is
/// `attachment_not_found`, more than ten is `attachment_limit`, and nothing is written.
#[tauri::command(async)]
pub fn send_message<R: Runtime>(
    app: AppHandle<R>,
    store: State<'_, Store>,
    chat_id: String,
    body: String,
    reply_to_id: Option<String>,
    attachment_ids: Option<Vec<String>>,
) -> ApiResult<MessageView> {
    let message = store
        .send_entry(
            &chat_id,
            &body,
            reply_to_id.as_deref(),
            attachment_ids.as_deref().unwrap_or_default(),
            &Now::system(),
        )
        .map_err(api)?;
    crate::tray::refresh(&app);
    Ok(message)
}

/// Settings: the text size, the copy state and the keyboard shortcuts with their "assigned / taken" state from the system.
#[tauri::command(async)]
pub fn get_settings(store: State<'_, Store>, hotkeys: State<'_, Hotkeys>) -> ApiResult<Settings> {
    settings_with_shortcuts(&store, &hotkeys)
}

fn settings_with_shortcuts(store: &Store, hotkeys: &Hotkeys) -> ApiResult<Settings> {
    let mut settings = store.get_settings().map_err(api)?;
    hotkeys.overlay(&mut settings);
    Ok(settings)
}

/// The draft of the chat input field (§5): survives a restart; it also carries the entry that is being replied to (slice 5).
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

/// The "On this day" card of Inbox (slice 5); `null` means there are no entries of this date in past years or the card is hidden for today.
#[tauri::command(async)]
pub fn get_on_this_day(store: State<'_, Store>) -> ApiResult<Option<OnThisDay>> {
    store.get_on_this_day(&Now::system()).map_err(api)
}

/// "Hide until tomorrow": until the end of the journal day `date` the card is not shown. The state is local (`app_setting`).
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

/// Changing a keyboard shortcut: check before saving, registration in the system, saving. A refusal is an error code, the previous
/// shortcut keeps working. The reply is the whole settings.
#[tauri::command(async)]
pub fn set_shortcut<R: Runtime>(
    app: AppHandle<R>,
    store: State<'_, Store>,
    hotkeys: State<'_, Hotkeys>,
    file: State<'_, SettingsFile>,
    kind: ShortcutKind,
    accelerator: String,
) -> ApiResult<Settings> {
    // The edit goes through the file thread: it applies an external edit if there is one, and writes the file (ADR-0010).
    edit(&file, move |store| {
        let hotkeys = app.try_state::<Hotkeys>().ok_or(ErrorCode::Internal)?;
        hotkeys.change(store, kind, &accelerator)
    })?;
    settings_with_shortcuts(&store, &hotkeys)
}

/// "Reset": the default shortcut with the same checks.
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

/// Recording of a new shortcut in "Settings" started (`true`) or ended: for that time the hotkeys do not open the capture window,
/// otherwise an already assigned shortcut could not be pressed in the recording field.
#[tauri::command]
pub fn shortcut_recording(hotkeys: State<'_, Hotkeys>, active: bool) {
    hotkeys.set_recording(active);
}

#[tauri::command(async)]
pub fn set_text_size(file: State<'_, SettingsFile>, size: TextSize) -> ApiResult<()> {
    edit(&file, move |store| store.set_text_size(size).map_err(code))
}

/// The system dialog for choosing the Markdown copy folder. The dialog is called here, from Rust: the window has no JS permissions for dialogs,
/// and the path does not come from JS (§12.3). Cancelling the dialog changes nothing.
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
    // A blocking call is acceptable: the command runs in a thread pool (`async`), not on the main thread.
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

/// Opens a link from an entry in the system browser. The scheme is checked in `kinige-domain`: only `http`, `https`, `mailto`.
#[tauri::command(async)]
pub fn open_external_url<R: Runtime>(app: AppHandle<R>, url: String) -> ApiResult<()> {
    let checked = validate_external_url(&url).map_err(|error: LinkError| {
        log::warn!("link rejected: {error}");
        ApiError::from(ErrorCode::LinkNotAllowed)
    })?;
    app.opener().open_url(checked, None::<&str>).map_err(|_| {
        // We do not write the opener error text: it may contain the address itself.
        log::warn!("could not open the link in the system browser");
        ApiError::from(ErrorCode::Internal)
    })
}
