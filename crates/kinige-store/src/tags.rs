//! Теги записей (`message_tag`, слайс 3): локальная производная от `body_md`, обновляется в той же транзакции, что и
//! запись, и пересчитывается при миграции. Правила разбора — `kinige_domain::tags`.

use kinige_domain::tags::parse_tags;
use rusqlite::Connection;

/// Заменяет теги записи тегами её текущего `body_md`.
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

/// Пересчитывает теги всех записей (миграция 3): таблица всегда строится из текстов, а не наоборот.
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
