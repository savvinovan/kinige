//! Копия перед импортом (слайс 4, задача 4.7): имя и вид, согласованность при работающем писателе, ротация, недоступная
//! папка.

mod common;

use common::*;
use kinige_domain::BackupKind;
use kinige_domain::ErrorCode;
use kinige_domain::backup::classify;
use kinige_domain::groups::INBOX_ID;
use kinige_store::{StoreError, backups, backups_dir, utc_stamp};
use std::fs;

#[test]
fn copy_is_named_listed_and_consistent_with_the_database() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    for i in 0..3 {
        store
            .send_message(INBOX_ID, &format!("запись {i}"), &moscow(T0 + i * MINUTE))
            .unwrap();
    }
    let now = moscow(T0 + 10 * MINUTE);
    let name = store.backup_before_import(&now).unwrap();
    assert_eq!(name, format!("pre-import-{}.db", utc_stamp(now.ms)));
    assert_eq!(classify(&name), Some(BackupKind::PreImport));

    let listed = backups(&db_path(dir.path()));
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].name, name);
    assert_eq!(listed[0].kind, BackupKind::PreImport);

    let copy = backups_dir(&db_path(dir.path())).join(&name);
    let conn = rusqlite::Connection::open(&copy).unwrap();
    let verdict: String = conn
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(verdict, "ok");
    let count: i64 = conn
        .query_row("SELECT count(*) FROM message", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 3);
    let mode: String = conn
        .query_row("PRAGMA journal_mode", [], |r| r.get(0))
        .unwrap();
    assert_eq!(mode, "delete", "копия — один файл без -wal");
}

#[test]
fn two_copies_in_the_same_second_keep_both_files() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    store.send_message(INBOX_ID, "до", &moscow(T0)).unwrap();
    let now = moscow(T0 + MINUTE);
    let first = store.backup_before_import(&now).unwrap();
    store
        .send_message(INBOX_ID, "между", &moscow(T0 + MINUTE))
        .unwrap();
    let second = store.backup_before_import(&now).unwrap();
    assert_ne!(first, second, "вторая копия не затирает первую");
    assert_eq!(classify(&second), Some(BackupKind::PreImport));
    let count = |name: &str| -> i64 {
        rusqlite::Connection::open(backups_dir(&db_path(dir.path())).join(name))
            .unwrap()
            .query_row("SELECT count(*) FROM message", [], |r| r.get(0))
            .unwrap()
    };
    assert_eq!((count(&first), count(&second)), (1, 2));
}

#[test]
fn copy_is_taken_while_the_writer_keeps_writing() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    store.send_message(INBOX_ID, "первая", &moscow(T0)).unwrap();
    let writer = {
        let store = store.clone();
        std::thread::spawn(move || {
            for i in 0..200 {
                store
                    .send_message(INBOX_ID, &format!("фон {i}"), &moscow(T0 + MINUTE + i))
                    .unwrap();
            }
        })
    };
    let name = store
        .backup_before_import(&moscow(T0 + 5 * MINUTE))
        .unwrap();
    writer.join().unwrap();
    let copy = backups_dir(&db_path(dir.path())).join(name);
    let conn = rusqlite::Connection::open(copy).unwrap();
    let verdict: String = conn
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(verdict, "ok");
}

#[test]
fn only_the_last_three_pre_import_copies_stay() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let mut names = Vec::new();
    for i in 0..5 {
        names.push(
            store
                .backup_before_import(&moscow(T0 + i * MINUTE))
                .unwrap(),
        );
    }
    let mut left: Vec<String> = backups(&db_path(dir.path()))
        .into_iter()
        .map(|b| b.name)
        .collect();
    left.sort();
    assert_eq!(left, names[2..]);
}

#[test]
fn unavailable_folder_is_an_error_and_the_database_is_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    store.send_message(INBOX_ID, "до", &moscow(T0)).unwrap();
    // Вместо папки копий лежит файл: создать папку нельзя.
    fs::write(backups_dir(&db_path(dir.path())), b"not a folder").unwrap();
    let error = store
        .backup_before_import(&moscow(T0 + MINUTE))
        .unwrap_err();
    assert!(matches!(error, StoreError::ImportBackupFailed));
    assert_eq!(error.code(), ErrorCode::ImportBackupFailed);
    let count: i64 = raw(dir.path())
        .query_row("SELECT count(*) FROM message", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1);
}
