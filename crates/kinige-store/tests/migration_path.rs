//! The upgrade path (KINIG-103): a database of every old schema version, made by `db_at_version` (the first `n` real entries of
//! the migration list), is opened by the current store and ends with the same schema as a database created today, compared
//! as data through `sqlite_master`. The pinned hashes of the migration texts are in `src/schema/tests.rs`.

// The root of the integration tests is entirely test code: `cfg(test)` gives helpers outside #[test] the same
// clippy.toml relaxations (unwrap/expect/panic/indexing) as the tests in src/.
#![cfg(test)]

mod common;

use common::old_db::downgrade_to;
use common::*;
use kinige_domain::groups::INBOX_ID;
use kinige_store::{LATEST_VERSION, db_at_version};
use rusqlite::Connection;
use std::path::Path;

/// `(type, name, table, sql)` of every object in the database, in a fixed order.
type Schema = Vec<(String, String, String, Option<String>)>;

#[test]
fn every_old_version_upgrades_to_the_schema_of_a_fresh_database() {
    let fresh = tempfile::tempdir().unwrap();
    drop(open_without_mirror(fresh.path()));
    let expected = schema_of(&db_path(fresh.path()));
    assert_eq!(user_version(&db_path(fresh.path())), LATEST_VERSION);
    assert!(
        expected.len() > 30,
        "the fresh schema was read: {}",
        expected.len()
    );

    for version in 1..=LATEST_VERSION {
        let dir = tempfile::tempdir().unwrap();
        db_at_version(&db_path(dir.path()), version).unwrap();
        assert_eq!(user_version(&db_path(dir.path())), version);
        if version < LATEST_VERSION {
            assert_ne!(
                schema_of(&db_path(dir.path())),
                expected,
                "schema {version} is an old one"
            );
        }
        drop(open_without_mirror(dir.path()));
        assert_eq!(
            user_version(&db_path(dir.path())),
            LATEST_VERSION,
            "from schema {version}"
        );
        assert_eq!(
            schema_of(&db_path(dir.path())),
            expected,
            "upgraded from schema {version}"
        );
    }
}

#[test]
fn an_old_database_is_never_made_from_nothing_or_over_something() {
    let dir = tempfile::tempdir().unwrap();
    for bad in [i64::MIN, -1, 0, LATEST_VERSION + 1, i64::MAX] {
        let path = dir
            .path()
            .join(format!("bad{}.db", bad.unsigned_abs() % 1000));
        assert!(db_at_version(&path, bad).is_err(), "version {bad}");
        assert!(!path.exists(), "nothing is created for version {bad}");
    }
    let path = dir.path().join("there.db");
    db_at_version(&path, 3).unwrap();
    assert!(
        db_at_version(&path, 4).is_err(),
        "an existing database is not touched"
    );
    assert_eq!(user_version(&path), 3);
}

#[test]
fn from_version_four_the_old_database_keeps_an_intact_search_index_across_the_migration() {
    for version in 3..LATEST_VERSION {
        let dir = tempfile::tempdir().unwrap();
        {
            let store = open_without_mirror(dir.path());
            store.send_message(INBOX_ID, "книга", &moscow(T0)).unwrap();
        }
        downgrade_to(dir.path(), version);
        let conn = Connection::open(db_path(dir.path())).unwrap();
        let mark: Option<String> = conn
            .query_row(
                "SELECT value FROM app_setting WHERE key = 'search_index_version'",
                [],
                |r| r.get(0),
            )
            .ok();
        if version < 4 {
            assert_eq!(mark, None, "schema {version} has no index and no mark");
        } else {
            assert!(mark.is_some(), "schema {version} keeps the mark");
            let found: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM search_fts WHERE search_fts MATCH 'книг*'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(found, 1, "schema {version} has the entry in its index");
        }
        drop(conn);
        let store = open_without_mirror(dir.path());
        let hits = store
            .search("книга", &Default::default(), None, 10)
            .unwrap()
            .hits;
        assert_eq!(hits.len(), 1, "from schema {version}");
    }
}

fn schema_of(path: &Path) -> Schema {
    Connection::open(path)
        .unwrap()
        .prepare("SELECT type, name, tbl_name, sql FROM sqlite_master ORDER BY type, name")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

fn user_version(path: &Path) -> i64 {
    Connection::open(path)
        .unwrap()
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap()
}
