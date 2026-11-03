//! Commands of the "AI agent access" window (slice 4a, ADR-0009): the switch, the mode, visible chats, the socket state, the access log.
//! The main window only. There are no business rules here: they are in `kinige-store`; the MCP server checks the settings on every call.

use crate::commands::edit;
use crate::mcp;
use kinige_domain::{AgentAccess, AgentLogEntry, AgentLogPage, AgentMode, ApiError, ErrorCode};
use kinige_store::{SettingsFile, Store, StoreError};
use tauri::{AppHandle, Runtime, State};

type ApiResult<T> = Result<T, ApiError>;

fn api(error: StoreError) -> ApiError {
    // Only the category goes to the process log: the text of an internal error may contain pieces of data.
    log::error!("store error: {}", error.category());
    error.code().into()
}

fn code(error: StoreError) -> ErrorCode {
    api(error).code
}

/// AI-agent access does not exist everywhere: Windows has no MCP server until named pipes (slice 10), so there is nothing
/// to change. The settings in the database and in the file are not touched.
fn ensure_supported() -> ApiResult<()> {
    if cfg!(unix) {
        Ok(())
    } else {
        Err(ErrorCode::Unsupported.into())
    }
}

/// The "AI agent access" window: the switch, the mode, chats with the "visible to the agent" mark, the socket state and connection snippets.
#[tauri::command(async)]
pub fn get_agent_access<R: Runtime>(
    app: AppHandle<R>,
    store: State<'_, Store>,
) -> ApiResult<AgentAccess> {
    let mut access = store.agent_access().map_err(api)?;
    access.socket = mcp::socket_info(&app);
    Ok(access)
}

/// The access switch. The warning on enabling is shown by the window before the call.
#[tauri::command(async)]
pub fn set_agent_enabled(file: State<'_, SettingsFile>, enabled: bool) -> ApiResult<()> {
    ensure_supported()?;
    edit(&file, move |store| {
        store.set_agent_enabled(enabled).map_err(code)
    })
}

/// The mode "Read only" / "Read and write".
#[tauri::command(async)]
pub fn set_agent_mode(file: State<'_, SettingsFile>, mode: AgentMode) -> ApiResult<()> {
    ensure_supported()?;
    edit(&file, move |store| store.set_agent_mode(mode).map_err(code))
}

/// Chats visible to the agent. Only existing chats without `ai_excluded` are accepted.
#[tauri::command(async)]
pub fn set_agent_chats(file: State<'_, SettingsFile>, chat_ids: Vec<String>) -> ApiResult<()> {
    ensure_supported()?;
    edit(&file, move |store| {
        store.set_agent_chats(chat_ids).map_err(code)
    })
}

/// The access log, newest first. `before` is the `id` of the last row of the previous page.
#[tauri::command(async)]
pub fn list_agent_log(store: State<'_, Store>, before: Option<i64>) -> ApiResult<AgentLogPage> {
    store.agent_log_page(before).map_err(api)
}

/// Entries touched by an access (expanding a log row). An entry deleted forever arrives as `message: null`.
#[tauri::command(async)]
pub fn list_agent_log_entries(
    store: State<'_, Store>,
    log_id: i64,
) -> ApiResult<Vec<AgentLogEntry>> {
    store.agent_log_entries(log_id).map_err(api)
}
