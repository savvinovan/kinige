//! Запись для интерфейса: одни и те же колонки и разбор для ленты, окна захвата, корзины и ответов на действия.

use crate::StoreError;
use crate::error::read_err;
use kinige_domain::MessageView;
use kinige_domain::groups::SCHEMA_VERSION;
use kinige_domain::reply::{ReplyTarget, reply_to};
use kinige_domain::tags::parse_tags;
use rusqlite::{Connection, OptionalExtension, Row};

/// Старая запись, на которую ответили, одним значением JSON (чат, день, начало текста — 1000 знаков: цитата берёт из них
/// 120 знаков простого текста, а разметка при снятии съедает часть — и признак корзины). Таблица в запросе называется `r`.
macro_rules! reply_target_object {
    () => {
        "json_object('chat', r.chat_id, 'date', r.journal_date, 'body', substr(r.body_md, 1, 1000), \
         'trash', r.deleted_at IS NOT NULL)"
    };
}

/// Колонки `message`, из которых строится [`MessageView`]; таблица в запросе называется `m`. Последняя колонка — запись,
/// на которую ответили: то же, что `LEFT JOIN message r ON r.id = m.reply_to_id` (внешнего ключа нет, записи может не быть),
/// но подзапросом, чтобы все запросы с этими колонками получали её без правки своих `FROM`. Без ответа подзапрос не
/// выполняется.
pub(crate) const VIEW_COLUMNS: &str = concat!(
    "m.id, m.chat_id, m.body_md, m.journal_date, m.created_at, m.created_tz, \
     m.is_task, m.task_done_at, m.favorite_at, m.schema_version, m.origin, m.reply_to_id, \
     (CASE WHEN m.reply_to_id IS NULL THEN NULL ELSE (SELECT ",
    reply_target_object!(),
    " FROM message r WHERE r.id = m.reply_to_id) END)"
);
/// Сколько колонок в [`VIEW_COLUMNS`]: дополнительные колонки запроса идут с этого номера.
pub(crate) const VIEW_LEN: usize = 13;

/// Запись из строки с колонками [`VIEW_COLUMNS`], начиная с `first`. Теги разбираются из текста, а не читаются из
/// `message_tag`: ответ не зависит от производной таблицы. Запись «только для чтения», если какая-то её группа новее
/// поддерживаемой (`schema_version` — максимум `sv` по группам).
pub(crate) fn view_from_row_at(row: &Row<'_>, first: usize) -> rusqlite::Result<MessageView> {
    let body_md: String = row.get(first + 2)?;
    let journal_date: String = row.get(first + 3)?;
    let reply_id: Option<String> = row.get(first + 11)?;
    let target_json: Option<String> = row.get(first + 12)?;
    let reply_to = reply_id.map(|id| {
        let target = target_json.as_deref().and_then(parse_target);
        reply_to(&id, &journal_date, target)
    });
    Ok(MessageView {
        id: row.get(first)?,
        chat_id: row.get(first + 1)?,
        journal_date,
        created_at: row.get(first + 4)?,
        created_tz: row.get(first + 5)?,
        is_task: row.get::<_, i64>(first + 6)? != 0,
        task_done_at: row.get(first + 7)?,
        favorite_at: row.get(first + 8)?,
        read_only: row.get::<_, i64>(first + 9)? > i64::from(SCHEMA_VERSION),
        origin: row.get(first + 10)?,
        tags: parse_tags(&body_md),
        body_md,
        reply_to,
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

/// Цитата для записи-адресата `target_id` в день `reply_date`: строка над полем ввода у черновика-ответа.
pub(crate) fn load_reply_to(
    conn: &Connection,
    target_id: &str,
    reply_date: &str,
) -> Result<kinige_domain::ReplyTo, StoreError> {
    let json: Option<String> = conn
        .query_row(
            concat!(
                "SELECT ",
                reply_target_object!(),
                " FROM message r WHERE r.id = ?1"
            ),
            [target_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(read_err)?;
    Ok(reply_to(
        target_id,
        reply_date,
        json.as_deref().and_then(parse_target),
    ))
}

/// Разбор значения [`VIEW_COLUMNS`]; повреждённое значение — «записи нет».
fn parse_target(json: &str) -> Option<ReplyTarget> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    let text = |key: &str| value.get(key)?.as_str().map(str::to_string);
    Some(ReplyTarget {
        chat_id: text("chat")?,
        journal_date: text("date")?,
        body_md: text("body")?,
        in_trash: value.get("trash")?.as_i64()? != 0,
    })
}
