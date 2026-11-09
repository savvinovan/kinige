//! Reading for the agent (ADR-0009): every statement carries the predicate of the visible set of chats (`VISIBLE`), the trash is not visible.
//! The list of chats for the search "Chats" row is a separate statement with the same predicate, not a filter in Rust.

use crate::agent::VISIBLE;
use crate::agent_entry::{chat_title, page, to_entry};
use crate::error::read_err;
use crate::search_sql::{Filters, Position, SearchSql};
use crate::view::{VIEW_COLUMNS, VIEW_LEN, view_from_row_at};
use crate::{Now, Store, StoreError, read_on};
use kinige_domain::MessageView;
use kinige_domain::agent::{
    AgentCursor, ENTRIES_PAGE, EntriesArgs, SEARCH_PAGE, SearchArgs, ToolArgs,
};
use kinige_domain::agent_reply::{AgentChat, fit_entries, month_day};
use kinige_domain::groups::INBOX_ID;
use kinige_domain::journal::{DAY_BOUNDARY_MINUTES, journal_date};
use kinige_domain::on_this_day::{past_dates, year_of};
use kinige_domain::search::{fts_query, title_matches};
use rusqlite::types::Value;
use rusqlite::{Connection, Row, params_from_iter};
use serde_json::json;
use std::panic::{AssertUnwindSafe, catch_unwind};

/// What the call knows at the time of reading.
pub(crate) struct ReadCtx<'a> {
    pub(crate) inbox_title: &'a str,
    today: String,
}

impl<'a> ReadCtx<'a> {
    pub(crate) fn new(inbox_title: &'a str, now: &Now) -> Self {
        ReadCtx {
            inbox_title,
            today: journal_date(now.ms, now.utc_offset_secs, DAY_BOUNDARY_MINUTES),
        }
    }
}

/// The read reply: the text for the agent and the entry ids that go to the log (only those that made it into the reply).
pub(crate) struct ReadOutput {
    pub(crate) text: String,
    pub(crate) ids: Vec<String>,
}

pub(crate) enum ReadFail {
    NotFound,
    Store(StoreError),
}

impl From<StoreError> for ReadFail {
    fn from(error: StoreError) -> Self {
        ReadFail::Store(error)
    }
}

impl From<rusqlite::Error> for ReadFail {
    fn from(error: rusqlite::Error) -> Self {
        ReadFail::Store(read_err(error))
    }
}

impl Store {
    /// A read for an AI agent's tool call, on the agent's own connection (KINIG-47.2): a long agent query (a search over
    /// 100,000 entries) must not hold the window's reads (feed pages, the chat list, the tray menu) behind the same lock.
    pub(crate) fn read_agent<T>(
        &self,
        job: impl FnOnce(&Connection) -> Result<T, StoreError>,
    ) -> Result<T, StoreError> {
        let result = read_on(&self.inner.agent_reader, job);
        self.note_if_corrupt(&result);
        result
    }

    /// Reading on the read connection; a panic inside the query does not poison the connection. A damaged search index
    /// is healed by a rebuild and one retry (as in the main window search).
    pub(crate) fn read_with_heal(
        &self,
        ctx: &ReadCtx<'_>,
        args: &ToolArgs,
    ) -> Result<ReadOutput, ReadFail> {
        let run = |conn: &Connection| run_read(conn, args, ctx);
        self.read_healing(|| self.with_reader(run))
    }

    fn with_reader<T>(
        &self,
        job: impl FnOnce(&Connection) -> Result<T, ReadFail>,
    ) -> Result<T, ReadFail> {
        // As in `read_unnoted`: a panic inside the read (release profile with stack unwinding) is caught here and the
        // call gets `Internal`; if the lock is poisoned anyway, it is taken as is -- the read connection (`query_only`,
        // no transaction) is consistent after unwinding. Otherwise one panic would break all agent reads until a restart.
        // The agent has its own connection (KINIG-47.2): the window's reads do not queue behind an agent query.
        let conn = self
            .inner
            .agent_reader
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        catch_unwind(AssertUnwindSafe(|| job(&conn))).unwrap_or_else(|_| {
            log::error!("panic in an agent read, call cancelled");
            Err(ReadFail::Store(StoreError::Internal(
                "panic in an agent query".into(),
            )))
        })
    }

    /// Tests only: runs `held` while an agent read occupies the agent connection (its lock is taken and kept). The window's reads
    /// made inside `held` must return, because they use their own connection.
    #[doc(hidden)]
    pub fn with_agent_read_held_for_test<T>(&self, held: impl FnOnce() -> T) -> T {
        let _agent = self
            .inner
            .agent_reader
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        held()
    }

    /// Tests only: tries to write through the agent's read connection. It is `query_only`, so the error text is returned
    /// (`attempt to write a readonly database`) and nothing is written.
    #[doc(hidden)]
    pub fn write_through_agent_read_for_test(&self) -> String {
        let attempt = self.read_agent(|conn| {
            Ok(conn
                .execute("UPDATE chat SET title = title || ' (agent)'", [])
                .map(|_| String::new())
                .unwrap_or_else(|error| error.to_string()))
        });
        attempt.unwrap_or_default()
    }

    /// Tests only: a panic inside `query_map` on the agent read connection. The following agent reads must keep working.
    #[doc(hidden)]
    #[allow(clippy::panic)] // a hook for the integration tests of the panic safety net: the panic is its whole purpose
    pub fn panic_in_agent_read_for_test(&self) -> Result<(), StoreError> {
        let result = self.with_reader(|conn| -> Result<(), ReadFail> {
            let mut statement = conn.prepare("SELECT id FROM chat")?;
            let rows = statement.query_map([], |_| -> rusqlite::Result<()> {
                panic!("test: panic in an agent query")
            })?;
            for row in rows {
                row?;
            }
            Ok(())
        });
        match result {
            Ok(()) => Ok(()),
            Err(ReadFail::Store(error)) => Err(error),
            Err(ReadFail::NotFound) => Err(StoreError::Internal("not found".into())),
        }
    }
}

fn run_read(conn: &Connection, args: &ToolArgs, ctx: &ReadCtx<'_>) -> Result<ReadOutput, ReadFail> {
    match args {
        ToolArgs::ListChats => list_chats(conn, ctx),
        ToolArgs::Search(search) => search_entries(conn, search, ctx),
        ToolArgs::GetEntries(entries) => get_entries(conn, entries, ctx),
        ToolArgs::OnThisDay { date } => on_this_day(conn, date.as_deref(), ctx),
        ToolArgs::GetEntry { id } => get_entry(conn, id, ctx),
        // Write tools never get here: `agent_write` handles them.
        ToolArgs::AddEntry { .. }
        | ToolArgs::AppendToLastEntry { .. }
        | ToolArgs::SetTaskDone { .. } => {
            Err(StoreError::Internal("write tool in a read".into()).into())
        }
    }
}

/// The chat with this id is visible to the agent. A closed, AI-excluded, deleted and nonexistent chat are indistinguishable.
pub(crate) fn chat_visible(conn: &Connection, id: &str) -> rusqlite::Result<bool> {
    conn.query_row(
        &format!("SELECT EXISTS(SELECT 1 FROM chat c WHERE c.id = ?1 AND {VISIBLE})"),
        [id],
        |row| row.get(0),
    )
}

/// A query with numbered parameters.
struct Sql {
    text: String,
    params: Vec<Value>,
}

impl Sql {
    fn new(text: String) -> Sql {
        Sql {
            text,
            params: Vec::new(),
        }
    }

    /// Adds the condition `<left> <op> ?N`.
    fn and(&mut self, condition: &str, value: impl Into<Value>) {
        self.params.push(value.into());
        self.text
            .push_str(&format!(" AND {condition} ?{}", self.params.len()));
    }

    /// Adds the condition `<left> IN (?N, ...)`.
    fn and_in(&mut self, left: &str, values: Vec<String>) {
        let marks: Vec<String> = values
            .into_iter()
            .map(|value| {
                self.params.push(value.into());
                format!("?{}", self.params.len())
            })
            .collect();
        self.text
            .push_str(&format!(" AND {left} IN ({})", marks.join(", ")));
    }

    fn rows(&self, conn: &Connection) -> Result<Vec<(MessageView, String)>, ReadFail> {
        let mut statement = conn.prepare(&self.text)?;
        let rows = statement.query_map(params_from_iter(&self.params), entry_row)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }
}

/// An entry and the title of its chat: the columns `VIEW_COLUMNS, c.title`.
fn entry_row(row: &Row<'_>) -> rusqlite::Result<(MessageView, String)> {
    Ok((view_from_row_at(row, 0)?, row.get::<_, String>(VIEW_LEN)?))
}

fn list_chats(conn: &Connection, ctx: &ReadCtx<'_>) -> Result<ReadOutput, ReadFail> {
    let chats = visible_chats(conn, ctx)?;
    Ok(ReadOutput {
        text: json!({ "chats": chats }).to_string(),
        ids: Vec::new(),
    })
}

/// Visible chats: a separate statement with the predicate (used by both the chat list and the search "Chats" row).
fn visible_chats(conn: &Connection, ctx: &ReadCtx<'_>) -> Result<Vec<AgentChat>, ReadFail> {
    let mut statement = conn.prepare(&format!(
        "SELECT c.id, c.title, c.icon, c.archived_at IS NOT NULL, \
         (SELECT COUNT(*) FROM message m WHERE m.chat_id = c.id) \
         - (SELECT COUNT(*) FROM message m INDEXED BY message_deleted \
            WHERE m.chat_id = c.id AND m.deleted_at IS NOT NULL) \
         FROM chat c WHERE {VISIBLE} ORDER BY c.sort_key, c.id"
    ))?;
    let rows = statement.query_map([], |row| {
        let id: String = row.get(0)?;
        let title: String = row.get(1)?;
        Ok(AgentChat {
            title: if id == INBOX_ID {
                ctx.inbox_title.to_string()
            } else {
                title
            },
            id,
            emoji: row.get(2)?,
            archived: row.get(3)?,
            entries: row.get(4)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn search_entries(
    conn: &Connection,
    args: &SearchArgs,
    ctx: &ReadCtx<'_>,
) -> Result<ReadOutput, ReadFail> {
    if let Some(chat) = &args.chat_id
        && !chat_visible(conn, chat)?
    {
        return Err(ReadFail::NotFound);
    }
    let Some(query) = fts_query(&args.query) else {
        return page(Vec::new(), false, Vec::new(), ctx);
    };
    let sql = SearchSql {
        chat_condition: VISIBLE,
        extra_columns: "c.title",
        leading: Vec::new(),
        query,
        filters: Filters {
            favorite: args.favorites,
            tasks: args.tasks,
            chat_id: args.chat_id.as_deref(),
            from: args.from.as_deref(),
            to: args.to.as_deref(),
        },
        // The search goes from new to old: the next page is strictly earlier than the cursor position.
        before: args.cursor.as_ref().map(|cursor| Position {
            journal_date: &cursor.journal_date,
            created_at: cursor.created_at,
            id: &cursor.id,
        }),
        limit: SEARCH_PAGE,
    };
    let (found, has_more) = sql.run(conn, entry_row)?;
    // The "Chats" row is only on the first page, as a separate statement with the predicate.
    let chats = if args.cursor.is_none() {
        visible_chats(conn, ctx)?
            .into_iter()
            .filter(|chat| chat.id != INBOX_ID && title_matches(&chat.title, &args.query))
            .collect()
    } else {
        Vec::new()
    };
    page(found, has_more, chats, ctx)
}

fn keyset(sql: &mut Sql, cursor: &AgentCursor, op: &str) {
    let at = sql.params.len();
    sql.params.push(cursor.journal_date.clone().into());
    sql.params.push(cursor.created_at.into());
    sql.params.push(cursor.id.clone().into());
    sql.text.push_str(&format!(
        " AND (m.journal_date, m.created_at, m.id) {op} (?{}, ?{}, ?{})",
        at + 1,
        at + 2,
        at + 3
    ));
}

fn get_entries(
    conn: &Connection,
    args: &EntriesArgs,
    ctx: &ReadCtx<'_>,
) -> Result<ReadOutput, ReadFail> {
    if let Some(chat) = &args.chat_id
        && !chat_visible(conn, chat)?
    {
        return Err(ReadFail::NotFound);
    }
    let mut sql = Sql::new(format!(
        "SELECT {VIEW_COLUMNS}, c.title FROM message m JOIN chat c ON c.id = m.chat_id \
         WHERE {VISIBLE} AND m.deleted_at IS NULL"
    ));
    sql.and("m.journal_date >=", args.from.clone());
    sql.and("m.journal_date <=", args.to.clone());
    if let Some(chat) = &args.chat_id {
        sql.and("m.chat_id =", chat.clone());
    }
    if let Some(cursor) = &args.cursor {
        // A day and a range are read in chronological order: further means strictly later than the cursor position.
        keyset(&mut sql, cursor, ">");
    }
    sql.text.push_str(&format!(
        " ORDER BY m.journal_date, m.created_at, m.id LIMIT {}",
        ENTRIES_PAGE + 1
    ));
    let mut found = sql.rows(conn)?;
    let has_more = found.len() > ENTRIES_PAGE;
    found.truncate(ENTRIES_PAGE);
    page(found, has_more, Vec::new(), ctx)
}

fn on_this_day(
    conn: &Connection,
    date: Option<&str>,
    ctx: &ReadCtx<'_>,
) -> Result<ReadOutput, ReadFail> {
    let Some(day) = month_day(date.unwrap_or(&ctx.today)) else {
        return Err(StoreError::Internal("the \"on this day\" date was not parsed".into()).into());
    };
    // The dates of past years are computed by the domain (one function shared with the "On this day" card; February 29 matches only February 29): the query goes by the
    // date index, not by `substr` over the whole table. The start is the year of the oldest entry visible to the agent.
    let oldest: Option<String> = conn
        .query_row(
            &format!(
                "SELECT MIN(m.journal_date) FROM message m JOIN chat c ON c.id = m.chat_id \
                 WHERE {VISIBLE} AND m.deleted_at IS NULL"
            ),
            [],
            |row| row.get(0),
        )
        .map_err(read_err)?;
    let oldest_year = oldest.as_deref().and_then(year_of).unwrap_or(day.year);
    let dates: Vec<String> = past_dates(date.unwrap_or(&ctx.today), oldest_year)
        .iter()
        .map(|date| date.format("%Y-%m-%d").to_string())
        .collect();
    if dates.is_empty() {
        return page(Vec::new(), false, Vec::new(), ctx);
    }
    let mut sql = Sql::new(format!(
        "SELECT {VIEW_COLUMNS}, c.title FROM message m JOIN chat c ON c.id = m.chat_id \
         WHERE {VISIBLE} AND m.deleted_at IS NULL"
    ));
    sql.and_in("m.journal_date", dates);
    sql.text.push_str(&format!(
        " ORDER BY m.journal_date DESC, m.created_at DESC, m.id DESC LIMIT {}",
        ENTRIES_PAGE + 1
    ));
    let mut found = sql.rows(conn)?;
    let has_more = found.len() > ENTRIES_PAGE;
    found.truncate(ENTRIES_PAGE);
    page(found, has_more, Vec::new(), ctx)
}

fn get_entry(conn: &Connection, id: &str, ctx: &ReadCtx<'_>) -> Result<ReadOutput, ReadFail> {
    let mut sql = Sql::new(format!(
        "SELECT {VIEW_COLUMNS}, c.title FROM message m JOIN chat c ON c.id = m.chat_id \
         WHERE {VISIBLE} AND m.deleted_at IS NULL"
    ));
    sql.and("m.id =", id.to_string());
    let Some((view, title)) = sql.rows(conn)?.into_iter().next() else {
        return Err(ReadFail::NotFound);
    };
    // In full, without trimming the list; an entry that is too long is cut by the common reply limit.
    let (entries, _) = fit_entries(vec![to_entry(
        &view,
        &chat_title(&view, &title, ctx.inbox_title),
    )]);
    let ids = entries.iter().map(|entry| entry.id.clone()).collect();
    let text = json!({ "entry": entries.first() }).to_string();
    Ok(ReadOutput { text, ids })
}
