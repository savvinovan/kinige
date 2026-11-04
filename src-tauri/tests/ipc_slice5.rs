//! Slice 5 through IPC on a mock runtime with the real capabilities: "On this day" and replies. The commands exist only for the main
//! window; the capture window still has exactly seven commands, the new ones are refused to the `settings` window; `capture_send` does not accept a reply.

// The whole root of the integration tests is test code: `cfg(test)` gives helpers outside #[test] the same relaxations
// of clippy.toml (unwrap/expect/panic/indexing) as the tests in src/.
#![cfg(test)]

mod common;

use common::*;
use kinige_lib::{CAPTURE_COMMANDS, MAIN_COMMANDS, SETTINGS_COMMANDS};
use serde_json::json;

const INBOX: &str = kinige_domain::groups::INBOX_ID;

/// The slice 5 commands. The capture window and the settings window are refused on each.
const SLICE_FIVE: &[&str] = &["get_on_this_day", "dismiss_on_this_day"];

#[test]
fn slice_five_commands_are_main_only_and_the_capture_set_is_unchanged() {
    assert_eq!(
        CAPTURE_COMMANDS,
        [
            "capture_state",
            "capture_send",
            "capture_append",
            "capture_save_draft",
            "capture_hide",
            "capture_resize",
            "capture_add_recent_emoji"
        ],
        "the capture window still has seven commands"
    );
    let fixture = fixture();
    let main = window(&fixture, "main");
    let capture = window(&fixture, "capture");
    let settings = window(&fixture, "settings");
    for command in SLICE_FIVE {
        assert!(MAIN_COMMANDS.contains(command), "{command}");
        assert!(!CAPTURE_COMMANDS.contains(command), "{command}");
        assert!(!SETTINGS_COMMANDS.contains(command), "{command}");
        for denied in [&capture, &settings] {
            let error = call(denied, command, args_for(command))
                .expect_err(&format!("{command} is unavailable to this window"));
            assert!(is_acl_denial(&error), "{command}: {error}");
        }
        let result = call(&main, command, args_for(command));
        if let Err(error) = result {
            assert!(!is_acl_denial(&error), "{command}: {error}");
        }
    }
}

#[test]
fn reply_arguments_of_send_message_and_drafts_are_main_only_in_effect() {
    // `send_message`, `get_draft`, `save_draft` were already main-window only; replies did not widen them.
    let fixture = fixture();
    let capture = window(&fixture, "capture");
    let settings = window(&fixture, "settings");
    for command in ["send_message", "get_draft", "save_draft"] {
        for denied in [&capture, &settings] {
            let error = call(denied, command, args_for(command)).expect_err(command);
            assert!(is_acl_denial(&error), "{command}: {error}");
        }
    }
}

#[test]
fn a_reply_goes_through_ipc_end_to_end() {
    let fixture = fixture();
    let main = window(&fixture, "main");

    let old = call(
        &main,
        "send_message",
        json!({"chatId": INBOX, "body": "старая мысль"}),
    )
    .unwrap();
    let old_id = old["id"].as_str().unwrap().to_string();
    assert_eq!(old["replyTo"], json!(null));

    let reply = call(
        &main,
        "send_message",
        json!({"chatId": INBOX, "body": "до сих пор так думаю", "replyToId": old_id}),
    )
    .unwrap();
    assert_eq!(reply["replyTo"]["id"], json!(old_id));
    assert_eq!(reply["replyTo"]["state"], "live");
    assert_eq!(reply["replyTo"]["excerpt"], "старая мысль");
    assert_eq!(reply["replyTo"]["yearsLater"], 0);

    let page = call(
        &main,
        "list_messages",
        json!({"chatId": INBOX, "limit": 50}),
    )
    .unwrap();
    assert_eq!(page["messages"][1]["replyTo"]["id"], json!(old_id));
}

#[test]
fn garbage_reply_ids_come_back_as_reply_target_not_found_and_write_nothing() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    let huge = "я".repeat(1 << 20);
    for bad in [
        json!(""),
        json!("не-uuid"),
        json!(huge),
        json!("5b1c0d9e-3f2a-4a7b-9c8d-1e2f3a4b5c6d"),
        json!(42),
    ] {
        let error = call(
            &main,
            "send_message",
            json!({"chatId": INBOX, "body": "ответ", "replyToId": bad}),
        )
        .unwrap_err();
        if bad.is_number() {
            assert!(!is_acl_denial(&error), "{error}");
            continue;
        }
        assert_eq!(error, json!({"code": "reply_target_not_found"}));
    }
    let page = call(
        &main,
        "list_messages",
        json!({"chatId": INBOX, "limit": 50}),
    )
    .unwrap();
    assert_eq!(page["messages"], json!([]), "nothing was written");
}

#[test]
fn the_draft_carries_the_reply_and_survives_through_get_draft() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    let old = call(
        &main,
        "send_message",
        json!({"chatId": INBOX, "body": "старая"}),
    )
    .unwrap();
    let old_id = old["id"].as_str().unwrap();
    call(
        &main,
        "save_draft",
        json!({"chatId": INBOX, "body": "", "replyToId": old_id}),
    )
    .unwrap();
    let draft = call(&main, "get_draft", json!({"chatId": INBOX})).unwrap();
    assert_eq!(draft["body"], "");
    assert_eq!(draft["replyTo"]["id"], json!(old_id));
    assert_eq!(draft["replyTo"]["state"], "live");
    let error = call(
        &main,
        "save_draft",
        json!({"chatId": INBOX, "body": "x", "replyToId": "мусор"}),
    )
    .unwrap_err();
    assert_eq!(error, json!({"code": "reply_target_not_found"}));
    // Without replyToId the draft is a regular one.
    call(
        &main,
        "save_draft",
        json!({"chatId": INBOX, "body": "просто текст"}),
    )
    .unwrap();
    let plain = call(&main, "get_draft", json!({"chatId": INBOX})).unwrap();
    assert_eq!(
        plain,
        json!({"body": "просто текст", "replyTo": null, "attachments": []})
    );
}

#[test]
fn capture_send_ignores_a_reply_argument() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    let capture = window(&fixture, "capture");
    let old = call(
        &main,
        "send_message",
        json!({"chatId": INBOX, "body": "старая"}),
    )
    .unwrap();
    let outcome = call(
        &capture,
        "capture_send",
        json!({"body": "из окна захвата", "replyToId": old["id"]}),
    )
    .unwrap();
    assert_eq!(outcome["message"]["replyTo"], json!(null));
}

#[test]
fn on_this_day_and_dismiss_work_and_refuse_garbage() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    assert_eq!(
        call(&main, "get_on_this_day", json!({})).unwrap(),
        json!(null),
        "an empty database"
    );
    for bad in ["", "x", "2026-02-30", "../../etc"] {
        let error = call(&main, "dismiss_on_this_day", json!({"date": bad})).unwrap_err();
        assert_eq!(error, json!({"code": "invalid_date"}), "{bad}");
    }
    call(&main, "dismiss_on_this_day", json!({"date": "2026-10-31"})).unwrap();
}
