//! The search index (slice 4, ADR-0008): `search_fts` -- FTS5 without storing the text (`contentless_delete`),
//! `search_map` -- `rowid → message_id`. A local derivative of `body_md`, not synced. Updated in the same transaction as
//! any change of entry text (send, append, edit, a `- [ ]` tick, import); deleting forever removes the row. The trash
//! stays in the index (the entry can be restored) and is cut off by the query. Revisions do not enter the index.
//!
//! The rules version is `kinige_domain::search::SEARCH_INDEX_VERSION` in `app_setting.search_index_version`: if it does
//! not match, the index is rebuilt when the database is opened.
//!
//! Corruption of the search table itself (`error::is_search_index_error`) is not database corruption: the index is
//! rebuilt from `message.body_md`. A write that runs into it rebuilds the index right in its own transaction (the write
//! is not lost), a search query and a maintenance pass do it with a separate writer job ([`Store::heal_search_index`]).

use crate::Store;
use crate::StoreError;
use crate::error::{is_search_index_error, write_err};
use crate::settings::get_setting;
use kinige_domain::attachment::{VOICE_SEARCH_WORD, is_voice_file_name};
use kinige_domain::search::{SEARCH_INDEX_VERSION, index_text};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior};
use std::sync::atomic::Ordering;

const VERSION_KEY: &str = "search_index_version";

/// The same table that migration 4 creates (`schema.rs`); a test compares the text with `sqlite_master`.
const FTS_DDL: &str = "CREATE VIRTUAL TABLE search_fts USING fts5(\n  t, content='', contentless_delete=1, tokenize='unicode61 remove_diacritics 0'\n)";

/// The entry's index row is replaced with the stems of its current `body_md` (no row -- it is created). If the table is
/// corrupt, the whole index is rebuilt from `message` (this entry's row is already there) and the write is not lost.
pub(crate) fn reindex(conn: &Connection, message_id: &str, body_md: &str) -> rusqlite::Result<()> {
    guarded(conn, || put_row(conn, message_id, body_md))
}

/// An index operation under a savepoint: one that failed on table corruption leaves FTS5 in a half-written state in
/// which neither `DROP` nor a write passes any more; rolling back to the savepoint clears it, and the index is rebuilt.
fn guarded(
    conn: &Connection,
    operation: impl FnOnce() -> rusqlite::Result<()>,
) -> rusqlite::Result<()> {
    conn.execute_batch("SAVEPOINT search_index")?;
    match operation() {
        Ok(()) => conn.execute_batch("RELEASE search_index"),
        Err(error) => {
            // The rollback failed (the savepoint is gone after a heavy failure: full disk, I/O, memory): the original error
            // goes out, not the rollback error, which would hide the real cause.
            if conn
                .execute_batch("ROLLBACK TO search_index; RELEASE search_index")
                .is_err()
            {
                return Err(error);
            }
            if is_search_index_error(&error) {
                rebuild_all(conn)?;
                REBUILT.with(|flag| flag.set(true));
                Ok(())
            } else {
                Err(error)
            }
        }
    }
}

thread_local! {
    /// A write rebuilt the index itself (there is one writer thread; `Store::write` takes the flag after the job).
    static REBUILT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Whether a write rebuilt the index since the last call; the flag is reset.
pub(crate) fn take_rebuilt() -> bool {
    REBUILT.with(|flag| flag.replace(false))
}

/// The text of an entry's index row: its body plus the stems of its live attachments' file names, extension left out (slice 7).
/// The `attachment` table may be missing while migrations run (migration 4 rebuilds the index before migration 7 exists).
fn index_source(conn: &Connection, message_id: &str, body_md: &str) -> rusqlite::Result<String> {
    let names = match conn
        .prepare_cached(
            "SELECT filename FROM attachment WHERE message_id = ?1 AND deleted_at IS NULL ORDER BY sort_index",
        )
        .and_then(|mut statement| {
            statement
                .query_map([message_id], |row| row.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<String>>>()
        }) {
        Ok(names) => names,
        Err(rusqlite::Error::SqliteFailure(_, Some(message))) if message.contains("no such table") => {
            Vec::new()
        }
        Err(error) => return Err(error),
    };
    let mut text = body_md.to_string();
    for name in names {
        text.push('\n');
        // A recorder's file is found by the word "voice", not by the date and time in its name (slice 7a).
        if is_voice_file_name(&name) {
            text.push_str(VOICE_SEARCH_WORD);
        } else {
            text.push_str(
                name.rsplit_once('.')
                    .map_or(name.as_str(), |(stem, _)| stem),
            );
        }
    }
    Ok(text)
}

fn put_row(conn: &Connection, message_id: &str, body_md: &str) -> rusqlite::Result<()> {
    let text = index_text(&index_source(conn, message_id, body_md)?);
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

/// Removes the entry from the index (deleting forever); called after the `message` row is deleted. No row -- nothing
/// happens; the table is corrupt -- the index is rebuilt from the remaining entries.
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

/// The index anew from `message.body_md` of all entries (including the trash) and the rules version. The table is
/// recreated (`DROP` + `CREATE`), not cleared: on a corrupt table `delete-all` may fail too. The caller opened a
/// transaction: migration 4, a rebuild on a version change, healing.
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

/// Drops `search_fts`. If the table settings are corrupt ("invalid fts5 file format"), such a table does not open and
/// so cannot be dropped. Then the format number in the shadow table is fixed by hand (4 is the current FTS5 format),
/// after which the drop passes.
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

/// Healing a corrupt index: the rules version is reset by a separate commit, then the index is rebuilt anew from
/// `body_md`. If the rebuild did not finish, there is no version, and the index will be rebuilt at the next open.
/// Corruption of the database itself shows up here as the writer's `Corrupt`.
pub(crate) fn heal(conn: &mut Connection) -> Result<(), StoreError> {
    conn.execute("DELETE FROM app_setting WHERE key = ?1", [VERSION_KEY])
        .map_err(write_err)?;
    rebuild_if_stale(conn)
}

/// On opening the database: the rules version in the database does not match the app's -- the index is rebuilt in one transaction.
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
    /// How many times the index was healed in this launch: the reader remembers the number before the query and passes it to
    /// [`Store::heal_search_index`].
    pub(crate) fn search_generation(&self) -> u64 {
        self.inner.search_heals.load(Ordering::SeqCst)
    }

    /// How many times the index was healed in this launch (for tests; not part of the app API).
    #[doc(hidden)]
    pub fn search_heal_count(&self) -> u64 {
        self.search_generation()
    }

    /// Heals the index on the writer if nobody has healed it since the failed read (`seen` is the heal count before it):
    /// several simultaneous queries that failed on the same corruption cause one rebuild, not one per query.
    pub(crate) fn heal_search_index(&self, seen: u64) -> Result<(), StoreError> {
        let heals = self.inner.search_heals.clone();
        self.write_raw(move |conn| {
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
    use crate::StoreError;

    /// A rebuild creates exactly the table migration 4 creates: otherwise after healing the index is built differently.
    #[test]
    fn rebuild_creates_the_table_migration_four_creates() {
        assert!(crate::schema::MIGRATION_4.contains(&format!("{FTS_DDL};")));
    }

    /// Index corruption that surfaced outside `guarded` (for example, on `COMMIT`): the write returns an error, but the
    /// index is healed right away so that a retry passes.
    #[test]
    fn an_index_error_outside_guarded_heals_the_index_for_the_retry() {
        let dir = tempfile::tempdir().unwrap();
        let mut options = crate::StoreOptions::new(dir.path().join("kinige.db"), None);
        options.start_maintenance = false;
        let store = crate::Store::open(options).unwrap();
        store
            .send_message(
                kinige_domain::groups::INBOX_ID,
                "книга",
                &crate::Now::at(1_800_000_000_000, "UTC"),
            )
            .unwrap();
        store
            .write(|conn| {
                conn.execute(
                    "UPDATE search_fts_data SET block = X'FFFFFFFFFFFF' WHERE id = 10",
                    [],
                )
                .map_err(crate::error::write_err)?;
                Ok(())
            })
            .unwrap();
        let failed: Result<(), _> =
            store.write(|_| Err(StoreError::SearchIndex("error on COMMIT".into())));
        assert!(matches!(failed, Err(StoreError::SearchIndex(_))));
        assert_eq!(store.search_heal_count(), 1);
        let found = store
            .search("книга", &Default::default(), None, 10)
            .unwrap()
            .hits;
        assert_eq!(
            found.len(),
            1,
            "the retry passes: the index is already healed"
        );
        assert_eq!(
            store.search_heal_count(),
            1,
            "the query did not rebuild again"
        );
    }

    /// Healing lives in the same transaction as the write: rolling back the write brings back the corrupt index too, and
    /// the next write heals it again and commits.
    #[test]
    fn a_rebuild_inside_a_rolled_back_transaction_is_rolled_back_with_it() {
        use super::reindex;
        let dir = tempfile::tempdir().unwrap();
        let mut options = crate::StoreOptions::new(dir.path().join("kinige.db"), None);
        options.start_maintenance = false;
        let store = crate::Store::open(options).unwrap();
        let sent = store
            .send_message(
                kinige_domain::groups::INBOX_ID,
                "книга",
                &crate::Now::at(1_800_000_000_000, "UTC"),
            )
            .unwrap();
        let id = sent.id.clone();
        fn broken(conn: &rusqlite::Connection) -> rusqlite::Result<i64> {
            conn.query_row(
                "SELECT COUNT(*) FROM search_fts WHERE search_fts MATCH 'книг*'",
                [],
                |row| row.get::<_, i64>(0),
            )
        }
        store
            .write(|conn| {
                conn.execute(
                    "UPDATE search_fts_data SET block = X'FFFFFFFFFFFF' WHERE id = 10",
                    [],
                )
                .map_err(crate::error::write_err)?;
                assert!(broken(conn).is_err(), "the index is corrupt");
                Ok(())
            })
            .unwrap();
        let again = id.clone();
        store
            .write(move |conn| {
                conn.execute_batch("BEGIN")
                    .map_err(crate::error::write_err)?;
                reindex(conn, &again, "новая книга").map_err(crate::error::write_err)?;
                assert_eq!(
                    broken(conn).unwrap(),
                    1,
                    "inside the transaction the index is healed"
                );
                conn.execute_batch("ROLLBACK")
                    .map_err(crate::error::write_err)?;
                assert!(
                    broken(conn).is_err(),
                    "after the rollback the index is corrupt again"
                );
                Ok(())
            })
            .unwrap();
        // The next write (its own transaction) heals the index and commits it.
        let last = id.clone();
        store
            .write(move |conn| {
                reindex(conn, &last, "ещё книга").map_err(crate::error::write_err)?;
                assert_eq!(broken(conn).unwrap(), 1);
                Ok(())
            })
            .unwrap();
        let found = store
            .search("книга", &Default::default(), None, 10)
            .unwrap()
            .hits;
        assert_eq!(found.len(), 1);
        assert_eq!(
            store.search_heal_count(),
            2,
            "both write rebuilds are counted"
        );
    }
}
