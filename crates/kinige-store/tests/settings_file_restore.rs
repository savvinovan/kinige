//! Восстановление из копии и файл настроек (ADR-0010): папка копии из файла уходит в `…-before-restore-…`, доступ агента из
//! копии не переносится, файл применяется поверх восстановленной базы.

mod common;

use common::ops::*;
use common::settings_file::*;
use common::*;
use kinige_domain::groups::INBOX_ID;
use kinige_store::{Now, Store, backups, restore_backup};
use std::fs;
use std::path::{Path, PathBuf};

const BACKUP_MS: i64 = 1_700_000_000_000;

fn at(ms: i64) -> Now {
    moscow(ms)
}

/// База с копией дня и записью; `prepare` настраивает хранилище до копии.
fn database_with_copy(dir: &Path, prepare: impl FnOnce(&Store)) -> String {
    let store = open_without_mirror(dir);
    store.send_message(INBOX_ID, "из копии", &at(T0)).unwrap();
    prepare(&store);
    store.run_maintenance(&at(T0)).unwrap();
    drop(store);
    wait_closed(dir);
    backups(&db_path(dir))[0].name.clone()
}

fn restore(dir: &Path, name: &str) {
    restore_backup(&db_path(dir), name, BACKUP_MS, Some(&mirror_dir(dir))).unwrap();
}

fn aside(folder: &Path) -> Vec<PathBuf> {
    let name = folder.file_name().unwrap().to_string_lossy().into_owned();
    let prefix = format!("{name}-before-restore-");
    fs::read_dir(folder.parent().unwrap())
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with(&prefix))
        .map(|e| e.path())
        .collect()
}

#[test]
fn the_mirror_folder_named_in_the_file_is_moved_aside_with_its_extra_day() {
    let dir = tempfile::tempdir().unwrap();
    let name = database_with_copy(dir.path(), |_| {});
    // Копия говорит «папка по умолчанию» (A), файл — другая папка (B) со днём, которого нет в копии.
    let elsewhere = tempfile::tempdir().unwrap();
    let folder_b = elsewhere.path().join("Kinige-B");
    let day = folder_b.join("Inbox").join("2026").join("2026-10-22.md");
    fs::create_dir_all(day.parent().unwrap()).unwrap();
    fs::write(&day, "запись после снимка").unwrap();
    write_file(
        dir.path(),
        serde_json::json!({"mirror": {"folder": folder_b}})
            .to_string()
            .as_bytes(),
        0o600,
    );
    restore(dir.path(), &name);
    assert!(
        !folder_b.exists(),
        "папка из файла ушла в сторону до применения файла"
    );
    let moved = aside(&folder_b);
    assert_eq!(moved.len(), 1);
    assert_eq!(
        fs::read_to_string(moved[0].join("Inbox/2026/2026-10-22.md")).unwrap(),
        "запись после снимка"
    );
}

#[test]
fn an_unsafe_file_does_not_make_restore_move_its_mirror_folder() {
    let dir = tempfile::tempdir().unwrap();
    let name = database_with_copy(dir.path(), |_| {});
    let elsewhere = tempfile::tempdir().unwrap();
    let folder_b = elsewhere.path().join("Kinige-B");
    fs::create_dir_all(&folder_b).unwrap();
    write_file(
        dir.path(),
        serde_json::json!({"mirror": {"folder": folder_b}})
            .to_string()
            .as_bytes(),
        0o666,
    );
    restore(dir.path(), &name);
    assert!(folder_b.exists());
    assert!(aside(&folder_b).is_empty());
}

#[test]
fn the_applied_file_after_restore_cannot_wipe_days_written_after_the_snapshot() {
    // Сквозной вариант ревью: после восстановления файл ставит папку B и помечает все дни на перезапись; писатель копии
    // удаляет в папке файлы дней, которых нет в базе. Папка B к этому времени уже ушла в сторону.
    let dir = tempfile::tempdir().unwrap();
    let name = database_with_copy(dir.path(), |_| {});
    let elsewhere = tempfile::tempdir().unwrap();
    let folder_b = elsewhere.path().join("Kinige-B");
    let day = folder_b.join("Inbox").join("2026").join("2026-10-22.md");
    fs::create_dir_all(day.parent().unwrap()).unwrap();
    fs::write(&day, "после снимка").unwrap();
    write_file(
        dir.path(),
        serde_json::json!({"mirror": {"folder": folder_b}})
            .to_string()
            .as_bytes(),
        0o600,
    );
    restore(dir.path(), &name);
    // Пользователь создал папку B заново (пустую): файл ставит её копией, писатель копии пишет туда дни из базы.
    fs::create_dir_all(&folder_b).unwrap();
    let store = open(dir.path());
    let rig = start_in(dir, store, |_| {});
    let today = folder_b.join("Inbox/2026/2026-10-21.md");
    wait_until("день из базы в новой папке", || {
        today.exists()
    });
    let moved = aside(&folder_b);
    assert_eq!(
        fs::read_to_string(moved[0].join("Inbox/2026/2026-10-22.md")).unwrap(),
        "после снимка"
    );
    assert!(!folder_b.join("Inbox/2026/2026-10-22.md").exists());
    drop(rig);
}

#[test]
fn access_from_the_copy_is_switched_off_and_an_unsafe_file_cannot_bring_it_back() {
    let dir = tempfile::tempdir().unwrap();
    let name = database_with_copy(dir.path(), |store| {
        store.set_agent_chats(vec![INBOX_ID.to_string()]).unwrap();
        store
            .set_agent_mode(kinige_domain::AgentMode::ReadWrite)
            .unwrap();
        store.set_agent_enabled(true).unwrap();
    });
    // Файл небезопасен (0644 + запись группе) и говорит «выключено»; даже без файла доступ из копии не должен остаться.
    write_file(dir.path(), br#"{"agent_access":{"enabled":false}}"#, 0o664);
    restore(dir.path(), &name);
    let store = open(dir.path());
    let access = store.agent_access().unwrap();
    assert!(!access.enabled, "доступ из копии не переносится");
    assert!(access.chats.iter().all(|c| !c.visible));
    let rig = start_in(dir, store, |_| {});
    assert!(!rig.store.agent_access().unwrap().enabled);
}

#[test]
fn after_restore_a_safe_file_is_applied_over_the_restored_database() {
    let dir = tempfile::tempdir().unwrap();
    let name = database_with_copy(dir.path(), |store| {
        store
            .set_text_size(kinige_domain::TextSize::Normal)
            .unwrap();
    });
    write_file(
        dir.path(),
        br#"{"text_size":"large","agent_access":{"enabled":true,"chats":[{"title":"Inbox"}]}}"#,
        0o600,
    );
    // Хэша в копии нет: файл применяется безусловно.
    restore(dir.path(), &name);
    let store = open(dir.path());
    let rig = start_in(dir, store, |_| {});
    assert_eq!(
        rig.store.get_settings().unwrap().text_size,
        kinige_domain::TextSize::Large
    );
    assert!(rig.store.agent_access().unwrap().enabled);
}

#[test]
fn a_null_folder_in_the_file_means_the_default_folder_and_it_is_moved_aside_too() {
    // В копии выбрана своя папка C, в файле `mirror.folder: null`: после применения файла копия пойдёт в папку по умолчанию,
    // где лежит день, появившийся после снимка. Её нужно увести так же, как папку из файла с путём.
    let dir = tempfile::tempdir().unwrap();
    let chosen = tempfile::tempdir().unwrap();
    let chosen_folder = chosen.path().join("Kinige-C");
    fs::create_dir(&chosen_folder).unwrap();
    let name = database_with_copy(dir.path(), |store| {
        store.set_mirror_dir(&chosen_folder).unwrap()
    });
    let default_folder = mirror_dir(dir.path());
    let day = default_folder.join("Inbox/2026/2026-10-22.md");
    fs::create_dir_all(day.parent().unwrap()).unwrap();
    fs::write(&day, "после снимка").unwrap();
    write_file(dir.path(), br#"{"mirror":{"folder":null}}"#, 0o600);
    restore(dir.path(), &name);
    assert!(
        !default_folder.exists(),
        "папка по умолчанию уведена в сторону"
    );
    let moved = aside(&default_folder);
    assert_eq!(moved.len(), 1);
    assert_eq!(
        fs::read_to_string(moved[0].join("Inbox/2026/2026-10-22.md")).unwrap(),
        "после снимка"
    );
    assert_eq!(
        aside(&chosen_folder).len(),
        1,
        "папка из копии тоже уведена"
    );
}

#[test]
fn a_null_folder_equal_to_the_copys_own_folder_is_moved_once() {
    let dir = tempfile::tempdir().unwrap();
    let name = database_with_copy(dir.path(), |_| {});
    let default_folder = mirror_dir(dir.path());
    fs::create_dir_all(default_folder.join("Inbox")).unwrap();
    write_file(dir.path(), br#"{"mirror":{"folder":null}}"#, 0o600);
    let report = restore_backup(
        &db_path(dir.path()),
        &name,
        BACKUP_MS,
        Some(&mirror_dir(dir.path())),
    )
    .unwrap();
    assert_eq!(aside(&default_folder).len(), 1);
    // Папка из файла совпала с папкой копии: уведена один раз и названа один раз, второй строки в отчёте нет.
    assert!(report.previous_mirror.is_some());
    assert_eq!(report.settings_file_mirror, None);
}
