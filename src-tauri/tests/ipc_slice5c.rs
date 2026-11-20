//! Slice 5c over IPC on the mock runtime with the real capabilities (§12.3): the library and the sticker commands belong to the main window,
//! the token commands to the settings window and to no other, the capture window keeps its ten commands, nothing reads the secret store until a pack
//! is downloaded, the token is nowhere on disk, `kinige-media://…/sticker/<id>` answers only `main`, and the CSP is as before the slice.

#![cfg(test)]

mod common;

use common::*;
use kinige_lib::{CAPTURE_COMMANDS, MAIN_COMMANDS, SETTINGS_COMMANDS};
use kinige_store::Store;
use log::{LevelFilter, Log, Metadata, Record};
use serde_json::json;
use std::path::Path;
use std::sync::Mutex;
use tauri::Manager;

const INBOX: &str = kinige_domain::groups::INBOX_ID;
const TOKEN: &str = "42:fake-token-for-tests";
/// The commands of the main window, and of the settings window.
const MAIN_ONLY: [&str; 7] = [
    "get_sticker_library",
    "add_sticker_pack",
    "cancel_sticker_pack",
    "remove_sticker_pack",
    "send_sticker",
    "list_journal_gifs",
    "send_gif_again",
];
const SETTINGS_ONLY: [&str; 3] = [
    "get_telegram_token_state",
    "set_telegram_bot_token",
    "clear_telegram_bot_token",
];

static CAPTURED: Mutex<Vec<String>> = Mutex::new(Vec::new());

struct Capture;

impl Log for Capture {
    fn enabled(&self, _: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        CAPTURED
            .lock()
            .unwrap()
            .push(format!("[{}] {}", record.level(), record.args()));
    }

    fn flush(&self) {}
}

fn install_logger() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        log::set_logger(&Capture).expect("the logger is installed once");
        log::set_max_level(LevelFilter::Trace);
    });
}

#[test]
fn the_sets_of_the_windows_are_what_the_slice_says() {
    assert_eq!(
        CAPTURE_COMMANDS, CAPTURE_SET,
        "the capture window keeps exactly its ten commands"
    );
    for command in MAIN_ONLY {
        assert!(
            MAIN_COMMANDS.contains(&command) && !SETTINGS_COMMANDS.contains(&command),
            "{command}"
        );
        assert!(!CAPTURE_COMMANDS.contains(&command), "{command}");
    }
    for command in SETTINGS_ONLY {
        assert!(
            SETTINGS_COMMANDS.contains(&command) && !MAIN_COMMANDS.contains(&command),
            "{command}"
        );
        assert!(!CAPTURE_COMMANDS.contains(&command), "{command}");
    }
    let fixture = fixture();
    let (main, settings, capture) = (
        window(&fixture, "main"),
        window(&fixture, "settings"),
        window(&fixture, "capture"),
    );
    for command in MAIN_ONLY {
        for (label, denied) in [("settings", &settings), ("capture", &capture)] {
            let error = call(denied, command, args_for(command)).expect_err(command);
            assert!(is_acl_denial(&error), "{command} from {label}: {error}");
        }
        let reached = call(&main, command, args_for(command));
        assert!(
            reached.as_ref().err().is_none_or(|e| !is_acl_denial(e)),
            "{command}: {reached:?}"
        );
    }
    for command in SETTINGS_ONLY {
        for (label, denied) in [("main", &main), ("capture", &capture)] {
            let error = call(denied, command, args_for(command)).expect_err(command);
            assert!(is_acl_denial(&error), "{command} from {label}: {error}");
        }
        let reached = call(&settings, command, args_for(command));
        assert!(
            reached.as_ref().err().is_none_or(|e| !is_acl_denial(e)),
            "{command}: {reached:?}"
        );
    }
}

#[test]
fn garbage_is_an_error_with_a_code_and_never_a_panic_and_writes_nothing() {
    let fixture = fixture();
    let (main, settings) = (window(&fixture, "main"), window(&fixture, "settings"));
    for link in [
        "",
        " ",
        "https://example.com/x",
        "a b",
        "t.me/addstickers/",
        "../x",
        &"a".repeat(100),
    ] {
        assert_eq!(
            call(&main, "add_sticker_pack", json!({"link": link})).unwrap_err(),
            json!({"code": "sticker_pack_link_invalid"}),
            "{link:?}"
        );
    }
    // A good link with no token: no request (the client's base is a closed port) and a category.
    assert_eq!(
        call(&main, "add_sticker_pack", json!({"link": "TestPack"})).unwrap_err(),
        json!({"code": "sticker_pack_no_token"})
    );
    for id in ["", "x", "../x", "00000000-0000-4000-8000-000000000000"] {
        assert_eq!(
            call(
                &main,
                "send_sticker",
                json!({"chatId": INBOX, "stickerId": id})
            )
            .unwrap_err(),
            json!({"code": "sticker_not_found"})
        );
        assert_eq!(
            call(&main, "remove_sticker_pack", json!({"packId": id})).unwrap_err(),
            json!({"code": "sticker_not_found"})
        );
        assert_eq!(
            call(
                &main,
                "send_gif_again",
                json!({"chatId": INBOX, "attachmentId": id})
            )
            .unwrap_err(),
            json!({"code": "attachment_not_found"})
        );
    }
    for token in [
        "",
        "x",
        "42:",
        ":x",
        "a:b",
        "42:has space",
        "42:é",
        &format!("{}:x", "9".repeat(21)),
        &format!("1:{}", "x".repeat(101)),
    ] {
        assert_eq!(
            call(&settings, "set_telegram_bot_token", json!({"token": token})).unwrap_err(),
            json!({"code": "telegram_token_invalid"}),
            "{token:?}"
        );
    }
    // Wrong types are an error, never a panic.
    assert!(call(&settings, "set_telegram_bot_token", json!({"token": 5})).is_err());
    assert!(call(&main, "send_sticker", json!({"chatId": 1})).is_err());
    assert!(call(&main, "add_sticker_pack", json!({})).is_err());
    assert_eq!(
        fixture.secrets.calls(),
        1,
        "only the good link read the store (and found nothing); nothing was saved"
    );
    assert_eq!(fixture.secrets.peek(), None);
}

#[test]
fn nothing_reads_the_secret_store_until_a_pack_is_downloaded() {
    let fixture = fixture();
    let (main, settings) = (window(&fixture, "main"), window(&fixture, "settings"));
    let library = call(&main, "get_sticker_library", json!({})).unwrap();
    assert_eq!(library["hasToken"], false);
    assert_eq!(
        call(&settings, "get_telegram_token_state", json!({})).unwrap(),
        json!({"saved": false, "itemLeft": false})
    );
    call(&main, "get_settings", json!({})).unwrap();
    call(&settings, "get_settings", json!({})).unwrap();
    call(&main, "list_journal_gifs", json!({})).unwrap();
    assert_eq!(
        fixture.secrets.calls(),
        0,
        "no read at startup, on opening Settings or the panel"
    );
}

#[test]
fn the_token_is_saved_through_settings_as_a_flag_and_is_nowhere_on_disk() {
    install_logger();
    let mark = CAPTURED.lock().unwrap().len();
    let fixture = fixture();
    let (main, settings) = (window(&fixture, "main"), window(&fixture, "settings"));
    let saved = call(
        &settings,
        "set_telegram_bot_token",
        json!({"token": format!(" {TOKEN} ")}),
    )
    .unwrap();
    assert_eq!(
        saved,
        json!({"saved": true, "itemLeft": false}),
        "the reply never carries the token"
    );
    assert_eq!(fixture.secrets.peek().as_deref(), Some(TOKEN));
    assert_eq!(
        call(&settings, "get_telegram_token_state", json!({})).unwrap(),
        json!({"saved": true, "itemLeft": false})
    );
    assert_eq!(
        call(&main, "get_sticker_library", json!({})).unwrap()["hasToken"],
        true
    );
    assert_eq!(fixture.secrets.calls(), 1, "one write, no read");
    // Where the token is not: the database, the backups, the data folder, the copy.
    let store = fixture.app.state::<Store>();
    store
        .send_message(INBOX, "запись", &kinige_store::Now::system())
        .unwrap();
    let report = store.run_maintenance(&kinige_store::Now::system()).unwrap();
    assert!(report.backup_made, "the day's backup exists to be scanned");
    assert!(
        !any_file_contains(fixture.data.path(), TOKEN.as_bytes()),
        "the token is in the data folder"
    );
    // A positive control: the scanner does find it when it is there.
    std::fs::write(fixture.data.path().join("leak.txt"), format!("x{TOKEN}y")).unwrap();
    assert!(any_file_contains(fixture.data.path(), TOKEN.as_bytes()));
    std::fs::remove_file(fixture.data.path().join("leak.txt")).unwrap();
    // Remove: the flag first, the item always deleted, whatever the flag says.
    assert_eq!(
        call(&settings, "clear_telegram_bot_token", json!({})).unwrap(),
        json!({"saved": false, "itemLeft": false})
    );
    assert_eq!(fixture.secrets.peek(), None);
    assert_eq!(
        call(&settings, "clear_telegram_bot_token", json!({})).unwrap(),
        json!({"saved": false, "itemLeft": false}),
        "twice is fine"
    );
    let lines = CAPTURED.lock().unwrap()[mark..].to_vec();
    assert!(
        lines.iter().all(|l| !l.contains("fake-token-for-tests")),
        "{lines:?}"
    );
}

fn any_file_contains(dir: &Path, needle: &[u8]) -> bool {
    std::fs::read_dir(dir).unwrap().flatten().any(|entry| {
        let path = entry.path();
        if path.is_dir() {
            any_file_contains(&path, needle)
        } else {
            std::fs::read(&path).is_ok_and(|bytes| bytes.windows(needle.len()).any(|w| w == needle))
        }
    })
}

/// N3 (the `windows` job runs it, and Linux would): where the platform has no secret store, saving a token and adding a pack by its link
/// are refused with their own error, Remove is harmless, and nothing is written. Never compiled in on macOS, where the store is the keychain.
#[cfg(not(target_os = "macos"))]
#[test]
fn the_real_store_of_this_platform_answers_unsupported() {
    let fixture = fixture_with_os_secrets();
    assert_unsupported_everywhere(&fixture);
    assert_eq!(fixture.secrets.calls(), 0, "the in-memory store was not used");
}

/// The same answers from a store that says "unsupported", on every platform.
#[test]
fn a_store_that_is_unsupported_refuses_the_token_and_the_pack_and_saves_nothing() {
    let fixture = fixture();
    fixture
        .secrets
        .fail_with(Some(kinige_lib::stickers::secret::SecretError::Unsupported));
    assert_unsupported_everywhere(&fixture);
}

fn assert_unsupported_everywhere(fixture: &Fixture) {
    let (main, settings) = (window(fixture, "main"), window(fixture, "settings"));
    assert_eq!(
        call(&settings, "set_telegram_bot_token", json!({"token": TOKEN})).unwrap_err(),
        json!({"code": "secret_store_unsupported"})
    );
    assert_eq!(
        call(&settings, "get_telegram_token_state", json!({})).unwrap(),
        json!({"saved": false, "itemLeft": false}),
        "no flag is set for a token that was not saved"
    );
    assert_eq!(
        call(&settings, "clear_telegram_bot_token", json!({})).unwrap(),
        json!({"saved": false, "itemLeft": false}),
        "there is nothing to delete, and that is no error"
    );
    assert_eq!(
        call(&main, "add_sticker_pack", json!({"link": "TestPack"})).unwrap_err(),
        json!({"code": "secret_store_unsupported"})
    );
    assert_eq!(
        call(&main, "get_sticker_library", json!({})).unwrap()["hasToken"],
        false
    );
}

#[test]
fn a_refused_secret_store_is_its_own_error_and_the_flag_stays_off() {
    let fixture = fixture();
    let settings = window(&fixture, "settings");
    fixture
        .secrets
        .fail_with(Some(kinige_lib::stickers::secret::SecretError::Unavailable));
    assert_eq!(
        call(&settings, "set_telegram_bot_token", json!({"token": TOKEN})).unwrap_err(),
        json!({"code": "secret_store_unavailable"})
    );
    assert_eq!(
        call(&settings, "get_telegram_token_state", json!({})).unwrap(),
        json!({"saved": false, "itemLeft": false})
    );
    // A denied delete (the system asked for access and it was refused) still clears the flag, and the answer says that the old item is
    // left in the keychain, so that Settings can tell the user where it is (N2). A plain delete never says it.
    fixture
        .secrets
        .fail_with(Some(kinige_lib::stickers::secret::SecretError::Denied));
    assert_eq!(
        call(&settings, "clear_telegram_bot_token", json!({})).unwrap(),
        json!({"saved": false, "itemLeft": true})
    );
    assert_eq!(
        call(&settings, "get_telegram_token_state", json!({})).unwrap(),
        json!({"saved": false, "itemLeft": false}),
        "the flag is off; the state read says nothing about the keychain"
    );
    // Another failure of the store is an error, with the flag off.
    fixture
        .secrets
        .fail_with(Some(kinige_lib::stickers::secret::SecretError::Unavailable));
    assert_eq!(
        call(&settings, "clear_telegram_bot_token", json!({})).unwrap_err(),
        json!({"code": "secret_store_unavailable"})
    );
}
