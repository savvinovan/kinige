//! Access for AI agents through MCP (ADR-0009, slice 4a): the access settings and the single entry point for a tool call.
//!
//! An agent gets nothing around [`Store::agent_call`]: the switch and the mode are checked on every call, the visible set
//! of chats (`VISIBLE`) goes into every read statement and into the write transaction, every call leaves exactly one
//! log row (`agent_log`), and entry text does not get into the log. We trust neither clients nor the bridge.

use crate::agent_log::{insert_log, log_connection, log_only};
use crate::agent_read::{ReadCtx, ReadFail, ReadOutput};
use crate::agent_write::run_write_job;
use crate::error::{read_err, write_err};
use crate::settings::get_setting;
use crate::{Now, Store, StoreError};
use kinige_domain::AgentMode;
use kinige_domain::agent::{
    AgentTool, ToolArgs, args_json, clean_client, parse_args, refusal_args_json,
};
use kinige_domain::agent_prefs::set_setting_args_json;
use rusqlite::{Connection, TransactionBehavior};
use serde_json::{Map, Value};
use std::path::PathBuf;

pub(crate) const KEY_ENABLED: &str = "mcp_enabled";
pub(crate) const KEY_MODE: &str = "mcp_mode";
pub(crate) const KEY_CHATS: &str = "mcp_chats";

/// A chat (`c`) is visible to the agent iff it is marked in `mcp_chats`, not excluded from AI and not deleted. Archived is visible. The predicate sits in each
/// agent read statement and inside the write transaction: between the statements of one call the set may change, each
/// statement is checked on its own. No key -> `json_each(NULL)` is empty -> no visible chats. Ids that are not in the
/// database match nothing.
pub(crate) const VISIBLE: &str = "c.deleted_at IS NULL AND c.ai_excluded = 0 AND c.id IN \
     (SELECT j.value FROM json_each((SELECT value FROM app_setting WHERE key = 'mcp_chats')) j)";

/// Agent access settings (`app_setting`, not synchronized).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSettings {
    pub enabled: bool,
    pub mode: AgentMode,
    pub chats: Vec<String>,
}

/// Settings from the database. A damaged `mcp_chats` (not an array of strings) is an error without data; no key -- an empty set.
pub(crate) fn load_settings(conn: &Connection) -> Result<AgentSettings, StoreError> {
    let enabled = get_setting(conn, KEY_ENABLED).map_err(read_err)?;
    let mode = get_setting(conn, KEY_MODE).map_err(read_err)?;
    let chats = match get_setting(conn, KEY_CHATS).map_err(read_err)? {
        None => Vec::new(),
        Some(text) => serde_json::from_str::<Vec<String>>(&text)
            .map_err(|_| StoreError::Internal("the mcp_chats value is damaged".into()))?,
    };
    Ok(AgentSettings {
        enabled: enabled.as_deref() == Some("1"),
        mode: mode
            .as_deref()
            .map_or(AgentMode::Read, AgentMode::from_setting),
        chats,
    })
}

/// A tool call from an agent. `client` is the client's own words (`clientInfo.name`), it goes into the log sanitized.
pub struct AgentRequest<'a> {
    pub client: &'a str,
    pub tool: &'a str,
    pub arguments: Option<&'a Map<String, Value>>,
    /// The Inbox name in the interface language (the database stores it empty).
    pub inbox_title: &'a str,
}

/// The reply to an agent: text with JSON and a "data changed" flag (the Tauri layer quietly refreshes the feed and the tray).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentReply {
    pub text: String,
    pub changed: bool,
    /// The text size was set (slice 5d): the Tauri layer tells the main and settings windows, as after a click.
    pub settings_changed: bool,
}

/// A refusal to an agent. A closed chat, `ai_excluded`, a deleted one, a nonexistent one and an entry in the trash all give the same `NotFound`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentError {
    Disabled,
    ReadOnly,
    NotFound,
    NoAgentEntry,
    NotATask,
    Invalid(String),
    /// A setting only the user may change, in the settings window (slice 5d).
    NotWritable,
    /// The database is corrupted: there is no data.
    Unavailable,
    /// Everything else; details only in the log category.
    Internal,
}

impl AgentError {
    /// The error code for the agent.
    pub fn code(&self) -> &'static str {
        match self {
            AgentError::Disabled => "mcp_disabled",
            AgentError::ReadOnly => "mcp_read_only",
            AgentError::NotFound => "not_found",
            AgentError::NoAgentEntry => "no_agent_entry",
            AgentError::NotATask => "not_a_task",
            AgentError::Invalid(_) => "invalid_argument",
            AgentError::NotWritable => "setting_not_writable",
            AgentError::Unavailable => "journal_unavailable",
            AgentError::Internal => "internal_error",
        }
    }

    /// The outcome for the access log.
    pub(crate) fn outcome(&self) -> &'static str {
        match self {
            AgentError::Disabled => "disabled",
            AgentError::ReadOnly => "read_only",
            AgentError::NotFound => "not_found",
            AgentError::NoAgentEntry => "no_agent_entry",
            AgentError::NotATask => "not_a_task",
            AgentError::Invalid(_) => "invalid_argument",
            AgentError::NotWritable => "not_writable",
            AgentError::Unavailable | AgentError::Internal => "error",
        }
    }
}

impl From<StoreError> for AgentError {
    fn from(error: StoreError) -> Self {
        log::warn!("agent: {}", error.category());
        match error {
            StoreError::Corrupt(_) => AgentError::Unavailable,
            _ => AgentError::Internal,
        }
    }
}

impl Store {
    /// The single entry point of a tool call. Order: tool known -> access enabled -> mode -> arguments -> execution.
    /// Every path leaves exactly one log row; a log write failure is an error to the agent, it gets no data.
    pub fn agent_call(
        &self,
        request: &AgentRequest<'_>,
        now: &Now,
    ) -> Result<AgentReply, AgentError> {
        let client = clean_client(request.client);
        let Some(tool) = AgentTool::from_name(request.tool) else {
            self.agent_refuse(
                &client,
                "unknown",
                "{}",
                &AgentError::Invalid("unknown tool".into()),
                now,
            )?;
            return Err(AgentError::Invalid("unknown tool".into()));
        };
        let early = early_args_json(tool, request.arguments);
        let settings = self.read_agent(load_settings)?;
        if !settings.enabled {
            return self.agent_refuse(&client, tool.name(), &early, &AgentError::Disabled, now);
        }
        if tool.writes() && settings.mode != AgentMode::ReadWrite {
            return self.agent_refuse(&client, tool.name(), &early, &AgentError::ReadOnly, now);
        }
        let args = match parse_args(tool, request.arguments) {
            Ok(args) => args,
            Err(why) => {
                let invalid = AgentError::Invalid(why.0);
                return self.agent_refuse(&client, tool.name(), &early, &invalid, now);
            }
        };
        // A setting only the user may change is refused before any transaction: it writes nothing.
        if let ToolArgs::SetSetting { key, .. } = &args
            && !key.writable()
        {
            let refused = refusal_args_json(&args);
            return self.agent_refuse(
                &client,
                tool.name(),
                &refused,
                &AgentError::NotWritable,
                now,
            );
        }
        let call = Call {
            client,
            tool,
            args_json: args_json(&args),
            failed_args_json: refusal_args_json(&args),
            inbox_title: request.inbox_title.to_string(),
            default_mirror: self.inner.default_mirror_dir.clone(),
            now: now.clone(),
        };
        let result = if tool.writes() {
            self.agent_write(&call, args)
        } else {
            self.agent_read(&call, &args)
        };
        if let Err(AgentError::Internal | AgentError::Unavailable) = &result {
            // Execution failed: there is no data, but the call stays in the log if the database still accepts writes.
            let _ = log_only(
                self,
                &call.client,
                tool.name(),
                &call.failed_args_json,
                "error",
                now,
            );
        }
        result
    }

    /// A refusal without execution: one log row, then an error to the agent.
    pub(crate) fn agent_refuse(
        &self,
        client: &str,
        tool: &str,
        args_json: &str,
        error: &AgentError,
        now: &Now,
    ) -> Result<AgentReply, AgentError> {
        log_only(self, client, tool, args_json, error.outcome(), now)?;
        Err(error.clone())
    }

    /// Read: a select on a read connection, then a writer transaction with a log row, then the reply. A log failure is
    /// an error, not data.
    fn agent_read(&self, call: &Call, args: &ToolArgs) -> Result<AgentReply, AgentError> {
        let ctx = ReadCtx::new(&call.inbox_title, &call.now);
        let read = match self.read_for(&ctx, args) {
            Ok(read) => read,
            Err(ReadFail::NotFound) => {
                return self.agent_refuse(
                    &call.client,
                    call.tool.name(),
                    &call.args_json,
                    &AgentError::NotFound,
                    &call.now,
                );
            }
            Err(ReadFail::Store(error)) => return Err(error.into()),
        };
        let job = call.clone();
        self.write(move |conn| {
            let tx = conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(write_err)?;
            let (tool, ms) = (job.tool.name(), job.now.ms);
            insert_log(&tx, ms, &job.client, tool, &job.args_json, "ok", &read.ids)
                .map_err(write_err)?;
            tx.commit().map_err(write_err)?;
            Ok(AgentReply {
                text: read.text,
                changed: false,
                settings_changed: false,
            })
        })
        .map_err(AgentError::from)
    }

    /// The statement part of a read: `get_settings` has no search index to heal and its own reader job.
    fn read_for(&self, ctx: &ReadCtx<'_>, args: &ToolArgs) -> Result<ReadOutput, ReadFail> {
        match args {
            ToolArgs::GetSettings => self.read_settings_for_agent().map_err(ReadFail::Store),
            _ => self.read_with_heal(ctx, args),
        }
    }

    /// Write: one writer transaction -- the access and mode check, chat visibility, the data change and the log row.
    fn agent_write(&self, call: &Call, args: ToolArgs) -> Result<AgentReply, AgentError> {
        let call = call.clone();
        #[cfg(feature = "test-hooks")]
        self.run_agent_gap_hook();
        let reply = self.write(move |conn| run_write_job(conn, &call, &args))??;
        self.wake_mirror();
        Ok(reply)
    }

    /// A client connecting (`initialize`): one log row with the sanitized client name. `reconnect` -- the connection was opened
    /// by the bridge after a drop: a `reconnect` row, and the previous session of this client, if it stayed open (the app
    /// was restarted), is closed with a "disconnect" row in the same transaction. There are no silent paths: every connection is a row.
    pub fn agent_connected(
        &self,
        client: &str,
        reconnect: bool,
        now: &Now,
    ) -> Result<(), StoreError> {
        log_connection(self, &clean_client(client), reconnect, now).map_err(|error| {
            log::warn!("agent: connection log: {}", error.category());
            StoreError::Write("connection log".into())
        })
    }

    /// A client disconnecting: one log row.
    pub fn agent_disconnected(&self, client: &str, now: &Now) -> Result<(), StoreError> {
        log_only(self, &clean_client(client), "disconnect", "{}", "ok", now).map_err(|error| {
            log::warn!("agent: disconnection log: {}", error.code());
            StoreError::Write("disconnection log".into())
        })
    }
}

/// One tool call after argument parsing (everything owned: it goes to the writer thread).
#[derive(Clone)]
pub(crate) struct Call {
    pub(crate) client: String,
    pub(crate) tool: AgentTool,
    pub(crate) args_json: String,
    /// What the log gets when the call does not succeed: the same without a `set_setting` value.
    pub(crate) failed_args_json: String,
    pub(crate) inbox_title: String,
    /// The default copy folder, to say "default folder" in the settings reply.
    pub(crate) default_mirror: Option<PathBuf>,
    pub(crate) now: Now,
}

/// `args_json` of a refusal before the arguments are parsed: only `set_setting` has any, and only a known name.
fn early_args_json(tool: AgentTool, arguments: Option<&Map<String, Value>>) -> String {
    if tool == AgentTool::SetSetting {
        let key = arguments
            .and_then(|arguments| arguments.get("key"))
            .and_then(Value::as_str);
        set_setting_args_json(key, None)
    } else {
        "{}".to_string()
    }
}
