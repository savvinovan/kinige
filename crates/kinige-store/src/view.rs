//! Запись для интерфейса: одни и те же колонки и разбор для ленты, окна захвата, корзины и ответов на действия.

use crate::StoreError;
use crate::error::read_err;
use kinige_domain::MessageView;
use kinige_domain::groups::SCHEMA_VERSION;
use kinige_domain::tags::parse_tags;
use rusqlite::{Connection, Row};

/// Колонки `message`, из которых строится [`MessageView`]; таблица в запросе называется `m`.
pub(crate) const VIEW_COLUMNS: &str = "m.id, m.chat_id, m.body_md, m.journal_date, m.created_at, m.created_tz, \
     m.is_task, m.task_done_at, m.favorite_at, m.schema_version, m.origin";
/// Сколько колонок в [`VIEW_COLUMNS`]: дополнительные колонки запроса идут с этого номера.
pub(crate) const VIEW_LEN: usize = 11;

/// Запись из строки с колонками [`VIEW_COLUMNS`], начиная с `first`. Теги разбираются из текста, а не читаются из
/// `message_tag`: ответ не зависит от производной таблицы. Запись «только для чтения», если какая-то её группа новее
/// поддерживаемой (`schema_version` — максимум `sv` по группам).
pub(crate) fn view_from_row_at(row: &Row<'_>, first: usize) -> rusqlite::Result<MessageView> {
    let body_md: String = row.get(first + 2)?;
    Ok(MessageView {
        id: row.get(first)?,
        chat_id: row.get(first + 1)?,
        journal_date: row.get(first + 3)?,
        created_at: row.get(first + 4)?,
        created_tz: row.get(first + 5)?,
        is_task: row.get::<_, i64>(first + 6)? != 0,
        task_done_at: row.get(first + 7)?,
        favorite_at: row.get(first + 8)?,
        read_only: row.get::<_, i64>(first + 9)? > i64::from(SCHEMA_VERSION),
        origin: row.get(first + 10)?,
        tags: parse_tags(&body_md),
        body_md,
    })
}

pub(crate) fn view_from_row(row: &Row<'_>) -> rusqlite::Result<MessageView> {
    view_from_row_at(row, 0)
}

/// Запись по id (в том числе из корзины); нет записи — `MessageNotFound`.
pub(crate) fn load_view(conn: &Connection, id: &str) -> Result<MessageView, StoreError> {
    match conn.query_row(
        &format!("SELECT {VIEW_COLUMNS} FROM message m WHERE m.id = ?1"),
        [id],
        view_from_row,
    ) {
        Ok(view) => Ok(view),
        Err(rusqlite::Error::QueryReturnedNoRows) => Err(StoreError::MessageNotFound),
        Err(error) => Err(read_err(error)),
    }
}
