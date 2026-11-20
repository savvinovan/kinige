//! Slice 5a over IPC on the mock runtime with the real capabilities: `set_message_reaction` belongs to the main window only (the capture
//! window keeps exactly its ten commands, the settings window is denied), a reaction travels through the feed and the settings, garbage
//! comes back as an error without a panic and without writing anything, and no emoji or id of a reaction reaches the process log.

// A whole integration-test root is test code: `cfg(test)` gives helpers outside #[test] the same clippy.toml allowances
// (unwrap/expect/panic/indexing) as the tests in src/.
#![cfg(test)]

mod common;

use common::*;
use kinige_lib::{CAPTURE_COMMANDS, MAIN_COMMANDS, SETTINGS_COMMANDS};
use log::{LevelFilter, Log, Metadata, Record};
use serde_json::{Value, json};
use std::sync::Mutex;

const INBOX: &str = kinige_domain::groups::INBOX_ID;
const COMMAND: &str = "set_message_reaction";
/// A unique mark in everything a person would type or choose.
const MARK: &str = "ZEBRAUNIQ";
/// An emoji nothing else in the suite uses: it must never appear in the log.
const RARE: &str = "🦩";

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

fn send(main: &tauri::WebviewWindow<tauri::test::MockRuntime>, body: &str) -> Value {
    call(main, "send_message", json!({"chatId": INBOX, "body": body})).unwrap()
}

#[test]
fn the_reaction_command_is_main_only_and_the_capture_set_is_unchanged() {
    assert_eq!(
        CAPTURE_COMMANDS, CAPTURE_SET,
        "the capture window's commands changed: this slice adds none (ten, KINIG-153)"
    );
    assert_eq!(CAPTURE_SET.len(), 10);
    assert!(MAIN_COMMANDS.contains(&COMMAND));
    assert!(!CAPTURE_COMMANDS.contains(&COMMAND));
    assert!(!SETTINGS_COMMANDS.contains(&COMMAND));
    assert!(
        !CAPTURE_COMMANDS.iter().any(|c| c.contains("reaction")),
        "no capture command for reactions"
    );
    let fixture = fixture();
    let main = window(&fixture, "main");
    for label in ["capture", "settings"] {
        let denied = window(&fixture, label);
        let error = call(&denied, COMMAND, args_for(COMMAND))
            .expect_err(&format!("{COMMAND} must be denied to {label}"));
        assert!(is_acl_denial(&error), "{label}: {error}");
    }
    // The main window reaches the handler: an unknown entry is the store's answer, not the ACL's.
    assert_eq!(
        call(&main, COMMAND, args_for(COMMAND)).unwrap_err(),
        json!({"code": "message_not_found"})
    );
}

#[test]
fn a_reaction_goes_through_the_feed_the_settings_and_back() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    let sent = send(&main, "Сдал экзамен!");
    let id = sent["id"].as_str().unwrap();
    assert_eq!(sent["reactions"], json!([]));
    assert_eq!(sent["reactionKeysHad"], json!([]));

    let put = call(&main, COMMAND, json!({"id": id, "emoji": "❤️", "on": true})).unwrap();
    assert_eq!(put["reactions"][0]["emoji"], "❤️");
    assert_eq!(put["reactions"][0]["yearsLater"], 0);
    assert!(put["reactions"][0]["putAt"].is_number());
    assert!(put["reactions"][0]["putTz"].is_string());
    assert_eq!(put["reactionKeysHad"], json!(["\u{2764}"]));
    assert!(put["bodyMd"] == sent["bodyMd"] && put["favoriteAt"] == Value::Null);

    let page = call(&main, "list_messages", json!({"chatId": INBOX})).unwrap();
    assert_eq!(page["messages"][0]["reactions"], put["reactions"]);
    let settings = call(&main, "get_settings", json!({})).unwrap();
    assert_eq!(settings["quickReactions"], json!(["❤️", "😂", "😢"]));
    call(&main, COMMAND, json!({"id": id, "emoji": "🔥", "on": true})).unwrap();
    let settings = call(&main, "get_settings", json!({})).unwrap();
    assert_eq!(settings["quickReactions"], json!(["🔥", "❤️", "😂"]));
    assert_eq!(settings["recentReactions"], json!(["🔥", "❤️"]));

    let removed = call(&main, COMMAND, json!({"id": id, "emoji": "❤", "on": false})).unwrap();
    assert_eq!(removed["reactions"].as_array().map(Vec::len), Some(1));
    assert_eq!(removed["reactionKeysHad"], json!(["\u{2764}", "🔥"]));
    // The two counts the caps use come through in the right order, and are not equal: three items held, one live (two removed).
    assert_eq!(
        (
            removed["reactionKeyCount"].clone(),
            removed["reactionLiveCount"].clone()
        ),
        (json!(2), json!(1))
    );
    call(&main, COMMAND, json!({"id": id, "emoji": "😂", "on": true})).unwrap();
    let three = call(
        &main,
        COMMAND,
        json!({"id": id, "emoji": "😂", "on": false}),
    )
    .unwrap();
    assert_eq!(
        (
            three["reactionKeyCount"].clone(),
            three["reactionLiveCount"].clone()
        ),
        (json!(3), json!(1))
    );
    assert_eq!(three["reactionKeysHad"], json!(["\u{2764}", "🔥", "😂"]));
    // The trash shows them and refuses a change.
    call(&main, "delete_message", json!({"id": id})).unwrap();
    assert_eq!(
        call(&main, "list_trash", json!({})).unwrap()[0]["message"]["reactions"][0]["emoji"],
        "🔥"
    );
    assert_eq!(
        call(
            &main,
            COMMAND,
            json!({"id": id, "emoji": "🔥", "on": false})
        )
        .unwrap_err(),
        json!({"code": "message_in_trash"})
    );
}

#[test]
fn the_limits_and_the_garbage_come_back_as_codes_and_write_nothing() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    let sent = send(&main, "text");
    let id = sent["id"].as_str().unwrap().to_string();
    let code = |emoji: Value| {
        call(
            &main,
            COMMAND,
            json!({"id": id, "emoji": emoji, "on": true}),
        )
        .unwrap_err()
    };
    // An emoji that is not one: empty, text, two emoji, a lone VS16 or ZWJ, controls, a megabyte.
    let big = "x".repeat(1 << 20);
    for text in [
        "",
        " ",
        "text",
        "😂😂",
        "\u{FE0F}",
        "\u{200D}",
        "\u{7}",
        "<b>",
        big.as_str(),
    ] {
        assert_eq!(
            code(json!(text)),
            json!({"code": "reaction_invalid"}),
            "{:.10}",
            text
        );
    }
    // Shapes that are not a string, a missing field, a field of another type, and an id that is no id: rejected before or in the handler.
    for args in [
        json!({"id": id, "on": true}),
        json!({"id": id, "emoji": 5, "on": true}),
        json!({"id": id, "emoji": ["🔥"], "on": true}),
        json!({"id": id, "emoji": "🔥", "on": "yes"}),
        json!({"id": id, "emoji": "🔥"}),
        json!({"emoji": "🔥", "on": true}),
        json!({"id": null, "emoji": "🔥", "on": true}),
        json!([id, "🔥", true]),
        json!("🔥"),
        json!(null),
    ] {
        assert!(call(&main, COMMAND, args.clone()).is_err(), "{args}");
    }
    for bad_id in [
        "",
        "x",
        &"y".repeat(1 << 16),
        "00000000-0000-4000-8000-000000000001",
    ] {
        assert_eq!(
            call(
                &main,
                COMMAND,
                json!({"id": bad_id, "emoji": "🔥", "on": true})
            )
            .unwrap_err(),
            json!({"code": "message_not_found"}),
            "{:.10}",
            bad_id
        );
    }
    // The order of the checks: the emoji is checked before the entry is looked up, so garbage on an id nobody has is `reaction_invalid`, not
    // `message_not_found`; a good emoji on that id is `message_not_found`.
    for bad_id in ["", "x", "00000000-0000-4000-8000-000000000001"] {
        for text in ["", "text", "😂😂", "<b>"] {
            assert_eq!(
                call(
                    &main,
                    COMMAND,
                    json!({"id": bad_id, "emoji": text, "on": true})
                )
                .unwrap_err(),
                json!({"code": "reaction_invalid"}),
                "{bad_id} {text}"
            );
        }
    }
    let page = call(&main, "list_messages", json!({"chatId": INBOX})).unwrap();
    assert_eq!(
        page["messages"][0]["reactions"],
        json!([]),
        "nothing was written"
    );
    // The caps arrive as `reaction_limit`.
    for i in 0..10u32 {
        let e = char::from_u32(0x1F600 + i).unwrap().to_string();
        call(&main, COMMAND, json!({"id": id, "emoji": e, "on": true})).unwrap();
    }
    assert_eq!(code(json!("🎉")), json!({"code": "reaction_limit"}));
}

#[test]
fn neither_an_emoji_nor_an_id_nor_a_typed_value_of_a_reaction_reaches_the_log() {
    install_logger();
    let fixture = fixture();
    let main = window(&fixture, "main");
    let sent = send(&main, "log probe");
    let id = sent["id"].as_str().unwrap();
    call(&main, COMMAND, json!({"id": id, "emoji": RARE, "on": true})).unwrap();
    call(
        &main,
        COMMAND,
        json!({"id": id, "emoji": RARE, "on": false}),
    )
    .unwrap();
    let _ = call(
        &main,
        COMMAND,
        json!({"id": id, "emoji": format!("{MARK}{RARE}"), "on": true}),
    );
    let _ = call(
        &main,
        COMMAND,
        json!({"id": format!("{MARK}-id"), "emoji": RARE, "on": true}),
    );
    let _ = call(
        &main,
        COMMAND,
        json!({"id": id, "emoji": MARK.repeat(1000), "on": false}),
    );
    // A refusal of the cap, which has the most to say, and a trashed entry.
    for i in 0..10u32 {
        let e = char::from_u32(0x1F600 + i).unwrap().to_string();
        call(&main, COMMAND, json!({"id": id, "emoji": e, "on": true})).unwrap();
    }
    let _ = call(&main, COMMAND, json!({"id": id, "emoji": RARE, "on": true}));
    let lines = CAPTURED.lock().unwrap().clone();
    for line in &lines {
        assert!(
            !line.to_uppercase().contains(MARK),
            "typed value in the log: {line}"
        );
        assert!(!line.contains(RARE), "reaction emoji in the log: {line}");
        assert!(!line.contains(id), "entry id in the log: {line}");
    }
}
