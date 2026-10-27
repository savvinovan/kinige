//! Восстановление из копии (слайс 3), продолжение `backup.rs`: копия, повреждённый файл рядом, новый `device_id`, отказ при
//! чужой или битой копии.

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
    // После копии появилась запись, которой в копии нет.
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
    // Заголовок файла испорчен (остальные страницы целы): SQLite отвечает «не база данных».
    let mut broken_bytes = fs::read(db_path(dir.path())).unwrap();
    broken_bytes[..64].copy_from_slice(&[0xAB; 64]);
    fs::write(db_path(dir.path()), &broken_bytes).unwrap();

    // Испорченный файл не открывается: код для окна «База повреждена».
    let error = Store::open_at(options(dir.path()), &at(T0)).err().unwrap();
    assert!(matches!(error, StoreError::Corrupt(_)), "{error}");
    assert_eq!(
        fs::read(db_path(dir.path())).unwrap(),
        broken_bytes,
        "ничего не тронуто"
    );

    kinige_store::mark_needs_recovery(&db_path(dir.path())).unwrap();
    assert!(kinige_store::needs_recovery(&db_path(dir.path())));
    let list = backups_list(dir.path());
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, daily_name("2026-10-21"));
    assert_eq!(list[0].kind, BackupKind::Daily);

    kinige_store::restore_backup(&db_path(dir.path()), &list[0].name, 1_700_000_000_000, None)
        .unwrap();

    // Повреждённый файл сохранён рядом, не удалён.
    let kept = dir.path().join("corrupt-1700000000000.db");
    assert_eq!(fs::read(&kept).unwrap(), broken_bytes);
    assert!(!kinige_store::needs_recovery(&db_path(dir.path())));
    // База из копии: запись «после копии» потеряна, «из копии» на месте, device_id новый.
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
    // Копия осталась в папке копий и после восстановления не изменилась.
    assert_eq!(backups.read_dir().unwrap().flatten().count(), 1);
    // Новая запись получает метку HLC с новым device_id и больше всех прежних.
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
