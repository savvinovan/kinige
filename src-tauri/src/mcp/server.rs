//! The MCP handler of one connection (ADR-0009): the handshake, the tool list by mode, a tool call.
//!
//! Rust checks permissions in the store on every call; here only protocol parsing and delivery. Each call takes
//! the `Store` from the app state anew and checks "database is corrupt": a `Store` clone is not kept in tasks, so
//! a restore from a copy does not find a live writer descriptor. No call shows a window, focus or a toast (§11).

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
use std::sync::{Arc, Mutex, PoisonError};
use tauri::{AppHandle, Manager, Runtime};

/// What the connection told about itself and whether the connect was written to the log (a disconnect is written only if a connect was).
#[derive(Default)]
pub struct ConnectionInfo {
    pub client: Mutex<Option<String>>,
    pub connected_logged: AtomicBool,
}

impl ConnectionInfo {
    /// Whether `initialize` happened on this connection (the client introduced itself). Without it calls are not executed: the built-in lifecycle
    /// of `rmcp` (`_meta.protocolVersion` in the request) lets `tools/call` bypass the handshake, and the server does not trust that.
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
    /// The connection was opened by the bridge after a drop: the repeated `initialize` is not written to the log as a new connect.
    pub reconnect: bool,
    pub info: Arc<ConnectionInfo>,
}

/// The Inbox name in the interface language: in the database it is empty.
fn inbox_title() -> &'static str {
    match get_language() {
        Language::Ru => "Входящие",
        Language::En => "Inbox",
    }
}

/// The store for a call: `None` if the journal is not open (recovery mode) or corruption was found.
/// It is taken inside a blocking task and lives only until its end.
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

/// A blocking task: a panic inside turns into a reply error, the app keeps working.
async fn blocking<R: Runtime, T: Send + 'static>(
    app: &AppHandle<R>,
    job: impl FnOnce(&Store) -> T + Send + 'static,
) -> Result<T, AgentError> {
    let app = app.clone();
    match tauri::async_runtime::spawn_blocking(move || with_store(&app, job)).await {
        Ok(result) => result,
        Err(_) => {
            log::error!("mcp: panic in the call task");
            Err(AgentError::Internal)
        }
    }
}

impl<R: Runtime> McpServer<R> {
    /// The log row "connect" (or "reconnect" after a bridge drop), once per connection. Called on
    /// `initialize` and once more before the first tool call if the write failed (the database is in recovery, a write failure):
    /// so a call always has a connect row.
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
        *self
            .info
            .client
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(client.clone());
        context.peer.set_peer_info(request.clone());
        self.record_connection().await;
        self.negotiate_initialize(&request)
    }

    /// Only protocol versions with `initialize` (up to 2025-11-25). Version 2026-07-28 allows the first request without a handshake
    /// (`_meta.protocolVersion`): the session would run without a "connect" row and without the client name. The list alone is not enough: `rmcp`
    /// lets a request with `_meta.protocolVersion = 2025-11-25` through too, so `list_tools` and `call_tool` themselves require
    /// `initialize` (`ConnectionInfo::initialized`). `server/discover` returns only `get_info` (name, `instructions`), no data.
    fn supported_protocol_versions(&self) -> Cow<'static, [ProtocolVersion]> {
        Cow::Borrowed(ProtocolVersion::known_up_to(&ProtocolVersion::V_2025_11_25))
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        self.require_initialize()?;
        // The mode is taken from the settings on every request; the list is visible even with access turned off (it holds no data).
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
        // Without `initialize` a call reaches neither the database nor the log.
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
                    // An open feed is refreshed quietly: the window is not brought forward and does not get focus.
                    notify_entries_changed(&self.app);
                }
                CallToolResult::success(vec![ContentBlock::text(reply.text)])
            }
            Err(error) => CallToolResult::error(vec![ContentBlock::text(explain(&error))]),
        };
        Ok(result.into())
    }
}

/// A client disconnect: one log row, if a connect was written.
pub async fn disconnected<R: Runtime>(app: &AppHandle<R>, info: &ConnectionInfo) {
    if info.connected_logged.load(Ordering::SeqCst) {
        let client = info.client();
        let _ = blocking(app, move |store| {
            store.agent_disconnected(&client, &Now::system())
        })
        .await;
    }
}
