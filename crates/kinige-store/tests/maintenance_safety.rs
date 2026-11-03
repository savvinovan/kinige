//! Maintenance and integrity (repeated guardian review, a continuation of `restore_safety.rs`): a check once a day, after
//! a finding nothing is written to the database, `verify_database` only reads, auto-delete is not blocked by skipped entries,
//! restoring from a schema 2 copy migrates the database.

// The root of the integration tests is entirely test code: `cfg(test)` gives helpers outside #[test] the same
// clippy.toml relaxations (unwrap/expect/panic/indexing) as the tests in src/.
#![cfg(test)]

mod common;

use common::ops::*;
use common::*;
use kinige_domain::backup::pre_migration_name;
use kinige_domain::groups::INBOX_ID;
use kinige_domain::history::DAY_MS;
use kinige_store::{CorruptionHook, Integrity, Now, Store, backups, backups_dir, verify_database};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

fn at(ms: i64) -> Now {
    moscow(ms)
}

/// Spoils pages at the tail of the file: the integrity check sees it, opening the database passes.
fn damage_pages(path: &Path) {
    let mut bytes = fs::read(path).unwrap();
    let len = bytes.len();
    assert!(len > 40 * 1024, "enough pages: {len}");
    for byte in &mut bytes[len - 12 * 1024..len - 4 * 1024] {
        *byte = 0xFF;
    }
    fs::write(path, bytes).unwrap();
}

/// A database of more than 256 pages: it has a "tail" that can be spoiled.
fn big_database(dir: &Path, options: kinige_store::StoreOptions) -> Store {
    let store = Store::open_at(options, &at(T0)).unwrap();
    for i in 0..300 {
        store
            .send_message(INBOX_ID, &"текст ".repeat(100 + i), &at(T0 + i as i64))
            .unwrap();
    }
    let _ = dir;
    store
}

// ---------- integrity check and maintenance ----------

#[test]
fn the_second_run_of_a_day_does_not_check_integrity_again() {
    let dir = tempfile::tempdir().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let mut opts = options(dir.path());
    let counter = calls.clone();
    opts.on_corruption = Some(CorruptionHook(Arc::new(move || {
        counter.fetch_add(1, Ordering::SeqCst);
    })));
    let store = big_database(dir.path(), opts);
    assert!(store.run_maintenance(&at(T0)).unwrap().integrity_ok);
    wait_closed_keep_open(dir.path());
    damage_pages(&db_path(dir.path()));

    // The same day: the check is not repeated (otherwise it would find the corruption), no signal.
    let report = store.run_maintenance(&at(T0 + MINUTE)).unwrap();
    assert!(
        report.integrity_ok,
        "a check once a day: there was one today already"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    // The next day: the check runs and finds it.
    let next = store.run_maintenance(&at(T0 + DAY_MS)).unwrap();
    assert!(!next.integrity_ok);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

/// A WAL checkpoint without closing the store: the database pages lie in the main file.
fn wait_closed_keep_open(dir: &Path) {
    let conn = raw(dir);
    let _ = conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()));
}

#[test]
fn after_corruption_is_found_nothing_is_written_not_even_the_trash_cleanup() {
    let dir = tempfile::tempdir().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let mut opts = options(dir.path());
    opts.start_mirror = false;
    let counter = calls.clone();
    opts.on_corruption = Some(CorruptionHook(Arc::new(move || {
        counter.fetch_add(1, Ordering::SeqCst);
    })));
    let store = big_database(dir.path(), opts);
    let sent = store.send_message(INBOX_ID, "в корзину", &at(T0)).unwrap();
    store.delete_message(&sent.id, &at(T0 + 1)).unwrap();
    wait_closed_keep_open(dir.path());
    damage_pages(&db_path(dir.path()));
    let later = T0 + 40 * DAY_MS;

    let first = store.run_maintenance(&at(later)).unwrap();
    assert!(!first.integrity_ok);
    assert_eq!(
        first.purged, 0,
        "auto-delete runs only after an \"intact\" verdict"
    );
    let second = store.run_maintenance(&at(later + MINUTE)).unwrap();
    assert!(!second.integrity_ok && second.purged == 0 && !second.backup_made);

    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(count_rows(dir.path(), "tombstone"), 0);
    let in_trash: i64 = raw(dir.path())
        .query_row(
            "SELECT COUNT(*) FROM message WHERE deleted_at IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(in_trash, 1, "the entry stayed in the trash");
}

#[test]
fn verifying_a_database_only_reads_it() {
    let dir = tempfile::tempdir().unwrap();
    drop(open_without_mirror(dir.path()));
    wait_closed(dir.path());
    let db = db_path(dir.path());
    let bytes = fs::read(&db).unwrap();
    let names = || {
        let mut n: Vec<String> = fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .map(|e| e.file_name().into_string().unwrap())
            .collect();
        n.sort();
        n
    };
    let before = names();

    assert_eq!(verify_database(&db), Integrity::Ok);

    assert_eq!(fs::read(&db).unwrap(), bytes, "the file is unchanged");
    // For a database reader in WAL mode SQLite itself creates `-shm` and `-wal`; nothing else should appear.
    let created: Vec<String> = names()
        .into_iter()
        .filter(|n| !before.contains(n) && !n.ends_with("-shm") && !n.ends_with("-wal"))
        .collect();
    assert!(created.is_empty(), "{created:?}");
    // No file: the check does not create an empty database.
    let missing = dir.path().join("none.db");
    assert_eq!(verify_database(&missing), Integrity::Corrupt);
    assert!(!missing.exists());
    // Not a database.
    let junk: PathBuf = dir.path().join("junk.db");
    fs::write(&junk, vec![0xAB; 8192]).unwrap();
    assert_eq!(verify_database(&junk), Integrity::Corrupt);
}

// ---------- auto-delete: skipped entries do not block healthy ones ----------

#[test]
fn more_than_a_page_of_skipped_entries_does_not_block_the_healthy_ones_behind_them() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let mut skipped = Vec::new();
    for i in 0..230 {
        let sent = store
            .send_message(INBOX_ID, &format!("новее {i}"), &at(T0 + i))
            .unwrap();
        store.delete_message(&sent.id, &at(T0 + 1000 + i)).unwrap();
        skipped.push(sent.id);
    }
    let mut healthy = Vec::new();
    for i in 0..5 {
        let sent = store
            .send_message(INBOX_ID, &format!("здоровая {i}"), &at(T0 + 500 + i))
            .unwrap();
        // Deleted later than all the skipped ones: in the selection they stand behind them.
        store.delete_message(&sent.id, &at(T0 + 5000 + i)).unwrap();
        healthy.push(sent.id);
    }
    drop(store);
    wait_closed(dir.path());
    // The first 230 have a `deleted` group from a future version: auto-delete skips them.
    let conn = raw(dir.path());
    for id in &skipped {
        let hlc: String = conn
            .query_row("SELECT hlc_json FROM message WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .unwrap();
        let mut value: serde_json::Value = serde_json::from_str(&hlc).unwrap();
        value["deleted"]["sv"] = serde_json::json!(2);
        conn.execute(
            "UPDATE message SET hlc_json = ?1 WHERE id = ?2",
            (value.to_string(), id),
        )
        .unwrap();
    }
    drop(conn);

    let store = open_without_mirror(dir.path());
    let purged = store.purge_expired(&at(T0 + 60 * DAY_MS)).unwrap();

    assert_eq!(
        purged, 5,
        "healthy entries behind the 230 skipped ones are deleted"
    );
    assert_eq!(count_rows(dir.path(), "tombstone"), 5);
    assert_eq!(count_rows(dir.path(), "message"), 230);
    let _ = healthy;
}

// ---------- restoring from a schema 2 copy ----------

#[test]
fn restoring_a_pre_migration_copy_of_schema_two_migrates_the_database_on_open() {
    let dir = tempfile::tempdir().unwrap();
    {
        let store = open_without_mirror(dir.path());
        store
            .send_message(INBOX_ID, "до миграции #старая", &at(T0))
            .unwrap();
    }
    wait_closed(dir.path());
    // A slice 2 database.
    let conn = raw(dir.path());
    conn.execute_batch(
        "DROP INDEX message_deleted; DROP TABLE message_revision; DROP TABLE tombstone; DROP TABLE message_tag; \
         DROP TABLE mirror_chat; DROP TABLE message_ext; DROP TABLE search_fts; DROP TABLE search_map; \
         DROP TABLE mcp_log; DROP TABLE mcp_log_message; ALTER TABLE draft DROP COLUMN reply_to_id; \
         DROP TABLE attachment; DROP TABLE attachment_local; DROP TABLE draft_attachment; DROP TABLE mirror_file; \
         DELETE FROM app_setting WHERE key = 'search_index_version';",
    )
    .unwrap();
    conn.pragma_update(None, "user_version", 2).unwrap();
    drop(conn);
    // The migration takes a "before update" copy, after which "disaster strikes" the database.
    drop(Store::open_at(options(dir.path()), &at(T0 + 5 * MINUTE)).unwrap());
    wait_closed(dir.path());
    let copy = pre_migration_name(2, &kinige_store::utc_stamp(T0 + 5 * MINUTE));
    assert!(backups_dir(&db_path(dir.path())).join(&copy).is_file());
    fs::write(db_path(dir.path()), b"broken").unwrap();

    kinige_store::restore_backup(&db_path(dir.path()), &copy, T0 + 10 * MINUTE, None).unwrap();
    // What `resume_after_restore` does: it opens the restored database (schema 2), which migrates to the latest.
    let store = Store::open_at(options(dir.path()), &at(T0 + 10 * MINUTE)).unwrap();

    let version: i64 = raw(dir.path())
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, kinige_store::LATEST_VERSION);
    assert_eq!(count_rows(dir.path(), "message"), 1);
    assert_eq!(
        count_rows(dir.path(), "search_map"),
        1,
        "the index is built"
    );
    assert_eq!(
        count_rows(dir.path(), "message_tag"),
        1,
        "tags are recomputed"
    );
    assert_eq!(store.list_chats().unwrap()[0].count, 1);
    assert!(
        backups(&db_path(dir.path()))
            .iter()
            .filter(|b| b.name.starts_with("pre-migration-v2-"))
            .count()
            >= 1
    );
}

// ---------- corruption found by the command, and a copy with corruption that `quick_check` does not see ----------

fn counting_options(dir: &Path, calls: &Arc<AtomicUsize>) -> kinige_store::StoreOptions {
    let mut opts = options(dir);
    opts.start_mirror = false;
    let counter = calls.clone();
    opts.on_corruption = Some(CorruptionHook(Arc::new(move || {
        counter.fetch_add(1, Ordering::SeqCst);
    })));
    opts
}

#[test]
fn a_read_that_hits_a_damaged_page_raises_the_hook_once_and_stops_the_maintenance_writes() {
    let dir = tempfile::tempdir().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let store = Store::open_at(counting_options(dir.path(), &calls), &at(T0)).unwrap();
    let sent = store.send_message(INBOX_ID, "в корзину", &at(T0)).unwrap();
    store.delete_message(&sent.id, &at(T0 + 1)).unwrap();
    wait_closed_keep_open(dir.path());
    let mut bytes = fs::read(db_path(dir.path())).unwrap();
    for byte in &mut bytes[4096..] {
        *byte = 0xFF;
    }
    fs::write(db_path(dir.path()), bytes).unwrap();

    let error = store.get_settings().unwrap_err();
    assert!(
        matches!(error, kinige_store::StoreError::Corrupt(_)),
        "{error}"
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "the same signal as the background check"
    );
    assert!(store.get_settings().is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 1, "once per launch");
    // After a finding, maintenance writes nothing to the database (trash auto-delete, the day's copy).
    let report = store.run_maintenance(&at(T0 + 40 * DAY_MS)).unwrap();
    assert!(!report.integrity_ok && report.purged == 0 && !report.backup_made);
}

#[test]
fn a_restored_copy_with_a_damaged_index_passes_quick_check_and_the_first_pass_raises_the_hook() {
    let dir = tempfile::tempdir().unwrap();
    {
        let store = open_without_mirror(dir.path());
        for i in 0..5 {
            store
                .send_message(INBOX_ID, &format!("запись {i}"), &at(T0 + i))
                .unwrap();
        }
        store.run_maintenance(&at(T0)).unwrap();
    }
    wait_closed(dir.path());
    let name = backups(&db_path(dir.path()))[0].name.clone();
    // Corruption of the `message_day` index right in the copy file: the index key diverges from the table. `quick_check` does not
    // compare indexes with the table and passes, `integrity_check` does not.
    let copy = backups_dir(&db_path(dir.path())).join(&name);
    let root: i64 = rusqlite::Connection::open(&copy)
        .unwrap()
        .query_row(
            "SELECT rootpage FROM sqlite_master WHERE name = 'message_day'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let mut bytes = fs::read(&copy).unwrap();
    let page = (root as usize - 1) * 4096;
    let at_key = bytes[page..page + 4096]
        .windows(8)
        .position(|w| w == b"2026-10-")
        .expect("day key in the index page");
    bytes[page + at_key + 9] = b'9';
    fs::write(&copy, bytes).unwrap();
    let verdict: String = rusqlite::Connection::open(&copy)
        .unwrap()
        .query_row("PRAGMA quick_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(verdict, "ok", "the copy passes the quick check");
    fs::write(db_path(dir.path()), b"broken").unwrap();

    kinige_store::restore_backup(&db_path(dir.path()), &name, T0 + MINUTE, None).unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let store = Store::open_at(counting_options(dir.path(), &calls), &at(T0 + MINUTE)).unwrap();
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "the corruption is not visible on open"
    );

    let report = store.run_maintenance(&at(T0 + MINUTE)).unwrap();

    assert!(
        !report.integrity_ok,
        "the first maintenance pass finds the corruption"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn a_command_that_finds_corruption_also_shows_it_in_the_settings_row() {
    let dir = tempfile::tempdir().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let store = Store::open_at(counting_options(dir.path(), &calls), &at(T0)).unwrap();
    for i in 0..5 {
        store
            .send_message(INBOX_ID, &format!("запись {i}"), &at(T0 + i))
            .unwrap();
    }
    wait_closed_keep_open(dir.path());
    // Only the root page of the `message` table is spoiled: settings are readable, entries are not.
    let root: i64 = raw(dir.path())
        .query_row(
            "SELECT rootpage FROM sqlite_master WHERE name = 'message'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let mut bytes = fs::read(db_path(dir.path())).unwrap();
    let page = (root as usize - 1) * 4096;
    for byte in &mut bytes[page..page + 4096] {
        *byte = 0xFF;
    }
    fs::write(db_path(dir.path()), bytes).unwrap();

    let error = store
        .list_messages(INBOX_ID, None, 50, &at(T0))
        .unwrap_err();
    assert!(
        matches!(error, kinige_store::StoreError::Corrupt(_)),
        "{error}"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    let settings = store.get_settings().unwrap();
    assert_eq!(
        settings.backup.error,
        Some(kinige_domain::BackupError::IntegrityFailed),
        "the row in Settings matches the \"Database is damaged\" window"
    );
}
