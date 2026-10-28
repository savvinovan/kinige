//! Команды окна «Доступ ИИ-агентов» (слайс 4a, ADR-0009): флажок, режим, видимые чаты, состояние сокета, журнал обращений.
//! Только главное окно. Бизнес-правил здесь нет: они в `kinige-store`; сервер MCP проверяет настройки при каждом вызове.

use crate::mcp;
use kinige_domain::{AgentAccess, AgentLogEntry, AgentLogPage, AgentMode, ApiError};
use kinige_store::{Store, StoreError};
use tauri::{AppHandle, Runtime, State};

type ApiResult<T> = Result<T, ApiError>;

fn api(error: StoreError) -> ApiError {
    // В журнал процесса — только категория: текст внутренней ошибки может содержать куски данных.
    log::error!("ошибка хранилища: {}", error.category());
    error.code().into()
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
pub fn set_agent_enabled(store: State<'_, Store>, enabled: bool) -> ApiResult<()> {
    store.set_agent_enabled(enabled).map_err(api)
}

/// Режим «Только чтение» / «Чтение и запись».
#[tauri::command(async)]
pub fn set_agent_mode(store: State<'_, Store>, mode: AgentMode) -> ApiResult<()> {
    store.set_agent_mode(mode).map_err(api)
}

/// Чаты, видимые агенту. Принимаются только существующие чаты без `ai_excluded`.
#[tauri::command(async)]
pub fn set_agent_chats(store: State<'_, Store>, chat_ids: Vec<String>) -> ApiResult<()> {
    store.set_agent_chats(chat_ids).map_err(api)
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
