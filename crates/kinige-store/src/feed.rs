//! Чтение: чаты сайдбара и страница ленты по keyset-курсору (ТЗ §4.1, §6).

use crate::error::read_err;
use crate::view::{VIEW_COLUMNS, view_from_row};
use crate::{Now, Store, StoreError};
use kinige_domain::groups::INBOX_ID;
use kinige_domain::journal::{DAY_BOUNDARY_MINUTES, journal_date};
use kinige_domain::{ChatList, ChatSummary, FeedCursor, MessagePage, MessageView};
use rusqlite::Connection;

/// Сколько записей в странице, если вызывающий не просил иначе, и наибольшая страница.
pub const DEFAULT_PAGE: usize = 200;
pub const MAX_PAGE: usize = 500;

/// Порядок чатов в сайдбаре: закреплённые (по времени закрепления), затем остальные по `sort_key`, затем архив.
/// Архивный закреплённый чат остаётся в архиве. Равные ключи решает `id` (ADR-0001, приложение §A.0).
fn section_rank(chat: &ChatSummary) -> u8 {
    if chat.archived_at.is_some() {
        2
    } else if chat.pinned_at.is_some() {
        0
    } else {
        1
    }
}

/// Чаты со счётчиками, в порядке показа, и размер корзины.
pub(crate) fn chat_list(conn: &Connection) -> Result<ChatList, StoreError> {
    let mut statement = conn
        .prepare(
            "SELECT c.id, c.title, c.icon, c.id = ?1, \
             (SELECT COUNT(*) FROM message m WHERE m.chat_id = c.id AND m.deleted_at IS NULL), \
             c.pinned_at, c.archived_at, c.sort_key \
             FROM chat c WHERE c.deleted_at IS NULL",
        )
        .map_err(read_err)?;
    // Inbox определяется по id, а не по флагу `is_inbox` (ADR-0001 §3.6).
    let rows = statement
        .query_map([INBOX_ID], |row| {
            Ok((
                ChatSummary {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    icon: row.get(2)?,
                    is_inbox: row.get(3)?,
                    count: row.get(4)?,
                    pinned_at: row.get(5)?,
                    archived_at: row.get(6)?,
                },
                row.get::<_, String>(7)?,
            ))
        })
        .map_err(read_err)?;
    let mut chats = rows.collect::<Result<Vec<_>, _>>().map_err(read_err)?;
    chats.sort_by(|(a, a_key), (b, b_key)| {
        section_rank(a)
            .cmp(&section_rank(b))
            .then_with(|| {
                if section_rank(a) == 0 {
                    a.pinned_at.cmp(&b.pinned_at)
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .then_with(|| a_key.cmp(b_key))
            .then_with(|| a.id.cmp(&b.id))
    });
    let trash_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM message WHERE deleted_at IS NOT NULL",
            [],
            |row| row.get(0),
        )
        .map_err(read_err)?;
    Ok(ChatList {
        chats: chats.into_iter().map(|(chat, _)| chat).collect(),
        trash_count,
    })
}

impl Store {
    /// Чаты сайдбара со счётчиком записей (все, кроме корзины) и размер корзины.
    pub fn sidebar(&self) -> Result<ChatList, StoreError> {
        self.read(chat_list)
    }

    /// Чаты сайдбара в порядке показа.
    pub fn list_chats(&self) -> Result<Vec<ChatSummary>, StoreError> {
        Ok(self.sidebar()?.chats)
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
        self.list_messages_tagged(chat_id, before, limit, None, now)
    }

    /// То же, что [`Store::list_messages`], но `tag` оставляет только записи с этим тегом (нижний регистр, без `#`).
    pub fn list_messages_tagged(
        &self,
        chat_id: &str,
        before: Option<&FeedCursor>,
        limit: usize,
        tag: Option<&str>,
        now: &Now,
    ) -> Result<MessagePage, StoreError> {
        let limit = limit.clamp(1, MAX_PAGE);
        let today = journal_date(now.ms, now.utc_offset_secs, DAY_BOUNDARY_MINUTES);
        self.read(|conn| {
            let fetch = limit as i64 + 1; // одна лишняя запись говорит, что есть более старые
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
            if let Some(cursor) = before {
                params.push(cursor.journal_date.clone().into());
                params.push(cursor.created_at.into());
                params.push(cursor.id.clone().into());
                let n = params.len();
                sql.push_str(&format!(
                    " AND (m.journal_date, m.created_at, m.id) < (?{}, ?{}, ?{})",
                    n - 2,
                    n - 1,
                    n
                ));
            }
            params.push(fetch.into());
            sql.push_str(&format!(
                " ORDER BY m.journal_date DESC, m.created_at DESC, m.id DESC LIMIT ?{}",
                params.len()
            ));
            let mut statement = conn.prepare(&sql).map_err(read_err)?;
            let rows = statement
                .query_map(rusqlite::params_from_iter(params), view_from_row)
                .map_err(read_err)?;
            let mut messages: Vec<MessageView> =
                rows.collect::<Result<Vec<_>, _>>().map_err(read_err)?;
            let has_more = messages.len() > limit;
            messages.truncate(limit);
            messages.reverse();
            let older = if has_more {
                messages.first().map(|m| FeedCursor {
                    journal_date: m.journal_date.clone(),
                    created_at: m.created_at,
                    id: m.id.clone(),
                })
            } else {
                None
            };
            Ok(MessagePage {
                messages,
                older,
                today,
            })
        })
    }
}
