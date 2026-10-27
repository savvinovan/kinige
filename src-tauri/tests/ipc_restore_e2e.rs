//! Сквозное восстановление через IPC: `restore_backup` -> файлы -> `resume_after_restore` (хранилище открывается, сервисы
//! запускаются) -> состояние `Recovery`. Отдельный бинарник с одним тестом: тест задаёт `KINIGE_DATA_DIR`, а переменные
//! окружения общие для всего процесса.
//!
//! На macOS сервисы начинаются с `panel::convert` (окно захвата как NSPanel), а в mock-рантайме нет ни этого окна, ни
//! плагина панелей: успешный сквозной проход там невозможен, и тест проверяет обратное — **сбой сервисов** не оставляет
//! приложение с хранилищем под флагом «журнал не открыт» (хранилище отдаётся приложению последним). Успешный проход
//! (хранилище управляется, `active = false`, `restoring = false`) проверяется на Linux (CI): там панели нет.

mod common;

use common::*;
use kinige_lib::recovery::Recovery;
use kinige_store::{Now, Store, StoreOptions};
use serde_json::json;
use tauri::Manager;

#[test]
fn restore_over_ipc_through_resume_after_restore() {
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
    let recovery = fixture.app.state::<Recovery>();
    let corrupt_aside = std::fs::read_dir(dir.path())
        .unwrap()
        .flatten()
        .any(|e| e.file_name().to_string_lossy().starts_with("corrupt-"));
    assert!(corrupt_aside, "повреждённый файл отложен");

    if cfg!(target_os = "macos") {
        // Сервисы не запустились (нет окна-панели): восстановление считается неудавшимся, окно с копиями остаётся и
        // «Восстановить» можно повторить; хранилище приложению не отдано, поэтому подмены базы под писателем нет.
        assert_eq!(outcome.unwrap_err(), json!({"code": "internal"}));
        assert!(recovery.is_active() && !recovery.is_restoring());
        assert!(fixture.app.try_state::<Store>().is_none());
        assert_eq!(
            call(&main, "startup_state", json!({})).unwrap()["needsRecovery"],
            true
        );
    } else {
        assert!(outcome.is_ok(), "{outcome:?}");
        assert!(!recovery.is_active() && !recovery.is_restoring());
        assert!(
            fixture.app.try_state::<Store>().is_some(),
            "хранилище отдано приложению"
        );
        assert_eq!(
            call(&main, "startup_state", json!({})).unwrap()["needsRecovery"],
            false
        );
        let chats = call(&main, "list_chats", json!({})).unwrap();
        assert_eq!(chats["chats"][0]["count"], 1, "запись из копии на месте");
        assert!(!kinige_store::needs_recovery(&db));
    }
}
