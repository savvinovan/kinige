//! Main window commands of slice 3: chats, entry actions, trash, sidebar and picker settings. There are no business rules here
//! (they are in `kinige-domain` and `kinige-store`); after changes the icon menu is refreshed. These commands are not granted to the capture window.

use kinige_domain::{ApiError, ChatColor, ChatSummary, MessageFlag, MessageView, TrashItem};
use kinige_store::{Now, Store, StoreError};
use tauri::{AppHandle, Runtime, State};

type ApiResult<T> = Result<T, ApiError>;

fn api(error: StoreError) -> ApiError {
    // Only the category goes to the process log: the text of an internal error may contain pieces of data.
    log::error!("store error: {}", error.category());
    error.code().into()
}

/// The "+" of the "Chats" section: a new chat at the end of the section; without an emoji it gets 💬.
#[tauri::command(async)]
pub fn create_chat<R: Runtime>(
    app: AppHandle<R>,
    store: State<'_, Store>,
    title: String,
    icon: Option<String>,
) -> ApiResult<ChatSummary> {
    let chat = store
        .create_chat(&title, icon.as_deref(), &Now::system())
        .map_err(api)?;
    crate::tray::refresh(&app);
    Ok(chat)
}

#[tauri::command(async)]
pub fn rename_chat<R: Runtime>(
    app: AppHandle<R>,
    store: State<'_, Store>,
    id: String,
    title: String,
) -> ApiResult<ChatSummary> {
    let chat = store
        .rename_chat(&id, &title, &Now::system())
        .map_err(api)?;
    crate::tray::refresh(&app);
    Ok(chat)
}

#[tauri::command(async)]
pub fn set_chat_icon<R: Runtime>(
    app: AppHandle<R>,
    store: State<'_, Store>,
    id: String,
    icon: String,
) -> ApiResult<ChatSummary> {
    let chat = store
        .set_chat_icon(&id, &icon, &Now::system())
        .map_err(api)?;
    crate::tray::refresh(&app);
    Ok(chat)
}

/// "Colour" in the chat menu (KINIG-59): the letter tile colour. An unknown name is refused when the arguments are read.
#[tauri::command(async)]
pub fn set_chat_color(
    store: State<'_, Store>,
    id: String,
    color: ChatColor,
) -> ApiResult<ChatSummary> {
    store
        .set_chat_color(&id, color, &Now::system())
        .map_err(api)
}

#[tauri::command(async)]
pub fn set_chat_pinned(
    store: State<'_, Store>,
    id: String,
    pinned: bool,
) -> ApiResult<ChatSummary> {
    store
        .set_chat_pinned(&id, pinned, &Now::system())
        .map_err(api)
}

#[tauri::command(async)]
pub fn set_chat_archived<R: Runtime>(
    app: AppHandle<R>,
    store: State<'_, Store>,
    id: String,
    archived: bool,
) -> ApiResult<ChatSummary> {
    let chat = store
        .set_chat_archived(&id, archived, &Now::system())
        .map_err(api)?;
    crate::tray::refresh(&app);
    Ok(chat)
}

/// "Delete": the entry goes to the trash. No optimistic display: the reply comes after the commit.
#[tauri::command(async)]
pub fn delete_message<R: Runtime>(
    app: AppHandle<R>,
    store: State<'_, Store>,
    id: String,
) -> ApiResult<()> {
    store.delete_message(&id, &Now::system()).map_err(api)?;
    crate::tray::refresh(&app);
    Ok(())
}

#[tauri::command(async)]
pub fn restore_message<R: Runtime>(
    app: AppHandle<R>,
    store: State<'_, Store>,
    id: String,
) -> ApiResult<()> {
    store.restore_message(&id, &Now::system()).map_err(api)?;
    crate::tray::refresh(&app);
    Ok(())
}

/// Editing the text in the bubble: the reply is the entry as it now looks; the old version is kept as a revision.
#[tauri::command(async)]
pub fn edit_message(store: State<'_, Store>, id: String, body: String) -> ApiResult<MessageView> {
    store.edit_message(&id, &body, &Now::system()).map_err(api)
}

/// A click on `- [ ]` inside an entry: the item `index` (zero-based) gets the state `checked`.
#[tauri::command(async)]
pub fn toggle_task_item(
    store: State<'_, Store>,
    id: String,
    index: usize,
    checked: bool,
) -> ApiResult<MessageView> {
    store
        .toggle_task_item(&id, index, checked, &Now::system())
        .map_err(api)
}

/// Task, done, favorite.
#[tauri::command(async)]
pub fn set_message_flag(
    store: State<'_, Store>,
    id: String,
    flag: MessageFlag,
    on: bool,
) -> ApiResult<MessageView> {
    store
        .set_message_flag(&id, flag, on, &Now::system())
        .map_err(api)
}

/// "Change date": `YYYY-MM-DD`; future dates are allowed.
#[tauri::command(async)]
pub fn set_message_date(
    store: State<'_, Store>,
    id: String,
    journal_date: String,
) -> ApiResult<MessageView> {
    store
        .set_message_date(&id, &journal_date, &Now::system())
        .map_err(api)
}

/// "Move to chat": the entry moves to another chat (the day and text do not change, no revision).
#[tauri::command(async)]
pub fn move_message<R: Runtime>(
    app: AppHandle<R>,
    store: State<'_, Store>,
    id: String,
    chat_id: String,
) -> ApiResult<MessageView> {
    let view = store
        .move_message(&id, &chat_id, &Now::system())
        .map_err(api)?;
    // "Recent chats" in the icon menu depend on the last entry of a chat.
    crate::tray::refresh(&app);
    Ok(view)
}

#[tauri::command(async)]
pub fn list_trash(store: State<'_, Store>) -> ApiResult<Vec<TrashItem>> {
    store.list_trash(&Now::system()).map_err(api)
}

/// "Delete forever" for an entry in the trash (the frontend asks for confirmation).
#[tauri::command(async)]
pub fn purge_message(store: State<'_, Store>, id: String) -> ApiResult<()> {
    store.purge_message(&id, &Now::system()).map_err(api)
}

#[tauri::command(async)]
pub fn set_sidebar_collapsed(store: State<'_, Store>, collapsed: bool) -> ApiResult<()> {
    store.set_sidebar_collapsed(collapsed).map_err(api)
}

/// The emoji chosen in the picker goes first in "recent"; the reply is the whole list.
#[tauri::command(async)]
pub fn add_recent_emoji(store: State<'_, Store>, emoji: String) -> ApiResult<Vec<String>> {
    store.add_recent_emoji(&emoji).map_err(api)
}
