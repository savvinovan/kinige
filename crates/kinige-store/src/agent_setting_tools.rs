//! The two settings tools of MCP (slice 5d, ADR-0022): `get_settings` reads what an agent may know, `set_setting` changes the
//! text size and nothing else. The reply is built from `kinige_domain::agent_prefs::SettingsReply`, field by field: no path, no
//! chat title, no window state and no raw `app_setting` key reaches an agent.

use crate::agent::{AgentReply, Call, VISIBLE, load_settings};
use crate::agent_read::ReadOutput;
use crate::error::read_err;
use crate::settings::{
    KEY_MIRROR_DIR, KEY_TEXT_SIZE, get_setting, put_text_size, stored_accelerator,
};
use crate::{Store, StoreError};
use kinige_domain::agent_prefs::{
    AgentAccessReply, MarkdownCopy, SettingsReply, ShortcutValue, ShortcutsReply, writable_names,
};
use kinige_domain::{ShortcutKind, TextSize};
use rusqlite::Connection;
use std::path::Path;

impl Store {
    /// `get_settings`: read on the agent's own connection. The caller has checked that access is on.
    pub(crate) fn read_settings_for_agent(&self) -> Result<ReadOutput, StoreError> {
        let default = self.inner.default_mirror_dir.clone();
        let reply = self.read_agent(|conn| settings_reply(conn, default.as_deref()))?;
        Ok(ReadOutput {
            text: to_text(&reply)?,
            ids: Vec::new(),
        })
    }
}

/// `set_setting` inside the writer transaction that also writes the log row. Only the text size gets here: the caller
/// refused every other name before the transaction, and a name that is not writable has no `value`.
pub(crate) fn set_text_size_in_tx(
    conn: &Connection,
    call: &Call,
    size: TextSize,
) -> Result<AgentReply, StoreError> {
    put_text_size(conn, size)?;
    let reply = settings_reply(conn, call.default_mirror.as_deref())?;
    Ok(AgentReply {
        text: to_text(&reply)?,
        changed: false,
        settings_changed: true,
    })
}

fn to_text(reply: &SettingsReply) -> Result<String, StoreError> {
    serde_json::to_string(reply).map_err(|error| StoreError::Internal(error.to_string()))
}

/// The reply of both tools. Every field is picked by hand.
pub(crate) fn settings_reply(
    conn: &Connection,
    default_mirror: Option<&Path>,
) -> Result<SettingsReply, StoreError> {
    let text_size = get_setting(conn, KEY_TEXT_SIZE)
        .map_err(read_err)?
        .map_or(TextSize::Normal, |text| TextSize::from_setting(&text));
    let shortcut = |kind: ShortcutKind| -> Result<ShortcutValue, StoreError> {
        Ok(ShortcutValue {
            value: stored_accelerator(conn, kind).map_err(read_err)?,
            default: kind.default_accelerator().to_string(),
        })
    };
    let markdown_copy = match (
        get_setting(conn, KEY_MIRROR_DIR).map_err(read_err)?,
        default_mirror,
    ) {
        (Some(_), _) => MarkdownCopy::ChosenFolder,
        (None, Some(_)) => MarkdownCopy::DefaultFolder,
        (None, None) => MarkdownCopy::Off,
    };
    Ok(SettingsReply {
        text_size,
        shortcuts: ShortcutsReply {
            capture: shortcut(ShortcutKind::Capture)?,
            clipboard: shortcut(ShortcutKind::Clipboard)?,
        },
        markdown_copy,
        agent_access: AgentAccessReply {
            mode: load_settings(conn)?.mode,
            visible_chat_ids: visible_chat_ids(conn)?,
        },
        writable: writable_names(),
    })
}

/// The chats the agent can see, by ADR-0009's predicate: an id in `mcp_chats` of an excluded, deleted or missing chat is not here.
fn visible_chat_ids(conn: &Connection) -> Result<Vec<String>, StoreError> {
    let mut statement = conn
        .prepare(&format!(
            "SELECT c.id FROM chat c WHERE {VISIBLE} ORDER BY c.created_at, c.id"
        ))
        .map_err(read_err)?;
    let ids = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(read_err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(read_err)?;
    Ok(ids)
}
