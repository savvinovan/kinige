//! Страницы ленты вокруг записи и новее (слайс 4, задача 4.10): `Latest`, `Before`, `After`, `Around`. Порядок ленты —
//! `(journal_date, created_at, id)`; страница отдаётся по возрастанию, самая новая запись последней. Корзина не
//! показывается. `older` — курсор следующей более старой страницы (первая запись этой), `newer` — более новой
//! (последняя запись этой); `None` — в ту сторону записей нет.

use crate::error::read_err;
use crate::feed::MAX_PAGE;
use crate::view::{VIEW_COLUMNS, load_view, view_from_row};
use crate::{Now, Store, StoreError};
use kinige_domain::journal::{DAY_BOUNDARY_MINUTES, journal_date};
use kinige_domain::{FeedCursor, MessagePage, MessageView};
use rusqlite::Connection;

/// Откуда брать страницу.
#[derive(Debug, Clone, Copy)]
pub enum PageAt<'a> {
    /// Самые новые записи.
    Latest,
    /// Записи строго старше курсора.
    Before(&'a FeedCursor),
    /// Записи строго новее курсора.
    After(&'a FeedCursor),
    /// Половина страницы до записи, сама запись и остальное после неё.
    Around(&'a str),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    Before,
    After,
}

fn cursor_of(message: &MessageView) -> FeedCursor {
    FeedCursor {
        journal_date: message.journal_date.clone(),
        created_at: message.created_at,
        id: message.id.clone(),
    }
}

/// До `take` записей чата по одну сторону от курсора, ближайшие к нему первыми; `tag` оставляет записи с этим тегом.
fn fetch(
    conn: &Connection,
    chat_id: &str,
    tag: Option<&str>,
    cursor: Option<&FeedCursor>,
    side: Side,
    take: usize,
) -> Result<Vec<MessageView>, StoreError> {
    let mut sql = format!(
        "SELECT {VIEW_COLUMNS} FROM message m WHERE m.chat_id = ?1 AND m.deleted_at IS NULL"
    );
    let mut params: Vec<rusqlite::types::Value> = vec![chat_id.to_string().into()];
    if let Some(tag) = tag {
        params.push(tag.to_lowercase().into());
        sql.push_str(&format!(
            " AND EXISTS (SELECT 1 FROM message_tag t WHERE t.message_id = m.id AND t.tag = ?{})",
            params.len()
        ));
    }
    let (compare, order) = match side {
        Side::Before => ("<", "DESC"),
        Side::After => (">", "ASC"),
    };
    if let Some(cursor) = cursor {
        params.push(cursor.journal_date.clone().into());
        params.push(cursor.created_at.into());
        params.push(cursor.id.clone().into());
        let n = params.len();
        sql.push_str(&format!(
            " AND (m.journal_date, m.created_at, m.id) {compare} (?{}, ?{}, ?{})",
            n - 2,
            n - 1,
            n
        ));
    }
    params.push((take as i64).into());
    sql.push_str(&format!(
        " ORDER BY m.journal_date {order}, m.created_at {order}, m.id {order} LIMIT ?{}",
        params.len()
    ));
    let mut statement = conn.prepare(&sql).map_err(read_err)?;
    let rows = statement
        .query_map(rusqlite::params_from_iter(params), view_from_row)
        .map_err(read_err)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(read_err)
}

/// Есть ли записи по эту сторону от курсора.
fn exists(
    conn: &Connection,
    chat_id: &str,
    tag: Option<&str>,
    cursor: &FeedCursor,
    side: Side,
) -> Result<bool, StoreError> {
    Ok(!fetch(conn, chat_id, tag, Some(cursor), side, 1)?.is_empty())
}

/// Оставляет `keep` ближайших к курсору записей; `true`, если лишняя запись была (значит, дальше есть ещё).
fn trim(found: &mut Vec<MessageView>, keep: usize) -> bool {
    let more = found.len() > keep;
    found.truncate(keep);
    more
}

impl Store {
    /// Страница ленты чата: `limit` записей от `at`. `Around(id)` — запись и соседние с двух сторон; записи нет в этом чате,
    /// она в корзине или не подходит под `tag` — `MessageNotFound`.
    pub fn list_messages_page(
        &self,
        chat_id: &str,
        at: PageAt<'_>,
        limit: usize,
        tag: Option<&str>,
        now: &Now,
    ) -> Result<MessagePage, StoreError> {
        let limit = limit.clamp(1, MAX_PAGE);
        let today = journal_date(now.ms, now.utc_offset_secs, DAY_BOUNDARY_MINUTES);
        self.read(|conn| {
            let mut messages: Vec<MessageView>;
            let (older, newer);
            match at {
                PageAt::Latest | PageAt::Before(_) => {
                    let cursor = match at {
                        PageAt::Before(cursor) => Some(cursor),
                        _ => None,
                    };
                    messages = fetch(conn, chat_id, tag, cursor, Side::Before, limit + 1)?;
                    let more = trim(&mut messages, limit);
                    messages.reverse();
                    older = more.then(|| messages.first().map(cursor_of)).flatten();
                    newer = match (cursor, messages.last()) {
                        (Some(_), Some(last)) => {
                            exists(conn, chat_id, tag, &cursor_of(last), Side::After)?
                                .then(|| cursor_of(last))
                        }
                        _ => None,
                    };
                }
                PageAt::After(cursor) => {
                    messages = fetch(conn, chat_id, tag, Some(cursor), Side::After, limit + 1)?;
                    let more = trim(&mut messages, limit);
                    newer = more.then(|| messages.last().map(cursor_of)).flatten();
                    older = match messages.first() {
                        Some(first) => exists(conn, chat_id, tag, &cursor_of(first), Side::Before)?
                            .then(|| cursor_of(first)),
                        None => None,
                    };
                }
                PageAt::Around(id) => {
                    let target = load_view(conn, id)?;
                    let in_chat = target.chat_id == chat_id;
                    let alive: bool = conn
                        .query_row(
                            "SELECT deleted_at IS NULL FROM message WHERE id = ?1",
                            [id],
                            |row| row.get(0),
                        )
                        .map_err(read_err)?;
                    let tagged = tag
                        .is_none_or(|tag| target.tags.iter().any(|own| *own == tag.to_lowercase()));
                    if !in_chat || !alive || !tagged {
                        return Err(StoreError::MessageNotFound);
                    }
                    let center = cursor_of(&target);
                    // Половина страницы до записи, остальное после. У края ленты страница короче.
                    let half_before = (limit - 1) / 2;
                    let half_after = limit - 1 - half_before;
                    let mut before = fetch(
                        conn,
                        chat_id,
                        tag,
                        Some(&center),
                        Side::Before,
                        half_before + 1,
                    )?;
                    let mut after = fetch(
                        conn,
                        chat_id,
                        tag,
                        Some(&center),
                        Side::After,
                        half_after + 1,
                    )?;
                    let older_exists = trim(&mut before, half_before);
                    let newer_exists = trim(&mut after, half_after);
                    before.reverse();
                    messages = before;
                    messages.push(target);
                    messages.extend(after);
                    older = older_exists
                        .then(|| messages.first().map(cursor_of))
                        .flatten();
                    newer = newer_exists
                        .then(|| messages.last().map(cursor_of))
                        .flatten();
                }
            }
            Ok(MessagePage {
                messages,
                older,
                newer,
                today,
            })
        })
    }
}
