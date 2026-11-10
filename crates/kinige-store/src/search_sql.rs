//! The one statement and the one read wrapper behind both searches over entries: the window's (`search_query`) and the AI
//! agent's (`agent_read`). FTS5 `MATCH` over the ADR-0008 index, then `message` and `chat`, the filters, the feed order from
//! new to old and a keyset cursor. The trash never appears. What differs between the two is passed in: which chats count
//! (`chat_condition`) and which extra columns the caller reads; each keeps its own result shape.
//!
//! The index is a derived table: a read that fails on its corruption is healed by a rebuild and retried once
//! ([`Store::read_healing`]). If the error remains after the rebuild, the database itself is corrupt and the signal goes out
//! as usual.

use crate::view::VIEW_COLUMNS;
use crate::{Store, StoreError};
use rusqlite::types::Value;
use rusqlite::{Connection, Row, params_from_iter};

/// A search statement for one page.
pub(crate) struct SearchSql<'a> {
    /// A condition on the joined chat `c`: the window drops deleted chats, the agent adds the visible set (ADR-0009).
    pub(crate) chat_condition: &'a str,
    /// Columns read after `VIEW_COLUMNS`, in terms of `m` and `c`. They may use only the `leading` parameters.
    pub(crate) extra_columns: &'a str,
    /// Parameters of `extra_columns`, numbered from `?1`.
    pub(crate) leading: Vec<Value>,
    /// The FTS5 query (`kinige_domain::search::fts_query`).
    pub(crate) query: String,
    pub(crate) filters: Filters<'a>,
    /// The position of the last entry of the previous page: the page is strictly older.
    pub(crate) before: Option<Position<'a>>,
    pub(crate) limit: usize,
}

/// Which entries to keep.
#[derive(Default)]
pub(crate) struct Filters<'a> {
    pub(crate) favorite: bool,
    pub(crate) tasks: bool,
    pub(crate) chat_id: Option<&'a str>,
    /// From this `journal_date`, inclusive.
    pub(crate) from: Option<&'a str>,
    /// Up to this `journal_date`, inclusive.
    pub(crate) to: Option<&'a str>,
}

/// A place in the feed order `(journal_date, created_at, id)`.
pub(crate) struct Position<'a> {
    pub(crate) journal_date: &'a str,
    pub(crate) created_at: i64,
    pub(crate) id: &'a str,
}

impl SearchSql<'_> {
    /// The rows of one page (at most `limit`) and whether there are more after it.
    pub(crate) fn run<R>(
        &self,
        conn: &Connection,
        map: impl FnMut(&Row<'_>) -> rusqlite::Result<R>,
    ) -> rusqlite::Result<(Vec<R>, bool)> {
        let (text, params) = self.statement();
        let mut statement = conn.prepare(&text)?;
        let mut rows = statement
            .query_map(params_from_iter(params), map)?
            .collect::<Result<Vec<_>, _>>()?;
        let has_more = rows.len() > self.limit;
        rows.truncate(self.limit);
        Ok((rows, has_more))
    }

    fn statement(&self) -> (String, Vec<Value>) {
        let mut params = self.leading.clone();
        let mut text = format!(
            "SELECT {VIEW_COLUMNS}, {} FROM search_fts f \
             JOIN search_map mp ON mp.rowid = f.rowid \
             JOIN message m ON m.id = mp.message_id \
             JOIN chat c ON c.id = m.chat_id \
             WHERE {} AND m.deleted_at IS NULL",
            self.extra_columns, self.chat_condition
        );
        text.push_str(&format!(
            " AND search_fts MATCH ?{}",
            bind(&mut params, self.query.clone())
        ));
        if self.filters.favorite {
            text.push_str(" AND m.favorite_at IS NOT NULL");
        }
        if self.filters.tasks {
            text.push_str(" AND m.is_task = 1");
        }
        for (condition, value) in [
            ("m.chat_id =", self.filters.chat_id),
            ("m.journal_date >=", self.filters.from),
            ("m.journal_date <=", self.filters.to),
        ] {
            if let Some(value) = value {
                text.push_str(&format!(
                    " AND {condition} ?{}",
                    bind(&mut params, value.to_string())
                ));
            }
        }
        if let Some(before) = &self.before {
            let date = bind(&mut params, before.journal_date.to_string());
            let at = bind(&mut params, before.created_at);
            let id = bind(&mut params, before.id.to_string());
            text.push_str(&format!(
                " AND (m.journal_date, m.created_at, m.id) < (?{date}, ?{at}, ?{id})"
            ));
        }
        // One more than the page, to know whether there is a next one.
        let limit = i64::try_from(self.limit)
            .unwrap_or(i64::MAX)
            .saturating_add(1);
        text.push_str(&format!(
            " ORDER BY m.journal_date DESC, m.created_at DESC, m.id DESC LIMIT ?{}",
            bind(&mut params, limit)
        ));
        (text, params)
    }
}

/// A failure of a read that may be a damaged search index or a corrupt database. Implemented by `StoreError` here and by the
/// agent's `ReadFail` in `agent_read`.
pub(crate) trait ReadError: From<StoreError> {
    fn store_error(&self) -> Option<&StoreError>;
}

impl ReadError for StoreError {
    fn store_error(&self) -> Option<&StoreError> {
        Some(self)
    }
}

impl Store {
    /// Runs `read` (a read on the window's or the agent's connection). If it fails on a damaged search index, the index
    /// is rebuilt from the entries (once, even when several reads failed together) and `read` runs again. `Corrupt` in the
    /// end result takes the usual corruption path.
    pub(crate) fn read_healing<T, E: ReadError>(
        &self,
        mut read: impl FnMut() -> Result<T, E>,
    ) -> Result<T, E> {
        let seen = self.search_generation();
        let result = match read() {
            Err(error) if matches!(error.store_error(), Some(StoreError::SearchIndex(_))) => {
                match self.heal_search_index(seen) {
                    Ok(()) => read(),
                    Err(healing) => Err(E::from(healing)),
                }
            }
            other => other,
        };
        if let Err(error) = &result
            && matches!(error.store_error(), Some(StoreError::Corrupt(_)))
        {
            self.report_corruption();
        }
        result
    }
}

/// Adds a parameter and returns its number.
fn bind(params: &mut Vec<Value>, value: impl Into<Value>) -> usize {
    params.push(value.into());
    params.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The parameters follow the leading ones in the order the conditions are written, and the limit is the last, one above the page.
    #[test]
    fn parameters_are_numbered_after_the_leading_ones_in_the_order_of_the_text() {
        let (text, params) = sql(
            Filters {
                favorite: true,
                tasks: true,
                chat_id: Some("chat"),
                from: Some("2026-01-01"),
                to: Some("2026-12-31"),
            },
            Some(Position {
                journal_date: "2026-06-01",
                created_at: 7,
                id: "id",
            }),
        )
        .statement();
        for part in [
            "WHERE c.deleted_at IS NULL AND m.deleted_at IS NULL AND search_fts MATCH ?2",
            " AND m.favorite_at IS NOT NULL AND m.is_task = 1",
            " AND m.chat_id = ?3 AND m.journal_date >= ?4 AND m.journal_date <= ?5",
            " AND (m.journal_date, m.created_at, m.id) < (?6, ?7, ?8)",
            " ORDER BY m.journal_date DESC, m.created_at DESC, m.id DESC LIMIT ?9",
        ] {
            assert!(text.contains(part), "{part} is missing from {text}");
        }
        assert_eq!(params.len(), 9);
        assert_eq!(params.first(), Some(&Value::from("inbox".to_string())));
        assert_eq!(params.get(1), Some(&Value::from("kni*".to_string())));
        assert_eq!(params.get(6), Some(&Value::from(7_i64)));
        assert_eq!(params.last(), Some(&Value::from(51_i64)));
    }

    #[test]
    fn no_filters_and_no_cursor_leave_only_the_match_and_the_limit() {
        let (text, params) = sql(Filters::default(), None).statement();
        assert!(text.contains("search_fts MATCH ?2 ORDER BY"), "{text}");
        // (The columns read include both, so the conditions are told apart by their `AND`.)
        assert!(
            !text.contains(" AND m.favorite_at") && !text.contains(" AND m.is_task"),
            "{text}"
        );
        assert_eq!(params.len(), 3);
    }

    fn sql<'a>(filters: Filters<'a>, before: Option<Position<'a>>) -> SearchSql<'a> {
        SearchSql {
            chat_condition: "c.deleted_at IS NULL",
            extra_columns: "c.title, c.id = ?1",
            leading: vec!["inbox".to_string().into()],
            query: "kni*".into(),
            filters,
            before,
            limit: 50,
        }
    }
}
