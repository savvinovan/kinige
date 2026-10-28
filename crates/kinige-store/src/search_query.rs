//! Запрос поиска (слайс 4, задача 4.9): MATCH по индексу ADR-0008, фильтры, порядок ленты по убыванию и keyset-курсор.
//! Корзина, удалённые чаты и ревизии в результаты не попадают. Форма запроса — «A» из отчёта S5: от FTS к `message`,
//! сортировка найденного (p95 на 100 000 записей — десятки миллисекунд).

use crate::error::read_err;
use crate::feed::{MAX_PAGE, chat_list};
use crate::view::{VIEW_COLUMNS, view_from_row_at};
use crate::{Store, StoreError};
use kinige_domain::groups::INBOX_ID;
use kinige_domain::journal::is_valid_journal_date;
use kinige_domain::search::{fts_query, highlight_terms, snippet, title_matches};
use kinige_domain::{FeedCursor, SearchFilters, SearchHit, SearchPage};
use rusqlite::Connection;

/// Длина фрагмента в знаках.
const SNIPPET_CHARS: usize = 200;

impl Store {
    /// Поиск по записям. `before` — позиция последней записи предыдущей страницы. Пустой запрос (нет ни одного слова) —
    /// пустая страница без ошибки. Чаты, чьё название совпало с запросом, приходят только на первой странице.
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
        let run = |conn: &Connection| -> Result<SearchPage, StoreError> {
            let mut sql = format!(
                "SELECT {VIEW_COLUMNS}, c.title, c.icon, c.id = ?1 \
                 FROM search_fts f \
                 JOIN search_map mp ON mp.rowid = f.rowid \
                 JOIN message m ON m.id = mp.message_id \
                 JOIN chat c ON c.id = m.chat_id \
                 WHERE search_fts MATCH ?2 AND m.deleted_at IS NULL AND c.deleted_at IS NULL"
            );
            let mut params: Vec<rusqlite::types::Value> =
                vec![INBOX_ID.to_string().into(), query.clone().into()];
            if filters.favorite {
                sql.push_str(" AND m.favorite_at IS NOT NULL");
            }
            if filters.tasks {
                sql.push_str(" AND m.is_task = 1");
            }
            for (condition, value) in [
                ("m.chat_id =", &filters.chat_id),
                ("m.journal_date >=", &filters.from),
                ("m.journal_date <=", &filters.to),
            ] {
                if let Some(value) = value {
                    params.push(value.clone().into());
                    sql.push_str(&format!(" AND {condition} ?{}", params.len()));
                }
            }
            if let Some(cursor) = before {
                let at = params.len();
                params.push(cursor.journal_date.clone().into());
                params.push(cursor.created_at.into());
                params.push(cursor.id.clone().into());
                sql.push_str(&format!(
                    " AND (m.journal_date, m.created_at, m.id) < (?{}, ?{}, ?{})",
                    at + 1,
                    at + 2,
                    at + 3
                ));
            }
            params.push((limit as i64 + 1).into());
            sql.push_str(&format!(
                " ORDER BY m.journal_date DESC, m.created_at DESC, m.id DESC LIMIT ?{}",
                params.len()
            ));
            let mut statement = conn.prepare(&sql).map_err(read_err)?;
            let rows = statement
                .query_map(rusqlite::params_from_iter(params), |row| {
                    Ok((
                        view_from_row_at(row, 0)?,
                        row.get::<_, String>(10)?,
                        row.get::<_, Option<String>>(11)?,
                        row.get::<_, bool>(12)?,
                    ))
                })
                .map_err(read_err)?;
            let mut found = rows.collect::<Result<Vec<_>, _>>().map_err(read_err)?;
            let has_more = found.len() > limit;
            found.truncate(limit);
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
                .map(|(message, chat_title, chat_icon, chat_is_inbox)| {
                    let terms = highlight_terms(&message.body_md, input);
                    SearchHit {
                        snippet: snippet(&message.body_md, &terms, SNIPPET_CHARS),
                        terms,
                        message,
                        chat_title,
                        chat_icon,
                        chat_is_inbox,
                    }
                })
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
        // Индекс поиска — производная таблица: его повреждение лечится перестройкой из записей, а не экраном «база
        // повреждена». Если после перестройки ошибка осталась, повреждена сама база: сигнал уходит как обычно.
        let seen = self.search_generation();
        let result = match self.read_unnoted(run) {
            Err(StoreError::SearchIndex(_)) => match self.heal_search_index(seen) {
                Ok(()) => self.read_unnoted(run),
                Err(error) => Err(error),
            },
            other => other,
        };
        self.note_if_corrupt(&result);
        result
    }
}
