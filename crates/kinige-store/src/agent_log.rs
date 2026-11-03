//! The access log of AI agents (ADR-0009, PRD §12.3): the fact of an access, not the content. `args_json` holds only an allowlist of
//! fields (`kinige_domain::agent::args_json`), the ids of the affected entries go to `mcp_log_message`. Retention: 90 days and at most
//! 20,000 rows; an entry `purge` and the rotation delete the `mcp_log_message` rows in the same transaction, so no orphans remain.

use crate::agent::AgentError;
use crate::error::{read_err, write_err};
use crate::view::{VIEW_COLUMNS, VIEW_LEN, view_from_row_at};
use crate::{Now, Store, StoreError};
use kinige_domain::groups::INBOX_ID;
use kinige_domain::{AgentLogEntry, AgentLogPage, AgentLogRow};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior};

/// How many days the log is kept.
pub const LOG_DAYS: i64 = 90;
/// The most log rows kept.
pub const LOG_MAX_ROWS: i64 = 20_000;
/// Rows per log page in the window.
pub const LOG_PAGE: usize = 30;
const DAY_MS: i64 = 24 * 60 * 60 * 1000;

/// A log row and its entries. The caller has opened the writer transaction: the data and the log are committed together.
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

/// An access that changes no data (a refusal, a connection): a separate writer transaction with one log row.
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

/// A connection row (`connect`) or a bridge reconnection row (`reconnect`). On reconnection the client's previous session,
/// if its last row is a connection (not a "disconnect"), is closed with a "disconnect" row in the same transaction.
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

/// Deleting an entry forever also removes its traces in the log; the access row itself stays.
pub(crate) fn delete_log_messages(conn: &Connection, message_id: &str) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM mcp_log_message WHERE message_id = ?1",
        [message_id],
    )?;
    Ok(())
}

/// Rotation: older than 90 days and beyond 20,000 rows; the `mcp_log_message` rows of the deleted accesses go in the same transaction.
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
    /// The access log for the window, newest on top. `before` is the `id` of the last row of the previous page.
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

    /// The entries an access touched: the entry itself (including from the trash) or "deleted forever" (`None`).
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
            // Chronologically: by the date and time of the entry; the deleted ones go last.
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

    /// Log rotation during maintenance: returns how many rows were deleted.
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
        Some((mut message, title, icon)) => {
            // The log shows what the agent saw, and it is not given `reply_to_id` or the quote (ADR-0009).
            message.reply_to = None;
            AgentLogEntry {
                chat_is_inbox: message.chat_id == INBOX_ID,
                chat_title: title.unwrap_or_default(),
                chat_icon: icon,
                message: Some(message),
                message_id,
            }
        }
        None => AgentLogEntry {
            message_id,
            message: None,
            chat_title: String::new(),
            chat_icon: None,
            chat_is_inbox: false,
        },
    })
}
