//! Search query (slice 4, task 4.9): MATCH over the ADR-0008 index, filters, feed order descending and a keyset cursor.
//! Trash, deleted chats and revisions never appear in the results. The query shape is "A" from the S5 report: from FTS
//! to `message`, then sorting the matches (p95 on 100,000 entries is tens of milliseconds).

use crate::error::read_err;
use crate::feed::{MAX_PAGE, chat_list};
use crate::search_sql::{Filters, Position, SearchSql};
use crate::view::{VIEW_LEN, view_from_row_at};
use crate::{Store, StoreError};
use kinige_domain::chat_color::resolve_color;
use kinige_domain::groups::INBOX_ID;
use kinige_domain::journal::is_valid_journal_date;
use kinige_domain::search::{fts_query, highlight_terms, snippet, title_matches};
use kinige_domain::{FeedCursor, SearchFilters, SearchHit, SearchPage};
use rusqlite::Connection;

/// Snippet length in characters.
const SNIPPET_CHARS: usize = 200;

impl Store {
    /// Search over entries. `before` is the position of the last entry of the previous page. An empty query (no words)
    /// gives an empty page, not an error. Chats whose name matched the query arrive only on the first page.
    pub fn search(
        &self,
        input: &str,
        filters: &SearchFilters,
        before: Option<&FeedCursor>,
        limit: usize,
    ) -> Result<SearchPage, StoreError> {
        let empty = SearchPage {
            chats: Vec::new(),
            hits: Vec::new(),
            older: None,
        };
        let Some(query) = fts_query(input) else {
            return Ok(empty);
        };
        for date in [&filters.from, &filters.to].into_iter().flatten() {
            if !is_valid_journal_date(date) {
                return Err(StoreError::InvalidDate);
            }
        }
        let limit = limit.clamp(1, MAX_PAGE);
        let sql = SearchSql {
            chat_condition: "c.deleted_at IS NULL",
            extra_columns: "c.title, c.icon, c.id = ?1, c.color",
            leading: vec![INBOX_ID.to_string().into()],
            query,
            filters: Filters {
                favorite: filters.favorite,
                tasks: filters.tasks,
                chat_id: filters.chat_id.as_deref(),
                from: filters.from.as_deref(),
                to: filters.to.as_deref(),
            },
            before: before.map(|cursor| Position {
                journal_date: &cursor.journal_date,
                created_at: cursor.created_at,
                id: &cursor.id,
            }),
            limit,
        };
        let run = |conn: &Connection| -> Result<SearchPage, StoreError> {
            let (found, has_more) = sql
                .run(conn, |row| {
                    Ok((
                        view_from_row_at(row, 0)?,
                        row.get::<_, String>(VIEW_LEN)?,
                        row.get::<_, Option<String>>(VIEW_LEN + 1)?,
                        row.get::<_, bool>(VIEW_LEN + 2)?,
                        row.get::<_, Option<String>>(VIEW_LEN + 3)?,
                    ))
                })
                .map_err(read_err)?;
            let older = has_more
                .then(|| {
                    found.last().map(|(m, ..)| FeedCursor {
                        journal_date: m.journal_date.clone(),
                        created_at: m.created_at,
                        id: m.id.clone(),
                    })
                })
                .flatten();
            let hits = found
                .into_iter()
                .map(
                    |(message, chat_title, chat_icon, chat_is_inbox, stored_color)| {
                        let terms = highlight_terms(&message.body_md, input);
                        SearchHit {
                            snippet: snippet(&message.body_md, &terms, SNIPPET_CHARS),
                            terms,
                            chat_color: resolve_color(&message.chat_id, stored_color.as_deref()),
                            message,
                            chat_title,
                            chat_icon,
                            chat_is_inbox,
                        }
                    },
                )
                .collect();
            let chats = if before.is_none() {
                chat_list(conn)?
                    .chats
                    .into_iter()
                    .filter(|chat| !chat.is_inbox && title_matches(&chat.title, input))
                    .collect()
            } else {
                Vec::new()
            };
            Ok(SearchPage { chats, hits, older })
        };
        // The search index is a derived table: its corruption is fixed by rebuilding it from the entries, not by a
        // "database is corrupted" screen (`read_healing`).
        self.read_healing(|| self.read_unnoted(run))
    }
}
