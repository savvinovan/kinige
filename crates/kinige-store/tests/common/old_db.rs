//! An old database for the migration tests (KINIG-103). It is not the current schema with tables dropped: the schema comes from
//! `kinige_store::db_at_version`, the first `n` real entries of the migration list applied the way an upgrade applies them, so
//! the old tables and columns are exactly what a database of that version had. Only the rows come from the store that wrote them.

use super::{db_path, wait_until};
use rusqlite::Connection;
use std::fs;
use std::path::{Path, PathBuf};

/// Replaces the database in `dir` (closed: no `Store` on it) with one of schema `version` holding the rows the current database
/// had, table by table and column by column, for the tables and columns that version has. What a later migration added is
/// not there (no table, no column, no row). From version 4 the search index comes along: the map and the shadow tables of the FTS5
/// table are copied row for row together with the version mark in `app_setting`, so the first open finds an intact index and does not
/// rebuild it, as it is across a real migration. Every table is checked to have the same number of rows as its source.
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
            assert!(
                !columns.is_empty(),
                "{table} has no column in common with the current schema: its rows would be left behind"
            );
            // The old schema's own start (migration 4 puts the index version mark into `app_setting`) gives way to the rows.
            conn.execute_batch(&format!(
                "DELETE FROM main.{table}; \
                 INSERT INTO main.{table} ({columns}) SELECT {columns} FROM current.{table};"
            ))
            .unwrap();
            assert_eq!(
                count(&conn, "main", &table),
                count(&conn, "current", &table),
                "every row of {table} was copied"
            );
        }
        if version < 4 {
            // The mark of an index that this schema does not have yet (the rows of `app_setting` came from the current database).
            conn.execute_batch("DELETE FROM main.app_setting WHERE key = 'search_index_version';")
                .unwrap();
        }
        conn.execute_batch("DETACH DATABASE current;").unwrap();
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

/// The tables of the old schema that hold rows: the ordinary ones and the shadow tables of the search index (`search_fts_data`
/// and the others), but not the virtual table itself, which has no rows of its own, and not SQLite's own.
fn plain_tables(conn: &Connection) -> Vec<String> {
    conn.prepare(
        "SELECT name FROM main.sqlite_master \
         WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND sql NOT LIKE 'CREATE VIRTUAL%' ORDER BY name",
    )
    .unwrap()
    .query_map([], |row| row.get(0))
    .unwrap()
    .collect::<Result<_, _>>()
    .unwrap()
}

fn count(conn: &Connection, schema: &str, table: &str) -> i64 {
    conn.query_row(&format!("SELECT COUNT(*) FROM {schema}.{table}"), [], |r| {
        r.get(0)
    })
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
