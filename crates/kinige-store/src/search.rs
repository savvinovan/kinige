//! Индекс поиска (слайс 4, ADR-0008): `search_fts` — FTS5 без хранения текста (`contentless_delete`), `search_map` —
//! `rowid → message_id`. Локальная производная от `body_md`, не синхронизируется. Обновляется в той же транзакции, что
//! любое изменение текста записи (отправка, дописывание, правка, галочка `- [ ]`, импорт); удаление навсегда убирает
//! строку. Корзина в индексе остаётся (запись можно вернуть) и отсекается запросом. Ревизии в индекс не попадают.
//!
//! Версия правил — `kinige_domain::search::SEARCH_INDEX_VERSION` в `app_setting.search_index_version`: если она не
//! совпала, индекс перестраивается при открытии базы.

use crate::StoreError;
use crate::error::write_err;
use crate::settings::get_setting;
use kinige_domain::search::{SEARCH_INDEX_VERSION, index_text};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior};

const VERSION_KEY: &str = "search_index_version";

/// Строка индекса записи заменяется основами её текущего `body_md` (нет строки — создаётся).
pub(crate) fn reindex(conn: &Connection, message_id: &str, body_md: &str) -> rusqlite::Result<()> {
    let text = index_text(body_md);
    let existing: Option<i64> = conn
        .prepare_cached("SELECT rowid FROM search_map WHERE message_id = ?1")?
        .query_row([message_id], |row| row.get(0))
        .optional()?;
    let rowid = match existing {
        Some(rowid) => {
            conn.prepare_cached("DELETE FROM search_fts WHERE rowid = ?1")?
                .execute([rowid])?;
            rowid
        }
        None => {
            conn.prepare_cached("INSERT INTO search_map (message_id) VALUES (?1)")?
                .execute([message_id])?;
            conn.last_insert_rowid()
        }
    };
    conn.prepare_cached("INSERT INTO search_fts (rowid, t) VALUES (?1, ?2)")?
        .execute((rowid, text))?;
    Ok(())
}

/// Убирает запись из индекса (удаление навсегда). Нет строки — ничего не происходит.
pub(crate) fn remove(conn: &Connection, message_id: &str) -> rusqlite::Result<()> {
    let existing: Option<i64> = conn
        .prepare_cached("SELECT rowid FROM search_map WHERE message_id = ?1")?
        .query_row([message_id], |row| row.get(0))
        .optional()?;
    if let Some(rowid) = existing {
        conn.prepare_cached("DELETE FROM search_fts WHERE rowid = ?1")?
            .execute([rowid])?;
        conn.prepare_cached("DELETE FROM search_map WHERE rowid = ?1")?
            .execute([rowid])?;
    }
    Ok(())
}

/// Индекс заново из `message.body_md` всех записей (в том числе из корзины) и версия правил. Вызывающий открыл транзакцию:
/// миграция 4 и перестройка при смене версии.
pub(crate) fn rebuild_all(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO search_fts (search_fts) VALUES ('delete-all')",
        [],
    )?;
    conn.execute("DELETE FROM search_map", [])?;
    let mut statement = conn.prepare("SELECT id, body_md FROM message")?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let id: String = row.get(0)?;
        let body: String = row.get(1)?;
        reindex(conn, &id, &body)?;
    }
    conn.execute(
        "INSERT INTO app_setting (key, value) VALUES (?1, ?2) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        (VERSION_KEY, SEARCH_INDEX_VERSION.to_string()),
    )?;
    Ok(())
}

/// Лечение повреждённого индекса (ошибка `Corrupt` от запроса поиска): версия правил сбрасывается отдельной фиксацией,
/// затем индекс перестраивается заново из `body_md`. Если перестройка не дошла до конца, версии нет, и индекс
/// перестроится при следующем открытии. Повреждение самой базы проявится здесь же как `Corrupt` писателя.
pub(crate) fn heal(conn: &mut Connection) -> Result<(), StoreError> {
    conn.execute("DELETE FROM app_setting WHERE key = ?1", [VERSION_KEY])
        .map_err(write_err)?;
    rebuild_if_stale(conn)
}

/// При открытии базы: версия правил в базе не совпала с версией приложения — индекс перестраивается одной транзакцией.
pub(crate) fn rebuild_if_stale(conn: &mut Connection) -> Result<(), StoreError> {
    let stored = get_setting(conn, VERSION_KEY).map_err(write_err)?;
    if stored.as_deref() == Some(SEARCH_INDEX_VERSION.to_string().as_str()) {
        return Ok(());
    }
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(write_err)?;
    rebuild_all(&tx).map_err(write_err)?;
    tx.commit().map_err(write_err)
}
