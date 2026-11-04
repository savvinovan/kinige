//! The database file copy and integrity (slice 3): the copy before a migration, the day's copy, rotation, consistency with a running
//! writer, a corrupted database, restoring from a copy.

// The root of the integration tests is entirely test code: `cfg(test)` gives helpers outside #[test] the same
// clippy.toml relaxations (unwrap/expect/panic/indexing) as the tests in src/.
#![cfg(test)]
#![allow(clippy::disallowed_methods)] // `thread::spawn` is forbidden in app code (clippy.toml), the test needs a thread without handling a spawn failure

mod common;

use common::ops::*;
use common::*;
use kinige_domain::backup::{daily_name, pre_migration_name};
use kinige_domain::groups::INBOX_ID;
use kinige_domain::history::DAY_MS;
use kinige_domain::{BackupError, BackupKind, ErrorCode};
use kinige_store::{CorruptionHook, Now, Store, StoreError, backups, backups_dir};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

fn at(ms: i64) -> Now {
    moscow(ms)
}

fn backup_files(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(backups_dir(&db_path(dir)))
        .map(|entries| {
            entries
                .flatten()
                .map(|e| e.file_name().into_string().unwrap())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

fn user_version(path: &Path) -> i64 {
    rusqlite::Connection::open(path)
        .unwrap()
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap()
}

/// Returns the database to the slice 2 state: without the migration 3 tables, `user_version = 2`.
fn downgrade_to_two(dir: &Path) {
    let conn = raw(dir);
    conn.execute_batch(
        "DROP INDEX message_deleted; DROP TABLE message_revision; DROP TABLE tombstone; DROP TABLE message_tag; \
         DROP TABLE mirror_chat; DROP TABLE message_ext; DROP TABLE search_fts; DROP TABLE search_map; \
         DROP TABLE mcp_log; DROP TABLE mcp_log_message; ALTER TABLE draft DROP COLUMN reply_to_id; \
         DROP TABLE attachment; DROP TABLE attachment_local; DROP TABLE draft_attachment; DROP TABLE mirror_file; \
         DELETE FROM app_setting WHERE key = 'search_index_version';",
    )
    .unwrap();
    conn.pragma_update(None, "user_version", 2).unwrap();
}

#[test]
fn migration_three_takes_a_copy_first_and_the_copy_still_has_schema_two() {
    let dir = tempfile::tempdir().unwrap();
    {
        let store = open_without_mirror(dir.path());
        store
            .send_message(INBOX_ID, "до миграции #старая", &at(T0))
            .unwrap();
        store
            .send_message(INBOX_ID, "вторая", &at(T0 + MINUTE))
            .unwrap();
    }
    downgrade_to_two(dir.path());
    assert!(backup_files(dir.path()).is_empty());

    let store = Store::open_at(options(dir.path()), &at(T0 + 5 * MINUTE)).unwrap();
    let files = backup_files(dir.path());
    assert_eq!(
        files,
        [pre_migration_name(
            2,
            &kinige_store::utc_stamp(T0 + 5 * MINUTE)
        )],
        "exactly one copy, taken before the migration"
    );
    let copy = backups_dir(&db_path(dir.path())).join(&files[0]);
    assert_eq!(user_version(&copy), 2, "the copy has schema 2");
    let conn = rusqlite::Connection::open(&copy).unwrap();
    let verdict: String = conn
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(verdict, "ok");
    let in_copy: i64 = conn
        .query_row("SELECT COUNT(*) FROM message", [], |r| r.get(0))
        .unwrap();
    assert_eq!(in_copy, 2);
    assert!(
        conn.prepare("SELECT 1 FROM message_revision").is_err(),
        "the copy has no migration 3 tables"
    );
    let mode: String = conn
        .pragma_query_value(None, "journal_mode", |r| r.get(0))
        .unwrap();
    assert_eq!(mode, "delete", "the copy is a single file without -wal");

    // The database is migrated: tags are recomputed from the texts, entries are in place.
    assert_eq!(
        user_version(&db_path(dir.path())),
        kinige_store::LATEST_VERSION
    );
    assert_eq!(count_rows(dir.path(), "message"), 2);
    let tags: Vec<String> = raw(dir.path())
        .prepare("SELECT tag FROM message_tag")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(tags, ["старая"]);
    assert_eq!(store.list_chats().unwrap()[0].count, 2);
    // Reopening does not take a copy.
    drop(store);
    drop(open_without_mirror(dir.path()));
    assert_eq!(backup_files(dir.path()).len(), 1);
}

#[test]
fn a_fresh_database_and_a_current_one_take_no_migration_copy() {
    let dir = tempfile::tempdir().unwrap();
    drop(open_without_mirror(dir.path()));
    assert!(backup_files(dir.path()).is_empty(), "a fresh database");
    drop(open_without_mirror(dir.path()));
    assert!(backup_files(dir.path()).is_empty(), "the current version");
}

#[test]
fn no_copy_means_no_migration() {
    let dir = tempfile::tempdir().unwrap();
    drop(open_without_mirror(dir.path()));
    downgrade_to_two(dir.path());
    // A file lies where the copies folder should be: a copy cannot be taken.
    fs::write(backups_dir(&db_path(dir.path())), b"file").unwrap();
    let error = Store::open_at(options(dir.path()), &at(T0)).err().unwrap();
    assert!(matches!(error, StoreError::Write(_)), "{error}");
    assert_eq!(
        user_version(&db_path(dir.path())),
        2,
        "the migration did not start"
    );
}

#[test]
fn a_broken_database_is_not_migrated() {
    let dir = tempfile::tempdir().unwrap();
    {
        let store = open_without_mirror(dir.path());
        for i in 0..300 {
            store
                .send_message(INBOX_ID, &"текст ".repeat(100 + i), &at(T0 + i as i64))
                .unwrap();
        }
    }
    wait_closed(dir.path());
    downgrade_to_two(dir.path());
    damage_pages(&db_path(dir.path()));
    let error = Store::open_at(options(dir.path()), &at(T0)).err().unwrap();
    assert!(matches!(error, StoreError::Corrupt(_)), "{error}");
    assert_eq!(error.code(), ErrorCode::DatabaseCorrupt);
    assert_eq!(user_version(&db_path(dir.path())), 2);
    assert!(
        backup_files(dir.path()).is_empty(),
        "we do not copy a spoiled database"
    );
}

/// Spoils pages at the tail of the file: the integrity check must notice it, while opening the database (schema, chats) must still pass.
fn damage_pages(path: &Path) {
    let mut bytes = fs::read(path).unwrap();
    let len = bytes.len();
    assert!(len > 40 * 1024, "enough pages: {len}");
    for byte in &mut bytes[len - 12 * 1024..len - 4 * 1024] {
        *byte = 0xFF;
    }
    fs::write(path, bytes).unwrap();
}

#[test]
fn the_daily_copy_is_taken_once_a_day_is_consistent_and_keeps_seven() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    store.send_message(INBOX_ID, "запись", &at(T0)).unwrap();
    let first = store.run_maintenance(&at(T0)).unwrap();
    assert!(first.integrity_ok && first.backup_made);
    assert_eq!(backup_files(dir.path()), [daily_name("2026-10-21")]);
    // A second pass on the same day does not take a copy.
    assert!(
        !store
            .run_maintenance(&at(T0 + 10 * MINUTE))
            .unwrap()
            .backup_made
    );
    assert_eq!(backup_files(dir.path()).len(), 1);

    for day in 1..=9 {
        store
            .send_message(INBOX_ID, &format!("день {day}"), &at(T0 + day * DAY_MS))
            .unwrap();
        assert!(
            store
                .run_maintenance(&at(T0 + day * DAY_MS))
                .unwrap()
                .backup_made
        );
    }
    let files = backup_files(dir.path());
    assert_eq!(files.len(), 7, "{files:?}");
    assert_eq!(files.first().unwrap(), &daily_name("2026-10-24"));
    assert_eq!(files.last().unwrap(), &daily_name("2026-10-30"));
    // Foreign files in the folder are not touched.
    fs::write(backups_dir(&db_path(dir.path())).join("notes.txt"), "моё").unwrap();
    store
        .send_message(INBOX_ID, "ещё", &at(T0 + 10 * DAY_MS))
        .unwrap();
    store.run_maintenance(&at(T0 + 10 * DAY_MS)).unwrap();
    assert!(backup_files(dir.path()).contains(&"notes.txt".to_string()));

    let status = store.get_settings().unwrap().backup;
    assert_eq!(status.count, 7);
    assert!(status.last_at.is_some() && status.error.is_none());
    assert_eq!(
        status.folder.as_deref(),
        Some(backups_dir(&db_path(dir.path())).to_str().unwrap())
    );
    assert_eq!(
        backups(&db_path(dir.path()))
            .iter()
            .filter(|b| b.kind == BackupKind::Daily)
            .count(),
        7
    );
}

#[test]
fn the_copy_is_consistent_while_the_writer_keeps_writing() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    for i in 0..200 {
        store
            .send_message(INBOX_ID, &"x".repeat(2000 + i), &at(T0 + i as i64))
            .unwrap();
    }
    let writer = {
        let store = store.clone();
        std::thread::spawn(move || {
            for i in 0..150 {
                store
                    .send_message(INBOX_ID, &format!("пишу {i}"), &at(T0 + 1000 + i))
                    .unwrap();
            }
        })
    };
    for day in 0..4 {
        let report = store.run_maintenance(&at(T0 + day * DAY_MS)).unwrap();
        assert!(report.backup_made);
    }
    writer.join().unwrap();
    for name in backup_files(dir.path()) {
        let conn =
            rusqlite::Connection::open(backups_dir(&db_path(dir.path())).join(&name)).unwrap();
        let verdict: String = conn
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))
            .unwrap();
        assert_eq!(verdict, "ok", "{name}");
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM message", [], |r| r.get(0))
            .unwrap();
        assert!((200..=350).contains(&n), "{name}: {n}");
        // The copy is consistent: HLC and the entry columns are in place.
        let broken: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM message WHERE groups_raw = '' OR hlc_json = ''",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(broken, 0);
    }
}

#[test]
fn a_damaged_database_is_found_in_the_background_and_no_copy_is_taken() {
    let dir = tempfile::tempdir().unwrap();
    {
        let store = open_without_mirror(dir.path());
        for i in 0..300 {
            store
                .send_message(INBOX_ID, &"текст ".repeat(100 + i), &at(T0 + i as i64))
                .unwrap();
        }
        // There is a good copy from a previous day: the spoiled one will not push it out.
        store.run_maintenance(&at(T0 - DAY_MS)).unwrap();
    }
    wait_closed(dir.path());
    damage_pages(&db_path(dir.path()));
    let calls = Arc::new(AtomicUsize::new(0));
    let mut opts = options(dir.path());
    let counter = calls.clone();
    opts.on_corruption = Some(CorruptionHook(Arc::new(move || {
        counter.fetch_add(1, Ordering::SeqCst);
    })));
    let store = Store::open_at(opts, &at(T0)).unwrap();

    let report = store.run_maintenance(&at(T0)).unwrap();
    assert!(!report.integrity_ok && !report.backup_made);
    assert_eq!(
        backup_files(dir.path()),
        [daily_name("2026-10-20")],
        "no new copy"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    store.run_maintenance(&at(T0 + MINUTE)).unwrap();
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "the signal is given once per launch"
    );
    assert_eq!(
        store.get_settings().unwrap().backup.error,
        Some(BackupError::IntegrityFailed)
    );
}
