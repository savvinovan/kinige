//! Чтение: чаты сайдбара и страница ленты по keyset-курсору (ТЗ §4.1, §6).

use crate::{Now, Store, StoreError};
use kinige_domain::journal::{DAY_BOUNDARY_MINUTES, journal_date};
use kinige_domain::{ChatSummary, FeedCursor, MessagePage, MessageView};
use rusqlite::Row;

/// Сколько записей в странице, если вызывающий не просил иначе, и наибольшая страница.
pub const DEFAULT_PAGE: usize = 200;
pub const MAX_PAGE: usize = 500;

fn read_err(error: rusqlite::Error) -> StoreError {
    StoreError::Read(error.to_string())
}

fn message_from_row(row: &Row<'_>) -> rusqlite::Result<MessageView> {
    Ok(MessageView {
        id: row.get(0)?,
        chat_id: row.get(1)?,
        body_md: row.get(2)?,
        journal_date: row.get(3)?,
        created_at: row.get(4)?,
        created_tz: row.get(5)?,
    })
}

impl Store {
    /// Чаты сайдбара со счётчиком записей (все, кроме корзины).
    pub fn list_chats(&self) -> Result<Vec<ChatSummary>, StoreError> {
        self.read(|conn| {
            let mut statement = conn
                .prepare(
                    "SELECT c.id, c.title, c.icon, c.is_inbox, \
                     (SELECT COUNT(*) FROM message m WHERE m.chat_id = c.id AND m.deleted_at IS NULL) \
                     FROM chat c WHERE c.deleted_at IS NULL AND c.archived_at IS NULL ORDER BY c.sort_key, c.id",
                )
                .map_err(read_err)?;
            let rows = statement
                .query_map([], |row| {
                    Ok(ChatSummary {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        icon: row.get(2)?,
                        is_inbox: row.get(3)?,
                        count: row.get(4)?,
                    })
                })
                .map_err(read_err)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(read_err)
        })
    }

    /// Страница ленты: самая новая — без курсора, более старая — после `before`. Записи отдаются по возрастанию
    /// `(journal_date, created_at, id)`; корзина не показывается.
    pub fn list_messages(
        &self,
        chat_id: &str,
        before: Option<&FeedCursor>,
        limit: usize,
        now: &Now,
    ) -> Result<MessagePage, StoreError> {
        let limit = limit.clamp(1, MAX_PAGE);
        let today = journal_date(now.ms, now.utc_offset_secs, DAY_BOUNDARY_MINUTES);
        self.read(|conn| {
            let fetch = limit as i64 + 1; // одна лишняя запись говорит, что есть более старые
            let mut messages: Vec<MessageView> = match before {
                None => {
                    let mut statement = conn
                        .prepare(
                            "SELECT id, chat_id, body_md, journal_date, created_at, created_tz FROM message \
                             WHERE chat_id = ?1 AND deleted_at IS NULL \
                             ORDER BY journal_date DESC, created_at DESC, id DESC LIMIT ?2",
                        )
                        .map_err(read_err)?;
                    let rows = statement.query_map(rusqlite::params![chat_id, fetch], message_from_row).map_err(read_err)?;
                    rows.collect::<Result<Vec<_>, _>>().map_err(read_err)?
                }
                Some(cursor) => {
                    let mut statement = conn
                        .prepare(
                            "SELECT id, chat_id, body_md, journal_date, created_at, created_tz FROM message \
                             WHERE chat_id = ?1 AND deleted_at IS NULL AND (journal_date, created_at, id) < (?2, ?3, ?4) \
                             ORDER BY journal_date DESC, created_at DESC, id DESC LIMIT ?5",
                        )
                        .map_err(read_err)?;
                    let rows = statement
                        .query_map(
                            rusqlite::params![chat_id, cursor.journal_date, cursor.created_at, cursor.id, fetch],
                            message_from_row,
                        )
                        .map_err(read_err)?;
                    rows.collect::<Result<Vec<_>, _>>().map_err(read_err)?
                }
            };
            let has_more = messages.len() > limit;
            messages.truncate(limit);
            messages.reverse();
            let older = if has_more {
                messages.first().map(|m| FeedCursor { journal_date: m.journal_date.clone(), created_at: m.created_at, id: m.id.clone() })
            } else {
                None
            };
            Ok(MessagePage { messages, older, today })
        })
    }
}
