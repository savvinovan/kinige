//! Entry tags (`message_tag`, slice 3): a local derivative of `body_md`, updated in the same transaction as the entry
//! and recomputed on migration. The parsing rules are in `kinige_domain::tags`.

use kinige_domain::tags::parse_tags;
use rusqlite::Connection;

/// Replaces an entry's tags with the tags of its current `body_md`.
pub(crate) fn replace_tags(
    conn: &Connection,
    message_id: &str,
    body_md: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM message_tag WHERE message_id = ?1",
        [message_id],
    )?;
    insert_tags(conn, message_id, body_md)
}

fn insert_tags(conn: &Connection, message_id: &str, body_md: &str) -> rusqlite::Result<()> {
    for tag in parse_tags(body_md) {
        conn.execute(
            "INSERT OR IGNORE INTO message_tag (message_id, tag) VALUES (?1, ?2)",
            (message_id, tag),
        )?;
    }
    Ok(())
}

/// Recomputes the tags of all entries (migration 3): the table is always built from the texts, not the other way round.
pub(crate) fn rebuild_all(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM message_tag", [])?;
    let mut statement = conn.prepare("SELECT id, body_md FROM message")?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let id: String = row.get(0)?;
        let body: String = row.get(1)?;
        insert_tags(conn, &id, &body)?;
    }
    Ok(())
}
