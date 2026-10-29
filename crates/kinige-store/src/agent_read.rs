//! Чтение для агента (ADR-0009): каждый оператор несёт предикат видимого множества чатов (`VISIBLE`), корзина не видна.
//! Список чатов для строки «Чаты» поиска — отдельный оператор с тем же предикатом, а не фильтр в Rust.

use crate::agent::VISIBLE;
use crate::agent_entry::{chat_title, page, to_entry};
use crate::error::read_err;
use crate::view::{VIEW_COLUMNS, VIEW_LEN, view_from_row_at};
use crate::{Now, Store, StoreError};
use kinige_domain::MessageView;
use kinige_domain::agent::{
    AgentCursor, ENTRIES_PAGE, EntriesArgs, SEARCH_PAGE, SearchArgs, ToolArgs,
};
use kinige_domain::agent_reply::{AgentChat, fit_entries, month_day};
use kinige_domain::groups::INBOX_ID;
use kinige_domain::journal::{DAY_BOUNDARY_MINUTES, journal_date};
use kinige_domain::search::{fts_query, title_matches};
use rusqlite::types::Value;
use rusqlite::{Connection, params_from_iter};
use serde_json::json;
use std::panic::{AssertUnwindSafe, catch_unwind};

/// Что известно вызову на время чтения.
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

/// Ответ чтения: текст для агента и id записей, которые уйдут в журнал (только те, что вошли в ответ).
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
    /// Чтение на соединении чтения; паника внутри запроса не отравляет соединение. Повреждение индекса поиска лечится
    /// перестройкой и одним повтором (как в поиске главного окна).
    pub(crate) fn read_with_heal(
        &self,
        ctx: &ReadCtx<'_>,
        args: &ToolArgs,
    ) -> Result<ReadOutput, ReadFail> {
        let run = |conn: &Connection| run_read(conn, args, ctx);
        let seen = self.search_generation();
        let result = match self.with_reader(run) {
            Err(ReadFail::Store(StoreError::SearchIndex(_))) => {
                match self.heal_search_index(seen) {
                    Ok(()) => self.with_reader(run),
                    Err(error) => Err(ReadFail::Store(error)),
                }
            }
            other => other,
        };
        if let Err(ReadFail::Store(StoreError::Corrupt(_))) = &result {
            self.report_corruption();
        }
        result
    }

    fn with_reader<T>(
        &self,
        job: impl FnOnce(&Connection) -> Result<T, ReadFail>,
    ) -> Result<T, ReadFail> {
        // Как в `read_unnoted`: паника внутри чтения (профиль release с размоткой стека) ловится здесь, вызов получает
        // `Internal`; если замок всё же отравлен, он берётся как есть — соединение чтения (`query_only`, без транзакции)
        // после размотки согласовано. Иначе одна паника ломала бы все чтения агента до перезапуска.
        let conn = self
            .inner
            .reader
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        catch_unwind(AssertUnwindSafe(|| job(&conn))).unwrap_or_else(|_| {
            log::error!("паника в чтении агента, вызов отменён");
            Err(ReadFail::Store(StoreError::Internal(
                "паника в запросе агента".into(),
            )))
        })
    }

    /// Только для тестов: паника внутри `query_map` на соединении чтения агента. Следующие чтения агента должны работать.
    #[doc(hidden)]
    pub fn panic_in_agent_read_for_test(&self) -> Result<(), StoreError> {
        let result = self.with_reader(|conn| -> Result<(), ReadFail> {
            let mut statement = conn.prepare("SELECT id FROM chat")?;
            let rows = statement.query_map([], |_| -> rusqlite::Result<()> {
                panic!("проверка: паника в запросе агента")
            })?;
            for row in rows {
                row?;
            }
            Ok(())
        });
        match result {
            Ok(()) => Ok(()),
            Err(ReadFail::Store(error)) => Err(error),
            Err(ReadFail::NotFound) => Err(StoreError::Internal("не найдено".into())),
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
        // Инструменты записи сюда не попадают: их ведёт `agent_write`.
        ToolArgs::AddEntry { .. }
        | ToolArgs::AppendToLastEntry { .. }
        | ToolArgs::SetTaskDone { .. } => {
            Err(StoreError::Internal("запись в чтении".into()).into())
        }
    }
}

/// Чат с этим id виден агенту. Закрытый, исключённый из AI, удалённый и несуществующий неразличимы.
pub(crate) fn chat_visible(conn: &Connection, id: &str) -> rusqlite::Result<bool> {
    conn.query_row(
        &format!("SELECT EXISTS(SELECT 1 FROM chat c WHERE c.id = ?1 AND {VISIBLE})"),
        [id],
        |row| row.get(0),
    )
}

/// Запрос с нумерованными параметрами.
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

    /// Добавляет условие `<left> <op> ?N`.
    fn and(&mut self, condition: &str, value: impl Into<Value>) {
        self.params.push(value.into());
        self.text
            .push_str(&format!(" AND {condition} ?{}", self.params.len()));
    }

    fn rows(&self, conn: &Connection) -> Result<Vec<(MessageView, String)>, ReadFail> {
        let mut statement = conn.prepare(&self.text)?;
        let rows = statement.query_map(params_from_iter(&self.params), |row| {
            Ok((view_from_row_at(row, 0)?, row.get::<_, String>(VIEW_LEN)?))
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }
}

fn list_chats(conn: &Connection, ctx: &ReadCtx<'_>) -> Result<ReadOutput, ReadFail> {
    let chats = visible_chats(conn, ctx)?;
    Ok(ReadOutput {
        text: json!({ "chats": chats }).to_string(),
        ids: Vec::new(),
    })
}

/// Видимые чаты: отдельный оператор с предикатом (используется и списком чатов, и строкой «Чаты» поиска).
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
    let mut sql = Sql::new(format!(
        "SELECT {VIEW_COLUMNS}, c.title FROM search_fts f \
         JOIN search_map mp ON mp.rowid = f.rowid \
         JOIN message m ON m.id = mp.message_id \
         JOIN chat c ON c.id = m.chat_id \
         WHERE {VISIBLE} AND m.deleted_at IS NULL"
    ));
    sql.and("search_fts MATCH", query);
    if args.favorites {
        sql.text.push_str(" AND m.favorite_at IS NOT NULL");
    }
    if args.tasks {
        sql.text.push_str(" AND m.is_task = 1");
    }
    for (condition, value) in [
        ("m.chat_id =", &args.chat_id),
        ("m.journal_date >=", &args.from),
        ("m.journal_date <=", &args.to),
    ] {
        if let Some(value) = value {
            sql.and(condition, value.clone());
        }
    }
    if let Some(cursor) = &args.cursor {
        // Поиск идёт от новых к старым: следующая страница — строго раньше позиции курсора.
        keyset(&mut sql, cursor, "<");
    }
    sql.text.push_str(&format!(
        " ORDER BY m.journal_date DESC, m.created_at DESC, m.id DESC LIMIT {}",
        SEARCH_PAGE + 1
    ));
    let mut found = sql.rows(conn)?;
    let has_more = found.len() > SEARCH_PAGE;
    found.truncate(SEARCH_PAGE);
    // Строка «Чаты» — только первая страница, отдельным оператором с предикатом.
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
        // День и диапазон читаются в хронологическом порядке: дальше — строго позже позиции курсора.
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
        return Err(StoreError::Internal("дата «в этот день» не разобрана".into()).into());
    };
    let mut sql = Sql::new(format!(
        "SELECT {VIEW_COLUMNS}, c.title FROM message m JOIN chat c ON c.id = m.chat_id \
         WHERE {VISIBLE} AND m.deleted_at IS NULL"
    ));
    sql.and("substr(m.journal_date, 6, 5) =", day.month_day);
    sql.and(
        "CAST(substr(m.journal_date, 1, 4) AS INTEGER) <",
        i64::from(day.year),
    );
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
    // Целиком, без обрезки списка; слишком длинная запись режется общим пределом ответа.
    let (entries, _) = fit_entries(vec![to_entry(
        &view,
        &chat_title(&view, &title, ctx.inbox_title),
    )]);
    let ids = entries.iter().map(|entry| entry.id.clone()).collect();
    let text = json!({ "entry": entries.first() }).to_string();
    Ok(ReadOutput { text, ids })
}
