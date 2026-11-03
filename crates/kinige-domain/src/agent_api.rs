//! Types of the "AI agent access" window (slice 4a, ADR-0009): access settings, socket state, the call log.
//! TS types are generated in `web/src/api/generated/` (ADR-0002).

use crate::MessageView;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The agent's access mode: one switch for all visible chats (decision 3, 28.10.2026).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "AgentMode.ts")]
pub enum AgentMode {
    /// Read-only: writing and marking tasks are refused.
    Read,
    /// Reading and writing in any visible chat.
    ReadWrite,
}

impl AgentMode {
    /// The value in `app_setting`. An unknown value means "read-only": safer.
    pub fn setting(self) -> &'static str {
        match self {
            AgentMode::Read => "read",
            AgentMode::ReadWrite => "read_write",
        }
    }

    pub fn from_setting(text: &str) -> AgentMode {
        if text == "read_write" {
            AgentMode::ReadWrite
        } else {
            AgentMode::Read
        }
    }
}

/// A chat in the "visible to the agent" list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "AgentChatChoice.ts")]
pub struct AgentChatChoice {
    pub id: String,
    pub title: String,
    pub icon: Option<String>,
    pub is_inbox: bool,
    #[ts(type = "number")]
    pub count: i64,
    /// Excluded from AI (`ai_excluded`): the row is inactive, the agent never sees it.
    pub ai_excluded: bool,
    /// Marked "visible to the agent".
    pub visible: bool,
}

/// The MCP socket state, connection snippets and the path to the bridge. Filled in by the Tauri layer.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "AgentSocket.ts")]
pub struct AgentSocket {
    /// The socket is listening.
    pub listening: bool,
    /// Why it is not listening:`dir_unsafe`, `second_instance`, `bind_failed`, `bridge_missing`.
    pub error: Option<String>,
    /// The absolute path of the bridge (`kinige-mcp`) next to the app.
    pub bridge_path: Option<String>,
    pub socket_path: Option<String>,
    /// A snippet for `claude_desktop_config.json`.
    pub desktop_config: Option<String>,
    /// The `claude mcp add …` command.
    pub code_command: Option<String>,
}

/// The "AI agent access" window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "AgentAccess.ts")]
pub struct AgentAccess {
    pub enabled: bool,
    pub mode: AgentMode,
    pub chats: Vec<AgentChatChoice>,
    pub socket: AgentSocket,
}

/// A call log row: the fact of the call without the entries' content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "AgentLogRow.ts")]
pub struct AgentLogRow {
    #[ts(type = "number")]
    pub id: i64,
    /// UTC, ms.
    #[ts(type = "number")]
    pub at: i64,
    /// The client name as the client states it: not verified.
    pub client: String,
    /// The tool name or a service session line: `connect`, `reconnect` (the bridge reconnected), `disconnect`.
    pub tool: String,
    /// The search query (up to 128 characters).
    pub query: Option<String>,
    /// `ok`, `disabled`, `read_only`, `not_found`, `no_agent_entry`, `not_a_task`, `invalid_argument`, `error`.
    pub outcome: String,
    /// How many entries the call touched. Written together with the row and does not change after "Delete permanently" (expanding
    /// will show fewer entries, the window will say how many were deleted).
    #[ts(type = "number")]
    pub count: i64,
}

/// A log page, the newest on top.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "AgentLogPage.ts")]
pub struct AgentLogPage {
    pub rows: Vec<AgentLogRow>,
    /// The `id` of the page's last row, if there are older ones.
    #[ts(type = "number | null")]
    pub older: Option<i64>,
}

/// An entry the call touched: the entry itself or `None` if it was deleted permanently.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "AgentLogEntry.ts")]
pub struct AgentLogEntry {
    pub message_id: String,
    pub message: Option<MessageView>,
    pub chat_title: String,
    pub chat_icon: Option<String>,
    pub chat_is_inbox: bool,
}
