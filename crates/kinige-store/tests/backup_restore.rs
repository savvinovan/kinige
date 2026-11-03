//! Restoring from a copy (slice 3), a continuation of `backup.rs`: the copy, the corrupted file kept alongside, a new `device_id`, refusal on
//! a foreign or broken copy.

// The root of the integration tests is entirely test code: `cfg(test)` gives helpers outside #[test] the same
// clippy.toml relaxations (unwrap/expect/panic/indexing) as the tests in src/.
#![cfg(test)]

mod common;

use common::ops::*;
use common::*;
use kinige_domain::backup::daily_name;
use kinige_domain::groups::INBOX_ID;
use kinige_domain::{BackupKind, ErrorCode};
use kinige_store::{Now, Store, StoreError, backups, backups_dir};
use std::fs;
use std::path::{Path, PathBuf};

fn at(ms: i64) -> Now {
    moscow(ms)
}

fn make_database_with_copy(dir: &Path) -> (String, PathBuf) {
    let store = open_without_mirror(dir);
    store.send_message(INBOX_ID, "из копии", &at(T0)).unwrap();
    store.run_maintenance(&at(T0)).unwrap();
    // After the copy an entry appeared that the copy does not have.
    store
        .send_message(INBOX_ID, "после копии", &at(T0 + MINUTE))
        .unwrap();
    let device: String = raw(dir)
        .query_row(
            "SELECT value FROM sync_state WHERE key = 'device_id'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    drop(store);
    wait_closed(dir);
    (device, backups_dir(&db_path(dir)))
}

#[test]
fn restoring_keeps_the_broken_file_beside_and_creates_a_new_device_id() {
    let dir = tempfile::tempdir().unwrap();
    let (old_device, backups) = make_database_with_copy(dir.path());
    // The file header is spoiled (the other pages are intact): SQLite answers "not a database".
    let mut broken_bytes = fs::read(db_path(dir.path())).unwrap();
    broken_bytes[..64].copy_from_slice(&[0xAB; 64]);
    fs::write(db_path(dir.path()), &broken_bytes).unwrap();

    // The spoiled file does not open: the code for the "Database is damaged" window.
    let error = Store::open_at(options(dir.path()), &at(T0)).err().unwrap();
    assert!(matches!(error, StoreError::Corrupt(_)), "{error}");
    assert_eq!(
        fs::read(db_path(dir.path())).unwrap(),
        broken_bytes,
        "nothing is touched"
    );

    kinige_store::mark_needs_recovery(&db_path(dir.path())).unwrap();
    assert!(kinige_store::needs_recovery(&db_path(dir.path())));
    let list = backups_list(dir.path());
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, daily_name("2026-10-21"));
    assert_eq!(list[0].kind, BackupKind::Daily);

    kinige_store::restore_backup(&db_path(dir.path()), &list[0].name, 1_700_000_000_000, None)
        .unwrap();

    // The corrupted file is kept alongside, not deleted.
    let kept = dir.path().join("corrupt-1700000000000.db");
    assert_eq!(fs::read(&kept).unwrap(), broken_bytes);
    assert!(!kinige_store::needs_recovery(&db_path(dir.path())));
    // The database from the copy: the entry "после копии" is lost, "из копии" is in place, device_id is new.
    let store = open_without_mirror(dir.path());
    let page = store
        .list_messages(INBOX_ID, None, 10, &at(T0 + 2 * MINUTE))
        .unwrap();
    assert_eq!(
        page.messages
            .iter()
            .map(|m| m.body_md.as_str())
            .collect::<Vec<_>>(),
        ["из копии"]
    );
    let new_device: String = raw(dir.path())
        .query_row(
            "SELECT value FROM sync_state WHERE key = 'device_id'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_ne!(new_device, old_device);
    assert!(kinige_domain::hlc::is_uuid(&new_device));
    // The copy stayed in the copies folder and did not change after the restore.
    assert_eq!(backups.read_dir().unwrap().flatten().count(), 1);
    // A new entry gets an HLC stamp with the new device_id, greater than all earlier ones.
    let sent = store
        .send_message(INBOX_ID, "после восстановления", &at(T0 + 3 * MINUTE))
        .unwrap();
    let h = group_hlc(dir.path(), "message", &sent.id, "core");
    assert!(h.ends_with(&new_device));
}

fn backups_list(dir: &Path) -> Vec<kinige_domain::BackupInfo> {
    backups(&db_path(dir))
}

#[test]
fn restoring_from_a_missing_foreign_or_broken_copy_touches_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (_, backups) = make_database_with_copy(dir.path());
    let before = fs::read(db_path(dir.path())).unwrap();
    fs::write(backups.join("daily-2026-10-01.db"), b"not a database").unwrap();
    fs::write(backups.join("notes.txt"), b"x").unwrap();
    for name in [
        "daily-2026-10-02.db",
        "daily-2026-10-01.db",
        "notes.txt",
        "../kinige.db",
        "..",
        "",
        "daily-2026-10-21.db/../../kinige.db",
    ] {
        let error = kinige_store::restore_backup(&db_path(dir.path()), name, 1, None).unwrap_err();
        assert!(
            matches!(error, StoreError::BackupNotFound),
            "{name}: {error}"
        );
        assert_eq!(error.code(), ErrorCode::BackupNotFound);
    }
    assert_eq!(fs::read(db_path(dir.path())).unwrap(), before);
    assert!(!dir.path().join("corrupt-1.db").exists());
}

/// Regression (slice 10a): the restored copy must not inherit the read-only attribute or mode of a read-only backup.
/// `fs::copy` carries it over, which made a restore of a 0444 backup produce a read-only database (and, on Windows, fail
/// at `FlushFileBuffers`).
#[test]
fn a_read_only_backup_restores_into_a_writable_database() {
    let dir = tempfile::tempdir().unwrap();
    make_database_with_copy(dir.path());
    let list = backups_list(dir.path());
    let backup_file = backups_dir(&db_path(dir.path())).join(&list[0].name);
    let mut permissions = fs::metadata(&backup_file).unwrap().permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&backup_file, permissions).unwrap();

    kinige_store::restore_backup(&db_path(dir.path()), &list[0].name, 1_700_000_000_000, None)
        .unwrap();

    assert!(
        !fs::metadata(db_path(dir.path()))
            .unwrap()
            .permissions()
            .readonly(),
        "the restored database is writable"
    );
    let store = open_without_mirror(dir.path());
    store
        .send_message(INBOX_ID, "after the restore", &at(T0 + 3 * MINUTE))
        .unwrap();
    // Leave the backup deletable for the temporary directory cleanup.
    let mut permissions = fs::metadata(&backup_file).unwrap().permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    permissions.set_readonly(false);
    fs::set_permissions(&backup_file, permissions).unwrap();
}
