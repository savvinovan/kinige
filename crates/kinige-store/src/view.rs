//! An entry for the interface: the same columns and parsing for the feed, the capture window, the trash and replies to actions.

use crate::StoreError;
use crate::attachment::{attachments_column, parse_attachments};
use crate::error::read_err;
use kinige_domain::MessageView;
use kinige_domain::groups::SCHEMA_VERSION;
use kinige_domain::reply::{ReplyTarget, reply_to};
use kinige_domain::tags::parse_tags;
use rusqlite::{Connection, OptionalExtension, Row};

/// The old entry that was replied to, as one JSON value (chat, day, start of the text - 1000 characters: the quote takes
/// 120 characters of plain text from them, and stripping the markup eats some - and the trash flag). The table in the
/// query is called `r`.
macro_rules! reply_target_object {
    () => {
        "json_object('chat', r.chat_id, 'date', r.journal_date, 'body', substr(r.body_md, 1, 1000), \
         'trash', r.deleted_at IS NOT NULL, \
         'att', (SELECT json_group_array(json_object('k', a.kind, 'n', a.filename)) FROM attachment a \
                 WHERE a.message_id = r.id AND a.deleted_at IS NULL))"
    };
}

/// The `message` columns that [`MessageView`] is built from; the table in the query is called `m`. The last column is
/// the entry that was replied to: the same as `LEFT JOIN message r ON r.id = m.reply_to_id` (there is no foreign key,
/// the entry may be missing), but as a subquery, so that all queries with these columns get it without editing their
/// `FROM`. Without a reply the subquery is not executed.
pub(crate) const VIEW_COLUMNS: &str = concat!(
    "m.id, m.chat_id, m.body_md, m.journal_date, m.created_at, m.created_tz, \
     m.is_task, m.task_done_at, m.favorite_at, m.schema_version, m.origin, m.reply_to_id, \
     (CASE WHEN m.reply_to_id IS NULL THEN NULL ELSE (SELECT ",
    reply_target_object!(),
    " FROM message r WHERE r.id = m.reply_to_id) END), ",
    attachments_column!()
);
/// How many columns [`VIEW_COLUMNS`] has: the query's extra columns start from this index.
pub(crate) const VIEW_LEN: usize = 14;

/// An entry from a row with the [`VIEW_COLUMNS`] columns, starting at `first`. Tags are parsed from the text, not read
/// from `message_tag`: the reply does not depend on the derived table. An entry is "read only" if any of its groups
/// is newer than the supported one (`schema_version` is the maximum `sv` over the groups).
pub(crate) fn view_from_row_at(row: &Row<'_>, first: usize) -> rusqlite::Result<MessageView> {
    let body_md: String = row.get(first + 2)?;
    let journal_date: String = row.get(first + 3)?;
    let reply_id: Option<String> = row.get(first + 11)?;
    let target_json: Option<String> = row.get(first + 12)?;
    let attachments = parse_attachments(row.get(first + 13)?);
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
        attachments,
    })
}

pub(crate) fn view_from_row(row: &Row<'_>) -> rusqlite::Result<MessageView> {
    view_from_row_at(row, 0)
}

/// An entry by id (including one from the trash); no such entry gives `MessageNotFound`.
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

/// The quote for the target entry `target_id` on the day `reply_date`: the line above the input field of a reply draft.
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

/// Parsing of the [`VIEW_COLUMNS`] value; a corrupted value means "no entry".
fn parse_target(json: &str) -> Option<ReplyTarget> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    let text = |key: &str| value.get(key)?.as_str().map(str::to_string);
    Some(ReplyTarget {
        chat_id: text("chat")?,
        journal_date: text("date")?,
        body_md: text("body")?,
        in_trash: value.get("trash")?.as_i64()? != 0,
        attachments: attachment_summary(value.get("att")),
    })
}

/// The `att` value of the reply target: an array of `{k, n}`, which SQLite hands over as text when it passes a subquery. A damaged
/// value means no attachments.
fn attachment_summary(value: Option<&serde_json::Value>) -> kinige_domain::AttachmentSummary {
    let parsed: Option<serde_json::Value> = match value {
        Some(serde_json::Value::String(text)) => serde_json::from_str(text).ok(),
        Some(other) => Some(other.clone()),
        None => None,
    };
    let items = parsed
        .as_ref()
        .and_then(serde_json::Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    kinige_domain::attachment::summarize(items.iter().filter_map(|item| {
        let kind = kinige_domain::AttachmentKind::parse(item.get("k")?.as_str()?)?;
        Some((kind, item.get("n")?.as_str()?))
    }))
}
