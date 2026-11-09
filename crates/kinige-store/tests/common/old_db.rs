//! An old database for the migration tests (KINIG-103). It is not the current schema with tables dropped: the schema comes from
//! `kinige_store::db_at_version`, the first `n` real entries of the migration list applied the way an upgrade applies them, so
//! the old tables and columns are exactly what a database of that version had. Only the rows come from the store that wrote them.

use super::{db_path, wait_until};
use rusqlite::Connection;
use std::fs;
use std::path::{Path, PathBuf};

/// Replaces the database in `dir` (closed: no `Store` on it) with one of schema `version` holding the rows the current database
/// had, table by table and column by column, for the tables and columns that version has. What a later migration added is
/// not there (no table, no column, no row); the search index is derived, so it is not copied either: the version mark is dropped and
/// the first open builds the index from the entries, as it does for any database whose index is missing.
pub fn downgrade_to(dir: &Path, version: i64) {
    let current = db_path(dir);
    // The store has let go of the file: the last connection to close deletes `-wal` and `-shm` after its checkpoint (the
    // background threads of a dropped store do not close theirs instantly), and on Windows an open file cannot be replaced.
    wait_until("the closed store releases the database", || {
        !side_file(&current, "-wal").exists() && !side_file(&current, "-shm").exists()
    });
    let old = dir.join("old-schema.db");
    kinige_store::db_at_version(&old, version).expect("a database of the old schema");
    {
        let conn = Connection::open(&old).unwrap();
        conn.execute("ATTACH DATABASE ?1 AS current", [current.to_str().unwrap()])
            .unwrap();
        for table in plain_tables(&conn) {
            let columns = shared_columns(&conn, &table);
            if columns.is_empty() {
                continue;
            }
            conn.execute_batch(&format!(
                // OR REPLACE: migration 4 has already put the index version mark into `app_setting`; it is deleted below.
                "INSERT OR REPLACE INTO main.{table} ({columns}) SELECT {columns} FROM current.{table};"
            ))
            .unwrap();
        }
        conn.execute_batch("DELETE FROM main.app_setting WHERE key = 'search_index_version'; DETACH DATABASE current;")
            .unwrap();
    }
    fs::remove_file(&current).unwrap();
    for suffix in ["-wal", "-shm"] {
        let _ = fs::remove_file(side_file(&current, suffix));
    }
    fs::rename(&old, &current).unwrap();
}

/// `kinige.db` + `-wal`: the file SQLite keeps next to the database.
fn side_file(database: &Path, suffix: &str) -> PathBuf {
    let mut name = database.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

/// The ordinary tables of the old schema: not the search index (virtual, with its shadow tables, and the map to it) and not
/// SQLite's own.
fn plain_tables(conn: &Connection) -> Vec<String> {
    conn.prepare(
        "SELECT name FROM main.sqlite_master \
         WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name NOT LIKE 'search_%' ORDER BY name",
    )
    .unwrap()
    .query_map([], |row| row.get(0))
    .unwrap()
    .collect::<Result<_, _>>()
    .unwrap()
}

/// The columns `table` has both in the old schema and in the current one, as a list for a statement.
fn shared_columns(conn: &Connection, table: &str) -> String {
    conn.prepare(
        "SELECT o.name FROM pragma_table_info(?1, 'main') o \
         JOIN pragma_table_info(?1, 'current') c ON c.name = o.name ORDER BY o.cid",
    )
    .unwrap()
    .query_map([table], |row| row.get::<_, String>(0))
    .unwrap()
    .collect::<Result<Vec<_>, _>>()
    .unwrap()
    .join(", ")
}
