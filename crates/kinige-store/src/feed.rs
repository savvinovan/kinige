//! Reading: sidebar chats and a feed page by keyset cursor (PRD §4.1, §6).

use crate::error::read_err;
use crate::feed_page::PageAt;
use crate::{Now, Store, StoreError};
use kinige_domain::groups::INBOX_ID;
use kinige_domain::{ChatList, ChatSummary, FeedCursor, MessagePage};
use rusqlite::Connection;

/// Entries per page if the caller did not ask otherwise, and the largest page.
pub const DEFAULT_PAGE: usize = 200;
pub const MAX_PAGE: usize = 500;

/// Sidebar chat order: pinned (by pin time), then the rest by `sort_key`, then the archive.
/// An archived pinned chat stays in the archive. Equal keys are decided by `id` (ADR-0001, appendix §A.0).
fn section_rank(chat: &ChatSummary) -> u8 {
    if chat.archived_at.is_some() {
        2
    } else if chat.pinned_at.is_some() {
        0
    } else {
        1
    }
}

/// Chats with counters, in display order, and the trash size.
///
/// A chat counter is all entries minus the trash. All entries are counted over the covering index `message_feed`, the
/// trash over the partial index `message_deleted`; the condition `deleted_at IS NULL` would read table rows itself (on
/// 100,000 imported entries the counter cost about 150 ms against 5 ms).
pub(crate) fn chat_list(conn: &Connection) -> Result<ChatList, StoreError> {
    let mut statement = conn
        .prepare(
            "SELECT c.id, c.title, c.icon, c.id = ?1, \
             (SELECT COUNT(*) FROM message m WHERE m.chat_id = c.id) \
             - (SELECT COUNT(*) FROM message m INDEXED BY message_deleted \
                WHERE m.chat_id = c.id AND m.deleted_at IS NOT NULL), \
             c.pinned_at, c.archived_at, c.sort_key \
             FROM chat c WHERE c.deleted_at IS NULL",
        )
        .map_err(read_err)?;
    // Inbox is identified by id, not by the `is_inbox` flag (ADR-0001 §3.6).
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
    /// Sidebar chats with an entry counter (all except the trash) and the trash size.
    pub fn sidebar(&self) -> Result<ChatList, StoreError> {
        self.read(chat_list)
    }

    /// Sidebar chats in display order.
    pub fn list_chats(&self) -> Result<Vec<ChatSummary>, StoreError> {
        Ok(self.sidebar()?.chats)
    }

    /// A feed page: the newest one without a cursor, an older one after `before`. Entries come in ascending
    /// `(journal_date, created_at, id)` order; the trash is not shown.
    pub fn list_messages(
        &self,
        chat_id: &str,
        before: Option<&FeedCursor>,
        limit: usize,
        now: &Now,
    ) -> Result<MessagePage, StoreError> {
        self.list_messages_tagged(chat_id, before, limit, None, now)
    }

    /// Same as [`Store::list_messages`], but `tag` keeps only entries with this tag (lower case, without `#`).
    pub fn list_messages_tagged(
        &self,
        chat_id: &str,
        before: Option<&FeedCursor>,
        limit: usize,
        tag: Option<&str>,
        now: &Now,
    ) -> Result<MessagePage, StoreError> {
        let at = match before {
            Some(cursor) => PageAt::Before(cursor),
            None => PageAt::Latest,
        };
        self.list_messages_page(chat_id, at, limit, tag, now)
    }
}
