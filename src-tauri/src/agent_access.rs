//! Команды окна «Доступ ИИ-агентов» (слайс 4a, ADR-0009): флажок, режим, видимые чаты, состояние сокета, журнал обращений.
//! Только главное окно. Бизнес-правил здесь нет: они в `kinige-store`; сервер MCP проверяет настройки при каждом вызове.

use crate::commands::edit;
use crate::mcp;
use kinige_domain::{AgentAccess, AgentLogEntry, AgentLogPage, AgentMode, ApiError, ErrorCode};
use kinige_store::{SettingsFile, Store, StoreError};
use tauri::{AppHandle, Runtime, State};

type ApiResult<T> = Result<T, ApiError>;

fn api(error: StoreError) -> ApiError {
    // В журнал процесса — только категория: текст внутренней ошибки может содержать куски данных.
    log::error!("ошибка хранилища: {}", error.category());
    error.code().into()
}

fn code(error: StoreError) -> ErrorCode {
    api(error).code
}

/// Окно «Доступ ИИ-агентов»: флажок, режим, чаты с отметкой «видно агенту», состояние сокета и фрагменты подключения.
#[tauri::command(async)]
pub fn get_agent_access<R: Runtime>(
    app: AppHandle<R>,
    store: State<'_, Store>,
) -> ApiResult<AgentAccess> {
    let mut access = store.agent_access().map_err(api)?;
    access.socket = mcp::socket_info(&app);
    Ok(access)
}

/// Выключатель доступа. Предупреждение при включении показывает окно до вызова.
#[tauri::command(async)]
pub fn set_agent_enabled(file: State<'_, SettingsFile>, enabled: bool) -> ApiResult<()> {
    edit(&file, move |store| {
        store.set_agent_enabled(enabled).map_err(code)
    })
}

/// Режим «Только чтение» / «Чтение и запись».
#[tauri::command(async)]
pub fn set_agent_mode(file: State<'_, SettingsFile>, mode: AgentMode) -> ApiResult<()> {
    edit(&file, move |store| store.set_agent_mode(mode).map_err(code))
}

/// Чаты, видимые агенту. Принимаются только существующие чаты без `ai_excluded`.
#[tauri::command(async)]
pub fn set_agent_chats(file: State<'_, SettingsFile>, chat_ids: Vec<String>) -> ApiResult<()> {
    edit(&file, move |store| {
        store.set_agent_chats(chat_ids).map_err(code)
    })
}

/// Журнал обращений, новые сверху. `before` — `id` последней строки предыдущей страницы.
#[tauri::command(async)]
pub fn list_agent_log(store: State<'_, Store>, before: Option<i64>) -> ApiResult<AgentLogPage> {
    store.agent_log_page(before).map_err(api)
}

/// Записи, которые затронуло обращение (раскрытие строки журнала). Удалённая навсегда запись приходит как `message: null`.
#[tauri::command(async)]
pub fn list_agent_log_entries(
    store: State<'_, Store>,
    log_id: i64,
) -> ApiResult<Vec<AgentLogEntry>> {
    store.agent_log_entries(log_id).map_err(api)
}
