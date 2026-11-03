//! The process log with a broken settings file (ADR-0010): one warning per state change, not one per poll.
//! A separate binary: there is one logger per process, other tests do not clutter its lines.

// The integration test root is test code as a whole: `cfg(test)` gives helpers outside #[test] the same clippy.toml
// relaxations (unwrap/expect/panic/indexing) as the tests in src/.
#![cfg(test)]

mod common;

use common::settings_file::*;
use common::{raw, wait_until};
use kinige_domain::TextSize;
use log::{LevelFilter, Log, Metadata, Record};
use std::sync::Mutex;
use std::sync::atomic::Ordering;
use std::time::Duration;

static CAPTURED: Mutex<Vec<String>> = Mutex::new(Vec::new());
/// The binary's tests run one after another: they share a logger.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

struct Capture;

impl Log for Capture {
    fn enabled(&self, _: &Metadata) -> bool {
        true
    }
    fn log(&self, record: &Record) {
        CAPTURED.lock().unwrap().push(record.args().to_string());
    }
    fn flush(&self) {}
}

fn install() -> std::sync::MutexGuard<'static, ()> {
    static ONCE: std::sync::Once = std::sync::Once::new();
    let guard = ONE_AT_A_TIME
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    ONCE.call_once(|| {
        log::set_logger(&Capture).unwrap();
        log::set_max_level(LevelFilter::Trace);
    });
    CAPTURED.lock().unwrap().clear();
    guard
}

/// How many full polls the "nothing is logged again" checks wait for.
const QUIET_POLLS: u64 = 10;

fn file_lines() -> Vec<String> {
    CAPTURED
        .lock()
        .unwrap()
        .iter()
        .filter(|line| line.contains("settings"))
        .cloned()
        .collect()
}

#[test]
fn a_broken_file_after_a_window_edit_is_logged_once_not_on_every_poll() {
    let _guard = install();
    let rig = rig_with(|options| options.poll = Duration::from_millis(20));
    rig.write(r#"{"text_size": "#);
    // An edit from the window with a broken file: the write is deferred; polls run every 20 ms.
    rig.file
        .edit(|store| store.set_text_size(TextSize::Large))
        .unwrap()
        .unwrap();
    let polls_before = file_lines().len();
    rig.after_polls(QUIET_POLLS);
    let lines = file_lines();
    assert_eq!(
        lines.len(),
        polls_before,
        "polls do not write repeats: {lines:?}"
    );
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].contains("not parsed") && lines[0].contains("parse"));
    // The file was not re-read on every poll: after a fix it is applied, and the "broken" state is logged anew.
    rig.write(r#"{"text_size": "normal"}"#);
    rig.write(r#"{"text_size": "#);
    rig.after_polls(QUIET_POLLS / 2);
    assert_eq!(file_lines().len(), 2);
}

// A FIFO (`mkfifo`) exists on Unix only.
#[cfg(unix)]
#[test]
fn an_unsafe_file_and_a_fifo_do_not_flood_the_log_either() {
    let _guard = install();
    let before = file_lines().len();
    let rig = rig_with(|options| options.poll = Duration::from_millis(20));
    let path = rig.path();
    // A swap with a single rename: a poll every 20 ms must not see "no file" and recreate it.
    let fifo = rig.dir.path().join("fifo.swap");
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success()
    );
    std::fs::rename(&fifo, &path).unwrap();
    rig.file.sync_now();
    rig.file
        .edit(|store| store.set_text_size(TextSize::Large))
        .unwrap()
        .unwrap();
    rig.after_polls(QUIET_POLLS);
    assert!(file_lines().len() - before <= 1, "{:?}", file_lines());
}

#[test]
fn a_dropped_handle_stops_the_thread_nothing_is_applied_and_nothing_is_logged() {
    let _guard = install();
    let rig = rig_with(|options| options.poll = Duration::from_millis(10));
    let applied = rig.changes.load(Ordering::SeqCst);
    let Rig {
        file,
        dir,
        store,
        changes,
    } = rig;
    // `Drop` waits for the thread to stop: there are no polls after it at all, so there is nothing to wait for.
    drop(file);
    write_file(dir.path(), br#"{"text_size": "large"}"#, 0o600);
    write_file(dir.path(), br#"{"text_size": "#, 0o600);
    assert_eq!(store.get_settings().unwrap().text_size, TextSize::Normal);
    assert_eq!(changes.load(Ordering::SeqCst), applied);
    assert!(file_lines().is_empty(), "{:?}", file_lines());
}

fn count(needle: &str) -> usize {
    file_lines()
        .iter()
        .filter(|line| line.contains(needle))
        .count()
}

#[test]
fn a_busy_database_logs_the_apply_error_once_and_again_after_a_success() {
    let _guard = install();
    let rig = rig_with(|options| options.poll = Duration::from_millis(20));
    let hide = |name: &str, to: &str| {
        raw(rig.dir.path())
            .execute_batch(&format!("ALTER TABLE {name} RENAME TO {to}"))
            .unwrap();
    };
    // The database is "busy": the settings table is absent, the write from applying the file fails, the file signature is cleared and polling goes on.
    hide("app_setting", "app_setting_away");
    write_file(rig.dir.path(), br#"{"text_size":"large"}"#, 0o600);
    // The poll notices the file and fails to apply it: the first line. The failed apply clears the file signature, so every
    // later poll reads and tries the file again; those polls must stay silent.
    wait_until("the first warning", || count("not applied") >= 1);
    rig.after_polls(QUIET_POLLS);
    assert_eq!(count("not applied"), 1, "{:?}", file_lines());
    // Fixed: it applies (a success clears the "apply" kind).
    hide("app_setting_away", "app_setting");
    wait_until("applied after the fix", || {
        rig.store.get_settings().unwrap().text_size == TextSize::Large
    });
    // Failing again: a second line, neither silence nor a flood.
    hide("app_setting", "app_setting_away");
    write_file(rig.dir.path(), br#"{"text_size":"normal"}"#, 0o600);
    wait_until("the second warning", || count("not applied") >= 2);
    rig.after_polls(QUIET_POLLS);
    assert_eq!(count("not applied"), 2, "{:?}", file_lines());
}

#[test]
fn a_failing_hash_write_and_the_apply_error_that_follows_are_each_logged_once_and_again_after_a_success()
 {
    let _guard = install();
    let rig = rig_with(|options| options.poll = Duration::from_millis(20));
    let block_hash = || {
        raw(rig.dir.path())
            .execute_batch(
                "CREATE TRIGGER no_hash_insert BEFORE INSERT ON app_setting WHEN NEW.key = 'settings_file_hash' \
                 BEGIN SELECT RAISE(ABORT, 'forbidden'); END; \
                 CREATE TRIGGER no_hash_update BEFORE UPDATE ON app_setting WHEN NEW.key = 'settings_file_hash' \
                 BEGIN SELECT RAISE(ABORT, 'forbidden'); END;",
            )
            .unwrap();
    };
    let allow_hash = || {
        raw(rig.dir.path())
            .execute_batch("DROP TRIGGER no_hash_insert; DROP TRIGGER no_hash_update;")
            .unwrap();
    };
    let edit = |size: TextSize| {
        rig.file
            .edit(move |store| store.set_text_size(size))
            .unwrap()
            .unwrap();
    };
    // The hash cannot be written: the first edit writes the file and runs into the hash write; the second finds the file "foreign" (the hash did not match),
    // applies it and runs into the same write inside the apply transaction. Each kind is one line, however many edits there are.
    block_hash();
    for size in [
        TextSize::Large,
        TextSize::Normal,
        TextSize::Large,
        TextSize::Normal,
    ] {
        edit(size);
    }
    // `edit` returns after the edit and the failed writes are done; the polls that follow must add no lines.
    rig.after_polls(QUIET_POLLS);
    assert_eq!(
        count("settings file hash not written"),
        1,
        "{:?}",
        file_lines()
    );
    assert_eq!(count("not applied"), 1, "{:?}", file_lines());
    // Fixed: edits go through, both kinds are cleared by a success.
    allow_hash();
    // An external edit (different file bytes) applies successfully: the "apply" kind is cleared; a window edit writes the hash: the "hash" kind is cleared.
    rig.write(r#"{"text_size":"large"}"#);
    edit(TextSize::Normal);
    // Failing again: a second line for each kind.
    block_hash();
    for size in [TextSize::Large, TextSize::Normal, TextSize::Large] {
        edit(size);
    }
    rig.after_polls(QUIET_POLLS);
    assert_eq!(
        count("settings file hash not written"),
        2,
        "{:?}",
        file_lines()
    );
    assert_eq!(count("not applied"), 2, "{:?}", file_lines());
}
