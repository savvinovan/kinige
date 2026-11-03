//! Startup state and restore through IPC (the repeated guardian review): corruption is a process flag, not a marker file;
//! during a restore the "Database is corrupt" window stays and a restart is forbidden; the store under an open database is not replaced;
//! a database newer than the app offers no copies.

// The whole root of the integration tests is test code: `cfg(test)` gives helpers outside #[test] the same relaxations
// of clippy.toml (unwrap/expect/panic/indexing) as the tests in src/.
#![cfg(test)]

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

    // A mark from a previous launch (or the check did not happen -- `Unknown`), no flag: "Database is corrupt" is not shown and
    // a restart is not allowed.
    kinige_store::mark_needs_recovery(&db_path).unwrap();
    let state = call(&main, "startup_state", json!({})).unwrap();
    assert_eq!(state["corruptionFound"], false);
    assert_eq!(
        call(&main, "restart_for_recovery", json!({})).unwrap_err(),
        json!({"code": "internal"})
    );
    kinige_store::clear_needs_recovery(&db_path);

    // The background check of this process found corruption: the window is in "running database" mode.
    recovery(&fixture)
        .corruption_flag()
        .store(true, Ordering::SeqCst);
    let found = call(&main, "startup_state", json!({})).unwrap();
    assert_eq!(found["corruptionFound"], true);
    assert_eq!(found["needsRecovery"], false, "the journal is still open");
}

#[test]
fn a_command_that_hits_a_damaged_page_raises_the_same_signal_as_the_background_check() {
    // Damage in pages that opening does not see: the first command that reads a broken page answers
    // `database_corrupt` and sets the flag, the mark and the event. Otherwise "Restart" in the window opened by this error would be
    // a dead button (a restart is allowed only by the flag).
    let fixture = fixture_with_corruption_hook();
    let main = window(&fixture, "main");
    let db = fixture.app.state::<Store>().db_path().to_path_buf();
    assert_eq!(
        call(&main, "startup_state", json!({})).unwrap()["corruptionFound"],
        false
    );
    // All pages after the first are corrupted: reading the settings will reach a broken one.
    let conn = rusqlite::Connection::open(&db).unwrap();
    let _ = conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()));
    drop(conn);
    let mut bytes = std::fs::read(&db).unwrap();
    for byte in &mut bytes[4096..] {
        *byte = 0xFF;
    }
    std::fs::write(&db, bytes).unwrap();

    let error = call(&main, "get_settings", json!({})).unwrap_err();

    assert_eq!(error, json!({"code": "database_corrupt"}));
    assert!(kinige_store::needs_recovery(&db), "the mark is set");
    assert_eq!(
        call(&main, "startup_state", json!({})).unwrap()["corruptionFound"],
        true,
        "\"Restart\" is allowed"
    );
    // Repeated errors of the same launch do not repeat the signal (the flag is already set, there is one event).
    let again = call(&main, "get_settings", json!({})).unwrap_err();
    assert_eq!(again, json!({"code": "database_corrupt"}));
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

    // The page may have reloaded in the middle of copying: there is no journal yet, the window with copies stays.
    let state = call(&main, "startup_state", json!({})).unwrap();
    assert_eq!(state["needsRecovery"], true);
    assert_eq!(state["corruptionFound"], false);
    // `app.restart()` in the middle of copying the database file is not acceptable.
    assert_eq!(
        call(&main, "restart_for_recovery", json!({})).unwrap_err(),
        json!({"code": "internal"})
    );
    // A second "Restore" at the same time is refused.
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
    assert_eq!(after["needsRecovery"], false, "the restore is finished");
}

#[test]
fn a_finding_made_by_the_restored_database_survives_the_end_of_the_restore() {
    // `resume_after_restore` opens the store, and maintenance checks integrity right away: if the copy turned out to have
    // damage that `quick_check` does not see, the hook sets the flag BEFORE `finish_restore`. "Restart" must not become dead.
    let dir = tempfile::tempdir().unwrap();
    let fixture = fixture_without_store(&dir.path().join("kinige.db"));
    let main = window(&fixture, "main");
    assert!(recovery(&fixture).begin_restore());

    recovery(&fixture)
        .corruption_flag()
        .store(true, Ordering::SeqCst); // the hook between `open_store` and `finish_restore`
    recovery(&fixture).finish_restore();

    let state = call(&main, "startup_state", json!({})).unwrap();
    assert_eq!(state["corruptionFound"], true);
    assert_eq!(state["needsRecovery"], false);
}

#[test]
fn restore_is_refused_while_a_store_is_open_even_if_the_flag_says_otherwise() {
    // A failure after the store was already handed to the app would leave the "journal not open" flag: a repeated "Restore"
    // would rename the database file under an open writer. The command refuses by the mere presence of the store.
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
        "the database file was not renamed"
    );
    // The journal works as before.
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
    assert!(
        !kinige_store::backups(&db).is_empty(),
        "the day's copy exists"
    );
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
    // There is nothing to restore from and no point.
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
