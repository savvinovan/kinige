//! Состояние запуска и восстановление через IPC (повторное ревью guardian): повреждение — флаг процесса, а не файл-отметка;
//! во время восстановления окно «База повреждена» остаётся и перезапуск запрещён; хранилище под открытой базой не подменяется;
//! база новее приложения не предлагает копий.

mod common;

use common::*;
use kinige_lib::recovery::Recovery;
use kinige_store::{LATEST_VERSION, Store, StoreError, StoreOptions};
use serde_json::json;
use std::sync::atomic::Ordering;
use tauri::Manager;

const INBOX: &str = kinige_domain::groups::INBOX_ID;

fn recovery(fixture: &Fixture) -> tauri::State<'_, Recovery> {
    fixture.app.state::<Recovery>()
}

#[test]
fn corruption_found_is_a_process_flag_not_a_leftover_marker_file() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    let db_path = fixture.app.state::<Store>().db_path().to_path_buf();

    // Отметка от прошлого запуска (или проверка не состоялась — `Unknown`), флага нет: «База повреждена» не показывается и
    // перезапуск не разрешён.
    kinige_store::mark_needs_recovery(&db_path).unwrap();
    let state = call(&main, "startup_state", json!({})).unwrap();
    assert_eq!(state["corruptionFound"], false);
    assert_eq!(
        call(&main, "restart_for_recovery", json!({})).unwrap_err(),
        json!({"code": "internal"})
    );
    kinige_store::clear_needs_recovery(&db_path);

    // Фоновая проверка этого процесса нашла повреждение: окно в режиме «работающая база».
    recovery(&fixture)
        .corruption_flag()
        .store(true, Ordering::SeqCst);
    let found = call(&main, "startup_state", json!({})).unwrap();
    assert_eq!(found["corruptionFound"], true);
    assert_eq!(found["needsRecovery"], false, "журнал по-прежнему открыт");
}

#[test]
fn during_a_restore_the_recovery_window_stays_and_restart_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("kinige.db");
    let fixture = fixture_without_store(&db);
    let main = window(&fixture, "main");
    recovery(&fixture)
        .corruption_flag()
        .store(true, Ordering::SeqCst);
    assert!(recovery(&fixture).begin_restore());

    // Страница могла перезагрузиться посреди копирования: журнала ещё нет, окно с копиями остаётся.
    let state = call(&main, "startup_state", json!({})).unwrap();
    assert_eq!(state["needsRecovery"], true);
    assert_eq!(state["corruptionFound"], false);
    // `app.restart()` посреди копирования файла базы недопустим.
    assert_eq!(
        call(&main, "restart_for_recovery", json!({})).unwrap_err(),
        json!({"code": "internal"})
    );
    // Второе «Восстановить» в то же время — отказ.
    assert_eq!(
        call(
            &main,
            "restore_backup",
            json!({"name": "daily-2026-10-21.db"})
        )
        .unwrap_err(),
        json!({"code": "internal"})
    );

    recovery(&fixture).finish_restore();
    let after = call(&main, "startup_state", json!({})).unwrap();
    assert_eq!(after["needsRecovery"], false, "восстановление закончено");
}

#[test]
fn restore_is_refused_while_a_store_is_open_even_if_the_flag_says_otherwise() {
    // Сбой после того, как хранилище уже отдано приложению, оставил бы флаг «журнал не открыт»: повторное «Восстановить»
    // переименовало бы файл базы под открытым писателем. Команда отказывает по самому наличию хранилища.
    let fixture = fixture();
    let main = window(&fixture, "main");
    let store = fixture.app.state::<Store>();
    store
        .send_message(INBOX, "из копии", &kinige_store::Now::system())
        .unwrap();
    store.run_maintenance(&kinige_store::Now::system()).unwrap();
    let db_path = store.db_path().to_path_buf();
    let name = kinige_store::backups(&db_path)[0].name.clone();
    recovery(&fixture).abort_restore();

    assert_eq!(
        call(&main, "restore_backup", json!({"name": name})).unwrap_err(),
        json!({"code": "internal"})
    );

    assert!(!recovery(&fixture).is_restoring());
    assert!(db_path.exists());
    assert_eq!(
        std::fs::read_dir(db_path.parent().unwrap())
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with("corrupt-"))
            .count(),
        0,
        "файл базы не переименовывался"
    );
    // Журнал работает как прежде.
    let list = call(&main, "list_chats", json!({})).unwrap();
    assert_eq!(list["chats"][0]["count"], 1);
}

#[test]
fn a_database_from_a_newer_app_offers_no_backups() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("kinige.db");
    {
        let mut options = StoreOptions::new(&db, None);
        options.start_mirror = false;
        options.start_maintenance = false;
        let store = Store::open(options).unwrap();
        store.run_maintenance(&kinige_store::Now::system()).unwrap();
    }
    assert!(!kinige_store::backups(&db).is_empty(), "копия дня есть");
    {
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.pragma_update(None, "user_version", LATEST_VERSION + 1)
            .unwrap();
    }
    let mut options = StoreOptions::new(&db, None);
    options.start_mirror = false;
    options.start_maintenance = false;
    let error = Store::open(options).err().unwrap();
    assert!(matches!(error, StoreError::TooNew(_)), "{error}");

    let fixture = fixture_too_new(&db);
    let main = window(&fixture, "main");
    assert_eq!(
        call(&main, "startup_state", json!({})).unwrap(),
        json!({"needsRecovery": false, "backups": [], "databaseTooNew": true, "corruptionFound": false})
    );
    // Восстанавливать нечем и незачем.
    assert_eq!(
        call(
            &main,
            "restore_backup",
            json!({"name": "daily-2026-10-21.db"})
        )
        .unwrap_err(),
        json!({"code": "internal"})
    );
}
