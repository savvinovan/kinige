//! `message_ext(src, ext_chat, ext_msg) → message_id` (слайс 4, ADR-0001 приложение §C): локальный несинхронизируемый
//! индекс внешних id импорта, производный от `core.ext`. Дедупликация повторного импорта идёт по нему. Вставка —
//! `INSERT OR IGNORE`: два сообщения с разными id и одним `ext` (импорт одного экспорта на двух устройствах до
//! синхронизации) не ломают пересчёт.

use rusqlite::Connection;

/// Запись о внешнем id; уже занятый ключ остаётся за первой записью. Ответ: добавлена ли строка.
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

/// Убирает строки записи (удаление навсегда): повторный импорт вернёт такую запись как новую (спецификация слайса 4,
/// решение 4).
pub(crate) fn remove(conn: &Connection, message_id: &str) -> rusqlite::Result<()> {
    conn.prepare_cached("DELETE FROM message_ext WHERE message_id = ?1")?
        .execute([message_id])?;
    Ok(())
}

/// Таблица заново из `message.ext_json` (миграция 4). Незаполненный или повреждённый `ext_json` строк не даёт.
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
