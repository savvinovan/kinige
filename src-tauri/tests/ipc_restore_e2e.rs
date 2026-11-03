//! End-to-end restore through IPC: `restore_backup` -> files -> `resume_after_restore` -> the `Recovery` state. A separate
//! binary with one test: the test sets `KINIGE_DATA_DIR`, and environment variables are shared by the whole process.
//!
//! **A successful pass in a mock runtime is impossible on both macOS and Linux.** `resume_after_restore` starts the services:
//! on macOS `panel::convert` goes first (the capture window as an NSPanel -- the mock runtime has neither a window nor a panel plugin), on
//! Linux `tray::setup`, which needs an initialized GTK (`gtk::Menu` panics without a display). So what is checked here
//! is what can be: **a failure of the services does not leave the app with a store under the "journal not open" flag**
//! (the store is handed to the app last) and the window with copies stays. The successful pass (the store is managed,
//! `active = false`, `restoring = false`) was confirmed in a live window of the built app (`docs/screens/3/live-run.txt`).

// The whole root of the integration tests is test code: `cfg(test)` gives helpers outside #[test] the same relaxations
// of clippy.toml (unwrap/expect/panic/indexing) as the tests in src/.
#![cfg(test)]
#![cfg(target_os = "macos")]

mod common;

use common::*;
use kinige_lib::recovery::Recovery;
use kinige_store::{Now, Store, StoreOptions};
use serde_json::json;
use tauri::Manager;

#[test]
#[allow(unsafe_code)] // `set_var` is unsafe in edition 2024; the justification is below
fn a_failure_of_the_services_after_restore_leaves_the_recovery_window_and_no_store() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("kinige.db");
    // SAFETY: the only test in this binary, other threads do not read the environment.
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

    // The files are restored (the corrupt one is set aside), but the services did not start: the restore counts as failed.
    assert!(
        std::fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .any(|e| e.file_name().to_string_lossy().starts_with("corrupt-")),
        "the corrupt file is set aside"
    );
    assert_eq!(outcome.unwrap_err(), json!({"code": "internal"}));
    let recovery = fixture.app.state::<Recovery>();
    assert!(recovery.is_active() && !recovery.is_restoring());
    assert!(
        fixture.app.try_state::<Store>().is_none(),
        "the store was not handed to the app: the database will not be replaced under the writer"
    );
    assert_eq!(
        call(&main, "startup_state", json!({})).unwrap()["needsRecovery"],
        true
    );
}
