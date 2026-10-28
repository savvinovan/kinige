//! Обработчик MCP одного соединения (ADR-0009): рукопожатие, список инструментов по режиму, вызов инструмента.
//!
//! Права проверяет Rust в хранилище при каждом вызове; здесь только разбор протокола и доставка. Каждый вызов заново
//! берёт `Store` из состояния приложения и проверяет «база повреждена»: клон `Store` в задачах не хранится, поэтому
//! восстановление из копии не застаёт живой дескриптор писателя. Ни один вызов не показывает окно, фокус или тост (§11).

use super::tools::{INSTRUCTIONS, explain, tools};
use crate::capture::notify_entries_changed;
use crate::commands::get_language;
use crate::recovery::Recovery;
use kinige_domain::Language;
use kinige_store::{AgentError, AgentRequest, Now, Store};
use rmcp::model::ProtocolVersion;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, ErrorData,
    Implementation, InitializeRequestParams, InitializeResult, ListToolsResult,
    PaginatedRequestParams, ServerCapabilities, ServerConfig,
};
use rmcp::service::RequestContext;
use rmcp::{RoleServer, ServerHandler};
use std::borrow::Cow;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager, Runtime};

/// Что соединение сообщило о себе и записано ли подключение в журнал (отключение пишется, только если писалось подключение).
#[derive(Default)]
pub struct ConnectionInfo {
    pub client: Mutex<Option<String>>,
    pub connected_logged: AtomicBool,
}

impl ConnectionInfo {
    /// Был ли `initialize` на этом соединении (клиент назвал себя). Без него вызовы не выполняются: встроенный жизненный
    /// цикл `rmcp` (`_meta.protocolVersion` в запросе) пропускает `tools/call` мимо рукопожатия, а сервер ему не верит.
    pub fn initialized(&self) -> bool {
        match self.client.lock() {
            Ok(client) => client.is_some(),
            Err(poisoned) => poisoned.into_inner().is_some(),
        }
    }

    pub fn client(&self) -> String {
        match self.client.lock() {
            Ok(client) => client.clone().unwrap_or_default(),
            Err(poisoned) => poisoned.into_inner().clone().unwrap_or_default(),
        }
    }
}

pub struct McpServer<R: Runtime> {
    pub app: AppHandle<R>,
    /// Соединение открыто мостом после обрыва: повторный `initialize` не пишется в журнал как новое подключение.
    pub reconnect: bool,
    pub info: Arc<ConnectionInfo>,
}

/// Название Inbox на языке интерфейса: в базе оно пустое.
fn inbox_title() -> &'static str {
    match get_language() {
        Language::Ru => "Входящие",
        Language::En => "Inbox",
    }
}

/// Хранилище для вызова: `None`, если журнал не открыт (режим восстановления) или найдено повреждение.
/// Берётся внутри блокирующей задачи и живёт только до её конца.
fn with_store<R: Runtime, T>(
    app: &AppHandle<R>,
    job: impl FnOnce(&Store) -> T,
) -> Result<T, AgentError> {
    if app
        .try_state::<Recovery>()
        .is_some_and(|recovery| recovery.corruption_found() || recovery.is_active())
    {
        return Err(AgentError::Unavailable);
    }
    let store = app.try_state::<Store>().ok_or(AgentError::Unavailable)?;
    Ok(job(&store))
}

/// Блокирующая задача: паника внутри превращается в ошибку ответа, приложение продолжает работать.
async fn blocking<R: Runtime, T: Send + 'static>(
    app: &AppHandle<R>,
    job: impl FnOnce(&Store) -> T + Send + 'static,
) -> Result<T, AgentError> {
    let app = app.clone();
    match tauri::async_runtime::spawn_blocking(move || with_store(&app, job)).await {
        Ok(result) => result,
        Err(_) => {
            log::error!("mcp: паника в задаче вызова");
            Err(AgentError::Internal)
        }
    }
}

impl<R: Runtime> McpServer<R> {
    /// Строка журнала «подключение» (или «переподключение» после обрыва моста), один раз за соединение. Вызывается при
    /// `initialize` и ещё раз перед первым вызовом инструмента, если запись не удалась (база в восстановлении, сбой записи):
    /// так у вызова всегда есть строка подключения.
    async fn record_connection(&self) {
        if self.info.connected_logged.swap(true, Ordering::SeqCst) {
            return;
        }
        let (client, reconnect) = (self.info.client(), self.reconnect);
        let stored = blocking(&self.app, move |store| {
            store.agent_connected(&client, reconnect, &Now::system())
        })
        .await;
        if !matches!(stored, Ok(Ok(()))) {
            self.info.connected_logged.store(false, Ordering::SeqCst);
        }
    }

    fn require_initialize(&self) -> Result<(), ErrorData> {
        if self.info.initialized() {
            Ok(())
        } else {
            Err(ErrorData::invalid_request(
                "initialize is required before any other request",
                None,
            ))
        }
    }

    fn config() -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("kinige", env!("CARGO_PKG_VERSION")))
            .with_instructions(INSTRUCTIONS)
    }
}

impl<R: Runtime> ServerHandler for McpServer<R> {
    fn get_info(&self) -> ServerConfig {
        Self::config()
    }

    async fn initialize(
        &self,
        request: InitializeRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<InitializeResult, ErrorData> {
        let client = request.client_info.name.clone();
        if let Ok(mut slot) = self.info.client.lock() {
            *slot = Some(client.clone());
        }
        context.peer.set_peer_info(request.clone());
        self.record_connection().await;
        self.negotiate_initialize(&request)
    }

    /// Только версии протокола с `initialize` (до 2025-11-25). Версия 2026-07-28 разрешает первый запрос без рукопожатия
    /// (`_meta.protocolVersion`): сеанс шёл бы без строки «подключение» и без имени клиента. Одного списка мало: `rmcp`
    /// пропускает запрос с `_meta.protocolVersion = 2025-11-25` тоже, поэтому `list_tools` и `call_tool` сами требуют
    /// `initialize` (`ConnectionInfo::initialized`). `server/discover` отдаёт только `get_info` (имя, `instructions`), данных нет.
    fn supported_protocol_versions(&self) -> Cow<'static, [ProtocolVersion]> {
        Cow::Borrowed(ProtocolVersion::known_up_to(&ProtocolVersion::V_2025_11_25))
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        self.require_initialize()?;
        // Режим берётся из настроек при каждом запросе; список виден и при выключенном доступе (данных в нём нет).
        let mode = blocking(&self.app, |store| store.agent_mode())
            .await
            .ok()
            .and_then(Result::ok)
            .unwrap_or(kinige_domain::AgentMode::Read);
        Ok(ListToolsResult {
            tools: tools(mode),
            ..Default::default()
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        // Без `initialize` вызов не доходит ни до базы, ни до журнала.
        self.require_initialize()?;
        self.record_connection().await;
        let client = self.info.client();
        let name = request.name.to_string();
        let arguments = request.arguments;
        let outcome = blocking(&self.app, move |store| {
            store.agent_call(
                &AgentRequest {
                    client: &client,
                    tool: &name,
                    arguments: arguments.as_ref(),
                    inbox_title: inbox_title(),
                },
                &Now::system(),
            )
        })
        .await
        .and_then(|reply| reply);
        let result = match outcome {
            Ok(reply) => {
                if reply.changed {
                    // Открытая лента обновляется тихо: окно не выводится вперёд и не получает фокус.
                    notify_entries_changed(&self.app);
                }
                CallToolResult::success(vec![ContentBlock::text(reply.text)])
            }
            Err(error) => CallToolResult::error(vec![ContentBlock::text(explain(&error))]),
        };
        Ok(result.into())
    }
}

/// Отключение клиента: одна строка журнала, если писалось подключение.
pub async fn disconnected<R: Runtime>(app: &AppHandle<R>, info: &ConnectionInfo) {
    if info.connected_logged.load(Ordering::SeqCst) {
        let client = info.client();
        let _ = blocking(app, move |store| {
            store.agent_disconnected(&client, &Now::system())
        })
        .await;
    }
}
