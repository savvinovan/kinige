//! Индекс поиска (слайс 4, ADR-0008): `search_fts` — FTS5 без хранения текста (`contentless_delete`), `search_map` —
//! `rowid → message_id`. Локальная производная от `body_md`, не синхронизируется. Обновляется в той же транзакции, что
//! любое изменение текста записи (отправка, дописывание, правка, галочка `- [ ]`, импорт); удаление навсегда убирает
//! строку. Корзина в индексе остаётся (запись можно вернуть) и отсекается запросом. Ревизии в индекс не попадают.
//!
//! Версия правил — `kinige_domain::search::SEARCH_INDEX_VERSION` в `app_setting.search_index_version`: если она не
//! совпала, индекс перестраивается при открытии базы.
//!
//! Повреждение самой таблицы поиска (`error::is_search_index_error`) — не повреждение базы: индекс перестраивается из
//! `message.body_md`. Запись, наткнувшаяся на него, перестраивает индекс прямо в своей транзакции (запись не теряется),
//! запрос поиска и проход обслуживания — отдельным заданием писателя ([`Store::heal_search_index`]).

use crate::Store;
use crate::StoreError;
use crate::error::{is_search_index_error, write_err};
use crate::settings::get_setting;
use kinige_domain::search::{SEARCH_INDEX_VERSION, index_text};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior};
use std::sync::atomic::Ordering;

const VERSION_KEY: &str = "search_index_version";

/// Та же таблица, что создаёт миграция 4 (`schema.rs`); тест сверяет текст с `sqlite_master`.
const FTS_DDL: &str = "CREATE VIRTUAL TABLE search_fts USING fts5(\n  t, content='', contentless_delete=1, tokenize='unicode61 remove_diacritics 0'\n)";

/// Строка индекса записи заменяется основами её текущего `body_md` (нет строки — создаётся). Таблица повреждена —
/// индекс целиком перестраивается из `message` (строка этой записи уже там) и запись не теряется.
pub(crate) fn reindex(conn: &Connection, message_id: &str, body_md: &str) -> rusqlite::Result<()> {
    guarded(conn, || put_row(conn, message_id, body_md))
}

/// Операция над индексом под точкой сохранения: упавшая на порче таблица оставляет у FTS5 полузаписанное состояние, в
/// котором ни `DROP`, ни запись уже не проходят; откат к точке сбрасывает его, и индекс перестраивается заново.
fn guarded(
    conn: &Connection,
    operation: impl FnOnce() -> rusqlite::Result<()>,
) -> rusqlite::Result<()> {
    conn.execute_batch("SAVEPOINT search_index")?;
    match operation() {
        Ok(()) => conn.execute_batch("RELEASE search_index"),
        Err(error) => {
            conn.execute_batch("ROLLBACK TO search_index; RELEASE search_index")?;
            if is_search_index_error(&error) {
                rebuild_all(conn)
            } else {
                Err(error)
            }
        }
    }
}

fn put_row(conn: &Connection, message_id: &str, body_md: &str) -> rusqlite::Result<()> {
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

/// Убирает запись из индекса (удаление навсегда); вызывается после удаления строки `message`. Нет строки — ничего не
/// происходит; таблица повреждена — индекс перестраивается из оставшихся записей.
pub(crate) fn remove(conn: &Connection, message_id: &str) -> rusqlite::Result<()> {
    guarded(conn, || remove_row(conn, message_id))
}

fn remove_row(conn: &Connection, message_id: &str) -> rusqlite::Result<()> {
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

/// Индекс заново из `message.body_md` всех записей (в том числе из корзины) и версия правил. Таблица пересоздаётся
/// (`DROP` + `CREATE`), а не очищается: у повреждённой таблицы `delete-all` тоже может не пройти. Вызывающий открыл
/// транзакцию: миграция 4, перестройка при смене версии, лечение.
pub(crate) fn rebuild_all(conn: &Connection) -> rusqlite::Result<()> {
    conn.flush_prepared_statement_cache();
    drop_index(conn)?;
    conn.execute(FTS_DDL, [])?;
    conn.execute("DELETE FROM search_map", [])?;
    let mut statement = conn.prepare("SELECT id, body_md FROM message")?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let id: String = row.get(0)?;
        let body: String = row.get(1)?;
        put_row(conn, &id, &body)?;
    }
    conn.execute(
        "INSERT INTO app_setting (key, value) VALUES (?1, ?2) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        (VERSION_KEY, SEARCH_INDEX_VERSION.to_string()),
    )?;
    Ok(())
}

/// Удаляет `search_fts`. Повреждены настройки таблицы («invalid fts5 file format»): такая таблица не открывается, а значит
/// и не удаляется. Тогда номер формата в теневой таблице чинится руками (4 — текущий формат FTS5), после чего удаление
/// проходит.
fn drop_index(conn: &Connection) -> rusqlite::Result<()> {
    match conn.execute("DROP TABLE IF EXISTS search_fts", []) {
        Err(error) if is_search_index_error(&error) => {
            conn.execute(
                "INSERT OR REPLACE INTO search_fts_config (k, v) VALUES ('version', 4)",
                [],
            )?;
            conn.execute("DROP TABLE IF EXISTS search_fts", [])
                .map(|_| ())
        }
        other => other.map(|_| ()),
    }
}

/// Лечение повреждённого индекса: версия правил сбрасывается отдельной фиксацией, затем индекс перестраивается заново из
/// `body_md`. Если перестройка не дошла до конца, версии нет, и индекс перестроится при следующем открытии. Повреждение
/// самой базы проявится здесь же как `Corrupt` писателя.
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

impl Store {
    /// Сколько раз индекс лечили в этом запуске: читающий запоминает число до запроса и передаёт в
    /// [`Store::heal_search_index`].
    pub(crate) fn search_generation(&self) -> u64 {
        self.inner.search_heals.load(Ordering::SeqCst)
    }

    /// Сколько раз лечили индекс в этом запуске (для тестов; не часть API приложения).
    #[doc(hidden)]
    pub fn search_heal_count(&self) -> u64 {
        self.search_generation()
    }

    /// Лечит индекс на писателе, если с момента неудачного чтения (`seen` — число лечений до него) его никто не вылечил:
    /// несколько одновременных запросов, упавших на одной порче, дают одну перестройку, а не по одной на запрос.
    pub(crate) fn heal_search_index(&self, seen: u64) -> Result<(), StoreError> {
        let heals = self.inner.search_heals.clone();
        self.write(move |conn| {
            if heals.load(Ordering::SeqCst) != seen {
                return Ok(());
            }
            heal(conn)?;
            heals.fetch_add(1, Ordering::SeqCst);
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::FTS_DDL;

    /// Перестройка создаёт ровно ту таблицу, что миграция 4: иначе после лечения индекс устроен иначе.
    #[test]
    fn rebuild_creates_the_table_migration_four_creates() {
        assert!(crate::schema::MIGRATION_4.contains(&format!("{FTS_DDL};")));
    }
}
