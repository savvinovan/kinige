//! Журнал обращений ИИ-агентов (ADR-0009, §12.3 ТЗ): факт обращения, а не содержимое. `args_json` — только белый список
//! полей (`kinige_domain::agent::args_json`), id затронутых записей — в `mcp_log_message`. Хранение: 90 дней и не больше
//! 20 000 строк; `purge` записи и ротация удаляют строки `mcp_log_message` в той же транзакции, сирот не остаётся.

use crate::agent::AgentError;
use crate::error::{read_err, write_err};
use crate::view::{VIEW_COLUMNS, VIEW_LEN, view_from_row_at};
use crate::{Now, Store, StoreError};
use kinige_domain::groups::INBOX_ID;
use kinige_domain::{AgentLogEntry, AgentLogPage, AgentLogRow};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior};

/// Сколько дней хранится журнал.
pub const LOG_DAYS: i64 = 90;
/// Сколько строк журнала хранится самое большее.
pub const LOG_MAX_ROWS: i64 = 20_000;
/// Строк на странице журнала в окне.
pub const LOG_PAGE: usize = 30;
const DAY_MS: i64 = 24 * 60 * 60 * 1000;

/// Строка журнала и её записи. Вызывающий открыл транзакцию писателя: данные и журнал фиксируются вместе.
pub(crate) fn insert_log(
    conn: &Connection,
    at_ms: i64,
    client: &str,
    tool: &str,
    args_json: &str,
    outcome: &str,
    message_ids: &[String],
) -> rusqlite::Result<()> {
    let unique: std::collections::BTreeSet<&String> = message_ids.iter().collect();
    conn.execute(
        "INSERT INTO mcp_log (at, client, tool, args_json, outcome, entry_count) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![at_ms, client, tool, args_json, outcome, unique.len() as i64],
    )?;
    let log_id = conn.last_insert_rowid();
    for id in unique {
        conn.execute(
            "INSERT OR IGNORE INTO mcp_log_message (log_id, message_id) VALUES (?1, ?2)",
            rusqlite::params![log_id, id],
        )?;
    }
    Ok(())
}

/// Обращение без изменения данных (отказ, подключение): отдельная транзакция писателя с одной строкой журнала.
pub(crate) fn log_only(
    store: &Store,
    client: &str,
    tool: &str,
    args_json: &str,
    outcome: &str,
    now: &Now,
) -> Result<(), AgentError> {
    let (client, tool, args_json, outcome, ms) = (
        client.to_string(),
        tool.to_string(),
        args_json.to_string(),
        outcome.to_string(),
        now.ms,
    );
    store
        .write(move |conn| {
            let tx = conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(write_err)?;
            insert_log(&tx, ms, &client, &tool, &args_json, &outcome, &[]).map_err(write_err)?;
            tx.commit().map_err(write_err)
        })
        .map_err(AgentError::from)
}

/// Строка подключения (`connect`) или переподключения моста (`reconnect`). При переподключении прежний сеанс клиента,
/// если его последняя строка — подключение (не «отключение»), закрывается строкой «отключение» в той же транзакции.
pub(crate) fn log_connection(
    store: &Store,
    client: &str,
    reconnect: bool,
    now: &Now,
) -> Result<(), StoreError> {
    let (client, ms) = (client.to_string(), now.ms);
    store.write(move |conn| {
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(write_err)?;
        if reconnect {
            let last: Option<String> = tx
                .query_row(
                    "SELECT tool FROM mcp_log WHERE client = ?1 AND tool IN ('connect', 'reconnect', 'disconnect') \
                     ORDER BY id DESC LIMIT 1",
                    [&client],
                    |row| row.get(0),
                )
                .optional()
                .map_err(write_err)?;
            if matches!(last.as_deref(), Some("connect" | "reconnect")) {
                insert_log(&tx, ms, &client, "disconnect", "{}", "ok", &[]).map_err(write_err)?;
            }
        }
        let tool = if reconnect { "reconnect" } else { "connect" };
        insert_log(&tx, ms, &client, tool, "{}", "ok", &[]).map_err(write_err)?;
        tx.commit().map_err(write_err)
    })
}

/// Удаление записи навсегда убирает и её следы в журнале; сама строка обращения остаётся.
pub(crate) fn delete_log_messages(conn: &Connection, message_id: &str) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM mcp_log_message WHERE message_id = ?1",
        [message_id],
    )?;
    Ok(())
}

/// Ротация: старше 90 дней и сверх 20 000 строк; строки `mcp_log_message` удалённых обращений уходят в той же транзакции.
pub(crate) fn rotate(conn: &Connection, now_ms: i64) -> rusqlite::Result<usize> {
    let old = conn.execute(
        "DELETE FROM mcp_log WHERE at < ?1",
        [now_ms.saturating_sub(LOG_DAYS * DAY_MS)],
    )?;
    let extra = conn.execute(
        "DELETE FROM mcp_log WHERE id <= (SELECT id FROM mcp_log ORDER BY id DESC LIMIT 1 OFFSET ?1)",
        [LOG_MAX_ROWS],
    )?;
    conn.execute(
        "DELETE FROM mcp_log_message WHERE log_id NOT IN (SELECT id FROM mcp_log)",
        [],
    )?;
    Ok(old + extra)
}

impl Store {
    /// Журнал обращений для окна, новые сверху. `before` — `id` последней строки предыдущей страницы.
    pub fn agent_log_page(&self, before: Option<i64>) -> Result<AgentLogPage, StoreError> {
        self.read(|conn| {
            let mut statement = conn
                .prepare(
                    "SELECT l.id, l.at, l.client, l.tool, json_extract(l.args_json, '$.query'), l.outcome, l.entry_count \
                     FROM mcp_log l WHERE l.id < COALESCE(?1, 9223372036854775807) \
                     ORDER BY l.id DESC LIMIT ?2",
                )
                .map_err(read_err)?;
            let rows = statement
                .query_map(rusqlite::params![before, LOG_PAGE as i64 + 1], |row| {
                    Ok(AgentLogRow {
                        id: row.get(0)?,
                        at: row.get(1)?,
                        client: row.get(2)?,
                        tool: row.get(3)?,
                        query: row.get(4)?,
                        outcome: row.get(5)?,
                        count: row.get(6)?,
                    })
                })
                .map_err(read_err)?;
            let mut rows = rows.collect::<Result<Vec<_>, _>>().map_err(read_err)?;
            let has_more = rows.len() > LOG_PAGE;
            rows.truncate(LOG_PAGE);
            let older = has_more.then(|| rows.last().map(|row| row.id)).flatten();
            Ok(AgentLogPage { rows, older })
        })
    }

    /// Записи, которые затронуло обращение: сама запись (в том числе из корзины) или «удалена навсегда» (`None`).
    pub fn agent_log_entries(&self, log_id: i64) -> Result<Vec<AgentLogEntry>, StoreError> {
        self.read(|conn| {
            let mut ids = conn
                .prepare(
                    "SELECT message_id FROM mcp_log_message WHERE log_id = ?1 ORDER BY message_id",
                )
                .map_err(read_err)?;
            let ids = ids
                .query_map([log_id], |row| row.get::<_, String>(0))
                .map_err(read_err)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(read_err)?;
            let mut entries: Vec<AgentLogEntry> = ids
                .into_iter()
                .map(|message_id| log_entry(conn, message_id))
                .collect::<Result<_, _>>()?;
            // Хронологически: по дате и времени записи; удалённые — в конце.
            entries.sort_by(|a, b| match (&a.message, &b.message) {
                (Some(x), Some(y)) => {
                    (&x.journal_date, x.created_at).cmp(&(&y.journal_date, y.created_at))
                }
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            });
            Ok(entries)
        })
    }

    /// Ротация журнала в обслуживании: возвращает, сколько строк удалено.
    pub fn rotate_agent_log(&self, now: &Now) -> Result<usize, StoreError> {
        let now_ms = now.ms;
        self.write(move |conn| {
            let tx = conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(write_err)?;
            let removed = rotate(&tx, now_ms).map_err(write_err)?;
            tx.commit().map_err(write_err)?;
            Ok(removed)
        })
    }
}

fn log_entry(conn: &Connection, message_id: String) -> Result<AgentLogEntry, StoreError> {
    let found = conn
        .query_row(
            &format!(
                "SELECT {VIEW_COLUMNS}, c.title, c.icon FROM message m \
                 LEFT JOIN chat c ON c.id = m.chat_id WHERE m.id = ?1"
            ),
            [&message_id],
            |row| {
                Ok((
                    view_from_row_at(row, 0)?,
                    row.get::<_, Option<String>>(VIEW_LEN)?,
                    row.get::<_, Option<String>>(VIEW_LEN + 1)?,
                ))
            },
        )
        .optional()
        .map_err(read_err)?;
    Ok(match found {
        Some((message, title, icon)) => AgentLogEntry {
            chat_is_inbox: message.chat_id == INBOX_ID,
            chat_title: title.unwrap_or_default(),
            chat_icon: icon,
            message: Some(message),
            message_id,
        },
        None => AgentLogEntry {
            message_id,
            message: None,
            chat_title: String::new(),
            chat_icon: None,
            chat_is_inbox: false,
        },
    })
}
