//! Сквозное восстановление через IPC: `restore_backup` -> файлы -> `resume_after_restore` -> состояние `Recovery`. Отдельный
//! бинарник с одним тестом: тест задаёт `KINIGE_DATA_DIR`, а переменные окружения общие для всего процесса.
//!
//! **Успешный проход в mock-рантайме невозможен ни на macOS, ни на Linux.** `resume_after_restore` запускает сервисы:
//! на macOS первым идёт `panel::convert` (окно захвата как NSPanel — в mock-рантайме нет ни окна, ни плагина панелей), на
//! Linux — `tray::setup`, которому нужен инициализированный GTK (`gtk::Menu` паникует без дисплея). Поэтому здесь
//! проверяется то, что можно: **сбой сервисов не оставляет приложение с хранилищем под флагом «журнал не открыт»**
//! (хранилище отдаётся приложению последним) и окно с копиями остаётся. Успешный проход (хранилище управляется,
//! `active = false`, `restoring = false`) подтверждён в живом окне собранного приложения (`docs/screens/3/live-run.txt`).

#![cfg(target_os = "macos")]

mod common;

use common::*;
use kinige_lib::recovery::Recovery;
use kinige_store::{Now, Store, StoreOptions};
use serde_json::json;
use tauri::Manager;

#[test]
fn a_failure_of_the_services_after_restore_leaves_the_recovery_window_and_no_store() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("kinige.db");
    // SAFETY: единственный тест в этом бинарнике, другие потоки окружение не читают.
    unsafe {
        std::env::set_var("KINIGE_DATA_DIR", dir.path());
        std::env::set_var("KINIGE_MIRROR_DIR", dir.path().join("mirror"));
    }
    let name = {
        let mut options = StoreOptions::new(&db, None);
        options.start_mirror = false;
        options.start_maintenance = false;
        let store = Store::open(options).unwrap();
        store
            .send_message(kinige_domain::groups::INBOX_ID, "из копии", &Now::system())
            .unwrap();
        store.run_maintenance(&Now::system()).unwrap();
        kinige_store::backups(&db)[0].name.clone()
    };
    std::fs::write(&db, b"broken").unwrap();

    let fixture = fixture_without_store(&db);
    let main = window(&fixture, "main");
    assert_eq!(
        call(&main, "startup_state", json!({})).unwrap()["needsRecovery"],
        true
    );

    let outcome = call(&main, "restore_backup", json!({"name": name}));

    // Файлы восстановлены (повреждённый отложен), но сервисы не запустились: восстановление считается неудавшимся.
    assert!(
        std::fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .any(|e| e.file_name().to_string_lossy().starts_with("corrupt-")),
        "повреждённый файл отложен"
    );
    assert_eq!(outcome.unwrap_err(), json!({"code": "internal"}));
    let recovery = fixture.app.state::<Recovery>();
    assert!(recovery.is_active() && !recovery.is_restoring());
    assert!(
        fixture.app.try_state::<Store>().is_none(),
        "хранилище приложению не отдано: подмены базы под писателем не будет"
    );
    assert_eq!(
        call(&main, "startup_state", json!({})).unwrap()["needsRecovery"],
        true
    );
}
