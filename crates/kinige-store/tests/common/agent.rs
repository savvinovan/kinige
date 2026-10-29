//! Помощники тестов доступа агентов (слайс 4a): окружение с чатами, вызов инструмента, строки журнала.

#![allow(dead_code)]

use super::*;
use kinige_domain::AgentMode;
use kinige_domain::groups::INBOX_ID;
use kinige_store::{AgentError, AgentRequest};
use serde_json::{Map, Value};
use tempfile::TempDir;

/// Хранилище с тремя чатами: «Работа» (отмечен агенту), «Секретный проект» (не отмечен), «Дневник» (`ai_excluded`, отмечен
/// в настройках, но агент его не видит).
pub struct Agent {
    pub dir: TempDir,
    pub store: Store,
    pub work: String,
    pub secret: String,
    pub excluded: String,
}

pub fn agent(mode: AgentMode, mark_inbox: bool) -> Agent {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let now = moscow(T0);
    let work = store.create_chat("Работа", None, &now).unwrap().id;
    let secret = store
        .create_chat("Секретный проект", None, &now)
        .unwrap()
        .id;
    let excluded = store.create_chat("Дневник", None, &now).unwrap().id;
    raw(dir.path())
        .execute("UPDATE chat SET ai_excluded = 1 WHERE id = ?1", [&excluded])
        .unwrap();
    let mut marked = vec![work.clone()];
    // Отмеченный Inbox — только если тест просит: по умолчанию агент не видит и его (решение 2).
    if mark_inbox {
        marked.push(INBOX_ID.to_string());
    }
    // `ai_excluded` в настройках не принимается: отмечаем в обход, как мог бы сделать повреждённый файл.
    store.set_agent_chats(marked.clone()).unwrap();
    store.set_agent_mode(mode).unwrap();
    store.set_agent_enabled(true).unwrap();
    marked.push(excluded.clone());
    raw(dir.path())
        .execute(
            "UPDATE app_setting SET value = ?1 WHERE key = 'mcp_chats'",
            [serde_json::to_string(&marked).unwrap()],
        )
        .unwrap();
    Agent {
        dir,
        store,
        work,
        secret,
        excluded,
    }
}

pub fn args(value: Value) -> Map<String, Value> {
    value.as_object().cloned().unwrap_or_default()
}

/// Вызов от имени клиента «Test Client» в момент `ms`.
pub fn call(store: &Store, tool: &str, arguments: Value, ms: i64) -> Result<Value, AgentError> {
    call_as(store, "Test Client", tool, arguments, ms)
}

pub fn call_as(
    store: &Store,
    client: &str,
    tool: &str,
    arguments: Value,
    ms: i64,
) -> Result<Value, AgentError> {
    let arguments = args(arguments);
    let reply = store.agent_call(
        &AgentRequest {
            client,
            tool,
            arguments: Some(&arguments),
            inbox_title: "Inbox",
        },
        &moscow(ms),
    )?;
    Ok(serde_json::from_str(&reply.text).expect("ответ — JSON"))
}

/// Строки `mcp_log` старые → новые: `(client, tool, args_json, outcome, число записей)`.
pub fn log_rows(dir: &Path) -> Vec<(String, String, String, String, i64)> {
    let conn = raw(dir);
    let mut statement = conn
        .prepare(
            "SELECT client, tool, args_json, outcome, \
             (SELECT COUNT(*) FROM mcp_log_message m WHERE m.log_id = l.id) FROM mcp_log l ORDER BY id",
        )
        .unwrap();
    statement
        .query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

pub fn count(dir: &Path, table: &str) -> i64 {
    raw(dir)
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
        .unwrap()
}

/// Все id записей агента из ответа `entries`.
pub fn entry_ids(reply: &Value) -> Vec<String> {
    reply["entries"]
        .as_array()
        .map(|entries| {
            entries
                .iter()
                .map(|e| e["id"].as_str().unwrap().to_string())
                .collect()
        })
        .unwrap_or_default()
}
