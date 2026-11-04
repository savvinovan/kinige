//! "Recent chats" for the tray icon menu (KINIG-47.3). A chat's place is the last live entry in feed order
//! `(journal_date, created_at, id)` (PRD §4.1): one backward step over the existing index `message_feed(chat_id, journal_date,
//! created_at, id)` per chat, instead of `MAX(created_at)` over every entry of every chat (on 100,000 entries that walked the
//! whole table and read each row for `deleted_at`). An entry moved to an old date no longer raises its chat.

use crate::error::read_err;
use crate::{Store, StoreError};
use kinige_domain::groups::INBOX_ID;
use rusqlite::{Connection, OptionalExtension};

/// A chat for the menu bar icon menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecentChat {
    pub id: String,
    /// The title; empty for a chat without a title (ordinary chats never lack one).
    pub title: String,
    pub icon: Option<String>,
}

/// The chats that can be listed: not Inbox (it has its own item), not the archive.
const CANDIDATES_SQL: &str = "SELECT id, title, icon FROM chat \
     WHERE deleted_at IS NULL AND archived_at IS NULL AND id != ?1";

/// The chat's last live entry in feed order. `message_feed` gives the order, so this is one seek to the end of the chat's
/// range and a step back past entries in the trash; `deleted_at` is read from the row of the one entry that is found.
pub(crate) const LAST_ENTRY_SQL: &str = "SELECT journal_date, created_at, id FROM message \
     WHERE chat_id = ?1 AND deleted_at IS NULL \
     ORDER BY journal_date DESC, created_at DESC, id DESC LIMIT 1";

/// Where a chat's last entry stands in feed order; larger is more recent.
type FeedPosition = (String, i64, String);

impl Store {
    /// Up to `limit` last chats by the last entry in feed order, for the icon menu. Without Inbox, without the archive and
    /// without chats that have no live entry. Equal positions are ordered by chat id.
    pub fn recent_chats(&self, limit: usize) -> Result<Vec<RecentChat>, StoreError> {
        self.read(|conn| recent_chats(conn, limit))
    }
}

fn recent_chats(conn: &Connection, limit: usize) -> Result<Vec<RecentChat>, StoreError> {
    let mut candidates = conn.prepare_cached(CANDIDATES_SQL).map_err(read_err)?;
    let chats: Vec<RecentChat> = candidates
        .query_map([INBOX_ID], |row| {
            Ok(RecentChat {
                id: row.get(0)?,
                title: row.get(1)?,
                icon: row.get(2)?,
            })
        })
        .map_err(read_err)?
        .collect::<Result<_, _>>()
        .map_err(read_err)?;
    let mut last = conn.prepare_cached(LAST_ENTRY_SQL).map_err(read_err)?;
    let mut placed: Vec<(FeedPosition, RecentChat)> = Vec::with_capacity(chats.len());
    for chat in chats {
        let position = last
            .query_row([&chat.id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })
            .optional()
            .map_err(read_err)?;
        if let Some(position) = position {
            placed.push((position, chat));
        }
    }
    placed.sort_by(|(a_at, a), (b_at, b)| b_at.cmp(a_at).then_with(|| a.id.cmp(&b.id)));
    placed.truncate(limit);
    Ok(placed.into_iter().map(|(_, chat)| chat).collect())
}
