//! `message_ext(src, ext_chat, ext_msg) → message_id` (slice 4, ADR-0001 appendix §C): a local, non-synced index of
//! external import ids, derived from `core.ext`. Deduplication of a repeated import goes through it. The insert is
//! `INSERT OR IGNORE`: two messages with different ids and the same `ext` (one export imported on two devices before
//! sync) do not break the recount.

use rusqlite::Connection;

/// Records an external id; an already taken key stays with the first entry. Returns whether a row was added.
pub(crate) fn insert(
    conn: &Connection,
    src: &str,
    ext_chat: &str,
    ext_msg: &str,
    message_id: &str,
) -> rusqlite::Result<bool> {
    let added = conn
        .prepare_cached(
            "INSERT OR IGNORE INTO message_ext (src, ext_chat, ext_msg, message_id) VALUES (?1, ?2, ?3, ?4)",
        )?
        .execute((src, ext_chat, ext_msg, message_id))?;
    Ok(added == 1)
}

/// Removes an entry's rows (permanent deletion): a repeated import will bring such an entry back as a new one (slice 4
/// specification, decision 4).
pub(crate) fn remove(conn: &Connection, message_id: &str) -> rusqlite::Result<()> {
    conn.prepare_cached("DELETE FROM message_ext WHERE message_id = ?1")?
        .execute([message_id])?;
    Ok(())
}

/// The table anew from `message.ext_json` (migration 4). An unset or corrupted `ext_json` yields no rows.
pub(crate) fn rebuild_all(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM message_ext", [])?;
    conn.execute(
        "INSERT OR IGNORE INTO message_ext (src, ext_chat, ext_msg, message_id) \
         SELECT json_extract(ext_json, '$.src'), json_extract(ext_json, '$.chat'), json_extract(ext_json, '$.msg'), id \
         FROM message \
         WHERE CASE WHEN json_valid(ext_json) \
                    THEN json_type(ext_json, '$.src') = 'text' AND json_type(ext_json, '$.chat') = 'text' \
                         AND json_type(ext_json, '$.msg') = 'text' \
                    ELSE 0 END \
         ORDER BY created_at, id",
        [],
    )?;
    Ok(())
}
