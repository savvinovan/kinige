//! A panic outside MCP after removing `panic = "abort"` (slice 4a): reading, a copy pass and a maintenance pass must not
//! silently break forever. A panic is an error of that call or that pass, the next ones work.

// The integration test root is test code as a whole: `cfg(test)` gives helpers outside #[test] the same clippy.toml
// relaxations (unwrap/expect/panic/indexing) as the tests in src/.
#![cfg(test)]

mod common;

use common::*;
use kinige_domain::BackupError;
use kinige_domain::MirrorError;
use kinige_domain::groups::INBOX_ID;
use kinige_store::{BackgroundThread, Store, StoreError};

#[test]
fn a_panic_inside_a_read_fails_that_call_only_and_later_reads_work() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    store
        .send_message(INBOX_ID, "до паники", &moscow(T0))
        .unwrap();
    assert!(matches!(
        store.panic_in_read_for_test(),
        Err(StoreError::Internal(_))
    ));
    // The read connection is alive: reads work, the write is visible.
    for _ in 0..3 {
        let page = store
            .list_messages(INBOX_ID, None, 10, &moscow(T0 + MINUTE))
            .unwrap();
        assert_eq!(page.messages.len(), 1);
        assert!(matches!(
            store.panic_in_read_for_test(),
            Err(StoreError::Internal(_))
        ));
    }
    assert!(store.get_settings().is_ok());
}

#[test]
fn a_panic_in_a_mirror_pass_is_caught_shown_in_settings_and_the_thread_keeps_going() {
    let dir = tempfile::tempdir().unwrap();
    // The retry after a panic is a minute away, so the error stays in the settings until the test wakes the thread itself
    // (the next `send_message`); with a short retry the error could be gone before a loaded runner got to look at it.
    let mut options = options(dir.path());
    options.mirror_retry = std::time::Duration::from_secs(60);
    let store = Store::open_at(options, &moscow(T0)).unwrap();
    store.send_message(INBOX_ID, "первая", &moscow(T0)).unwrap();
    let file = mirror_dir(dir.path())
        .join("Inbox")
        .join("2026")
        .join("2026-10-21.md");
    wait_until("the first copy", || file.exists());
    store.inject_background_panic_for_test(BackgroundThread::Mirror);
    wait_until("the copy error in settings", || {
        store.get_settings().unwrap().mirror.error == Some(MirrorError::WriteFailed)
    });
    // The thread is alive: a new entry reaches the copy, the error is cleared.
    store
        .send_message(INBOX_ID, "вторая после паники", &moscow(T0 + MINUTE))
        .unwrap();
    wait_until("the copy after the panic", || {
        std::fs::read_to_string(&file).is_ok_and(|text| text.contains("вторая после паники"))
    });
    wait_until("the error is cleared", || {
        store.get_settings().unwrap().mirror.error.is_none()
    });
}

#[test]
fn a_panic_in_a_maintenance_pass_is_caught_shown_in_settings_and_the_next_pass_runs() {
    let dir = tempfile::tempdir().unwrap();
    let mut options = options(dir.path());
    options.start_mirror = false;
    options.start_maintenance = true;
    let store = Store::open_at(options, &moscow(T0)).unwrap();
    // The first pass at startup runs on the real clock; we wait for its end (the day's copy appeared).
    wait_until("the first maintenance pass", || {
        store.get_settings().unwrap().backup.count >= 1
    });
    store.inject_background_panic_for_test(BackgroundThread::Maintenance);
    wait_until("the copy error in settings", || {
        store.get_settings().unwrap().backup.error == Some(BackupError::WriteFailed)
    });
    // The thread is alive: we wake it once more, the next pass runs and clears the error.
    store.wake_maintenance_for_test();
    wait_until("the next pass cleared the error", || {
        store.get_settings().unwrap().backup.error.is_none()
    });
}
