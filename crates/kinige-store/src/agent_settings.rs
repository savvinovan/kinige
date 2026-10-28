//! Настройки доступа агентов для окна «Доступ ИИ-агентов» (`app_setting`, локальные, не синхронизируются):
//! `mcp_enabled`, `mcp_mode`, `mcp_chats`. По умолчанию доступ выключен, режим «только чтение», видимых чатов нет.

use crate::agent::{KEY_CHATS, KEY_ENABLED, KEY_MODE, load_settings};
use crate::error::{read_err, write_err};
use crate::feed::chat_list;
use crate::{Store, StoreError};
use kinige_domain::{AgentAccess, AgentChatChoice, AgentMode, AgentSocket};
use rusqlite::{Connection, TransactionBehavior};
use std::collections::HashSet;

fn put(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO app_setting (key, value) VALUES (?1, ?2) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        (key, value),
    )?;
    Ok(())
}

impl Store {
    /// Состояние доступа для окна: флажок, режим, все чаты с отметкой «видно агенту». Состояние сокета заполняет слой Tauri.
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

    /// Режим доступа (для списка инструментов: в режиме «только чтение» инструментов записи в списке нет).
    pub fn agent_mode(&self) -> Result<AgentMode, StoreError> {
        self.read(load_settings).map(|settings| settings.mode)
    }

    /// Выключатель доступа. Выключенный доступ отказывает каждому вызову инструмента (`mcp_disabled`).
    pub fn set_agent_enabled(&self, enabled: bool) -> Result<(), StoreError> {
        self.write(move |conn| {
            put(conn, KEY_ENABLED, if enabled { "1" } else { "0" }).map_err(write_err)
        })
    }

    /// Режим доступа. Клиент видит прежний список инструментов до переподключения; сами вызовы проверяются по новому.
    pub fn set_agent_mode(&self, mode: AgentMode) -> Result<(), StoreError> {
        self.write(move |conn| put(conn, KEY_MODE, mode.setting()).map_err(write_err))
    }

    /// Чаты, видимые агенту. Принимаются только существующие чаты без `ai_excluded`; иначе `ChatNotFound` и ничего не
    /// меняется. Повторы убираются, порядок сохраняется.
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
