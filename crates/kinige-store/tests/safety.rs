//! Ревью guardian: обслуживание не останавливается из-за корзины, проверка целостности не даёт ложного повреждения, запуск с
//! отметкой, гонка и прерывание восстановления, Markdown-копия после восстановления, копия под непрерывной записью, purge
//! записи с группой `deleted` в неподдерживаемой `sv`.

mod common;

use common::ops::*;
use common::*;
use kinige_domain::backup::daily_name;
use kinige_domain::groups::INBOX_ID;
use kinige_domain::history::DAY_MS;
use kinige_domain::{BackupError, ErrorCode};
use kinige_store::{Now, StoreError, backups_dir};

fn at(ms: i64) -> Now {
    moscow(ms)
}

// ---------- обслуживание ----------

#[test]
fn a_broken_trash_entry_does_not_stop_the_integrity_check_and_the_daily_copy() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let broken = store.send_message(INBOX_ID, "сломана", &at(T0)).unwrap();
    let fine = store.send_message(INBOX_ID, "цела", &at(T0)).unwrap();
    for id in [&broken.id, &fine.id] {
        store.delete_message(id, &at(T0 + 1000)).unwrap();
    }
    raw(dir.path())
        .execute(
            "UPDATE message SET hlc_json = '{' WHERE id = ?1",
            [&broken.id],
        )
        .unwrap();

    let report = store
        .run_maintenance(&at(T0 + 40 * DAY_MS))
        .expect("обслуживание не прерывается");
    assert_eq!(report.purged, 1, "целая запись удалена");
    assert!(report.integrity_ok && report.backup_made, "{report:?}");
    assert!(
        backups_dir(&db_path(dir.path()))
            .join(daily_name("2026-11-30"))
            .is_file()
    );
    // Ошибка видна в «Настройках»; повреждённая запись осталась в корзине.
    assert_eq!(
        store.get_settings().unwrap().backup.error,
        Some(BackupError::PurgeFailed)
    );
    assert_eq!(count_rows(dir.path(), "message"), 1);
    // Следующий проход тоже доходит до проверки; ошибка остаётся, пока запись не убрана.
    let again = store.run_maintenance(&at(T0 + 41 * DAY_MS)).unwrap();
    assert!(again.backup_made);
    assert_eq!(
        store.get_settings().unwrap().backup.error,
        Some(BackupError::PurgeFailed)
    );
}

#[test]
fn a_corrupt_trash_entry_can_be_purged_by_hand_when_something_is_readable() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let sent = store.send_message(INBOX_ID, "частично", &at(T0)).unwrap();
    store.edit_message(&sent.id, "правка", &at(T0 + 1)).unwrap();
    store.delete_message(&sent.id, &at(T0 + 2)).unwrap();
    // Одна группа в `hlc_json` не читается, остальные целы; байты групп сломаны целиком.
    let mut hlcs = hlc_entries(dir.path(), "message", &sent.id);
    hlcs.insert("flags".into(), "123".into());
    let text = format!(
        "{{{}}}",
        hlcs.iter()
            .map(|(k, v)| format!("\"{k}\":{v}"))
            .collect::<Vec<_>>()
            .join(",")
    );
    raw(dir.path())
        .execute(
            "UPDATE message SET hlc_json = ?1, groups_raw = '{не json' WHERE id = ?2",
            (text, &sent.id),
        )
        .unwrap();

    store.purge_message(&sent.id, &at(T0 + 3)).unwrap();
    assert_eq!(count_rows(dir.path(), "message"), 0);
    let snapshot: String = raw(dir.path())
        .query_row(
            "SELECT purged_of FROM tombstone WHERE entity = 'message' AND id = ?1",
            [&sent.id],
            |r| r.get(0),
        )
        .unwrap();
    assert!(
        snapshot.contains("\"body\":") && !snapshot.contains("\"flags\""),
        "{snapshot}"
    );
    assert_eq!(count_rows(dir.path(), "message_revision"), 0);
    assert_eq!(count_rows(dir.path(), "tombstone"), 2, "запись и ревизия");
}

#[test]
fn a_hopelessly_corrupt_entry_stays_in_the_trash_with_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let sent = store.send_message(INBOX_ID, "ничего", &at(T0)).unwrap();
    store.delete_message(&sent.id, &at(T0 + 1)).unwrap();
    raw(dir.path())
        .execute(
            "UPDATE message SET hlc_json = '{' WHERE id = ?1",
            [&sent.id],
        )
        .unwrap();
    let error = store.purge_message(&sent.id, &at(T0 + 2)).unwrap_err();
    assert_eq!(error.code(), ErrorCode::Internal);
    assert_eq!(count_rows(dir.path(), "message"), 1);
    assert_eq!(count_rows(dir.path(), "tombstone"), 0, "ничего не записано");
    assert_eq!(
        store.list_trash(&at(T0 + 3)).unwrap().len(),
        1,
        "видна в корзине"
    );
}

#[test]
fn an_entry_with_a_newer_deleted_group_is_not_purged() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let sent = store.send_message(INBOX_ID, "новее", &at(T0)).unwrap();
    store.delete_message(&sent.id, &at(T0 + 1)).unwrap();
    edit_raw_groups(dir.path(), &sent.id, |_, hlcs| {
        let h =
            serde_json::from_str::<serde_json::Value>(hlcs["deleted"].get()).unwrap()["h"].clone();
        hlcs.insert(
            "deleted".into(),
            serde_json::value::RawValue::from_string(
                serde_json::json!({"h": h, "sv": 2}).to_string(),
            )
            .unwrap(),
        );
    });
    let error = store.purge_message(&sent.id, &at(T0 + 2)).unwrap_err();
    assert!(matches!(error, StoreError::ReadOnly), "{error}");
    assert_eq!(
        store.purge_expired(&at(T0 + 60 * DAY_MS)).unwrap(),
        0,
        "автоудаление пропускает запись молча"
    );
    assert_eq!(count_rows(dir.path(), "message"), 1);
    assert_eq!(count_rows(dir.path(), "tombstone"), 0);
}
