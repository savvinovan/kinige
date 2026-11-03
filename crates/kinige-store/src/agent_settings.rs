//! Agent access settings for the "AI agent access" window (`app_setting`, local, not synced):
//! `mcp_enabled`, `mcp_mode`, `mcp_chats`. By default access is off, the mode is "read only", and no chats are visible.

use crate::agent::{KEY_CHATS, KEY_ENABLED, KEY_MODE, load_settings};
use crate::error::{read_err, write_err};
use crate::feed::chat_list;
use crate::{Store, StoreError};
use kinige_domain::{AgentAccess, AgentChatChoice, AgentMode, AgentSocket};
use rusqlite::{Connection, TransactionBehavior};
use std::collections::HashSet;

pub(crate) fn put(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO app_setting (key, value) VALUES (?1, ?2) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        (key, value),
    )?;
    Ok(())
}

impl Store {
    /// Access state for the window: the switch, the mode, all chats with a "visible to the agent" mark. The Tauri layer
    /// fills in the socket state.
    pub fn agent_access(&self) -> Result<AgentAccess, StoreError> {
        self.read(|conn| {
            let settings = load_settings(conn)?;
            let excluded: HashSet<String> = conn
                .prepare("SELECT id FROM chat WHERE ai_excluded = 1 AND deleted_at IS NULL")
                .map_err(read_err)?
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(read_err)?
                .collect::<Result<_, _>>()
                .map_err(read_err)?;
            let chosen: HashSet<&String> = settings.chats.iter().collect();
            let chats = chat_list(conn)?
                .chats
                .into_iter()
                .map(|chat| {
                    let ai_excluded = excluded.contains(&chat.id);
                    AgentChatChoice {
                        visible: !ai_excluded && chosen.contains(&chat.id),
                        ai_excluded,
                        id: chat.id,
                        title: chat.title,
                        icon: chat.icon,
                        is_inbox: chat.is_inbox,
                        count: chat.count,
                    }
                })
                .collect();
            Ok(AgentAccess {
                enabled: settings.enabled,
                mode: settings.mode,
                chats,
                socket: AgentSocket::default(),
            })
        })
    }

    /// Access mode (for the tool list: in "read only" mode the list has no write tools).
    pub fn agent_mode(&self) -> Result<AgentMode, StoreError> {
        self.read(load_settings).map(|settings| settings.mode)
    }

    /// The access switch. With access off, every tool call is refused (`mcp_disabled`).
    pub fn set_agent_enabled(&self, enabled: bool) -> Result<(), StoreError> {
        self.write(move |conn| {
            put(conn, KEY_ENABLED, if enabled { "1" } else { "0" }).map_err(write_err)
        })
    }

    /// The access mode. The client sees the previous tool list until it reconnects; the calls themselves are checked
    /// against the new one.
    pub fn set_agent_mode(&self, mode: AgentMode) -> Result<(), StoreError> {
        self.write(move |conn| put(conn, KEY_MODE, mode.setting()).map_err(write_err))
    }

    /// Chats visible to the agent. Only existing chats without `ai_excluded` are accepted; otherwise `ChatNotFound` and
    /// nothing changes. Duplicates are removed, the order is kept.
    pub fn set_agent_chats(&self, ids: Vec<String>) -> Result<(), StoreError> {
        let mut seen = HashSet::new();
        let ids: Vec<String> = ids
            .into_iter()
            .filter(|id| seen.insert(id.clone()))
            .collect();
        self.write(move |conn| {
            let tx = conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(write_err)?;
            for id in &ids {
                let eligible: bool = tx
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM chat WHERE id = ?1 AND deleted_at IS NULL AND ai_excluded = 0)",
                        [id],
                        |row| row.get(0),
                    )
                    .map_err(write_err)?;
                if !eligible {
                    return Err(StoreError::ChatNotFound);
                }
            }
            let value = serde_json::to_string(&ids)
                .map_err(|error| StoreError::Internal(error.to_string()))?;
            put(&tx, KEY_CHATS, &value).map_err(write_err)?;
            tx.commit().map_err(write_err)
        })
    }
}
