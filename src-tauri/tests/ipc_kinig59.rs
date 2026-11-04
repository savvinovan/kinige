//! KINIG-59 over IPC on the mock runtime with the real capabilities: `set_chat_color` belongs to the main window only (the capture
//! window keeps exactly seven commands, the settings window is denied), the colour reaches the chat list, search hits and trash
//! plates resolved, and garbage comes back as an error without writing anything.

// A whole integration-test root is test code: `cfg(test)` gives helpers outside #[test] the same clippy.toml allowances
// (unwrap/expect/panic/indexing) as the tests in src/.
#![cfg(test)]

mod common;

use common::*;
use kinige_lib::{CAPTURE_COMMANDS, MAIN_COMMANDS, SETTINGS_COMMANDS};
use serde_json::{Value, json};

const INBOX: &str = kinige_domain::groups::INBOX_ID;

fn chat_rows(fixture: &Fixture) -> Vec<(String, Option<String>, String)> {
    let conn = rusqlite::Connection::open(fixture.data.path().join("kinige.db")).unwrap();
    let mut statement = conn
        .prepare("SELECT id, color, groups_raw FROM chat ORDER BY id")
        .unwrap();
    statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

fn colour_of(window: &tauri::WebviewWindow<tauri::test::MockRuntime>, id: &str) -> Value {
    call(window, "list_chats", json!({})).unwrap()["chats"]
        .as_array()
        .unwrap()
        .iter()
        .find(|chat| chat["id"] == id)
        .unwrap()["color"]
        .clone()
}

#[test]
fn set_chat_color_is_main_only_and_the_capture_set_is_unchanged() {
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
    assert!(MAIN_COMMANDS.contains(&"set_chat_color"));
    assert!(!CAPTURE_COMMANDS.contains(&"set_chat_color"));
    assert!(!SETTINGS_COMMANDS.contains(&"set_chat_color"));
    let fixture = fixture();
    let main = window(&fixture, "main");
    for label in ["capture", "settings", "not-main"] {
        let denied = window(&fixture, label);
        let error = call(&denied, "set_chat_color", args_for("set_chat_color"))
            .expect_err("denied to this window");
        assert!(is_acl_denial(&error), "{label}: {error}");
    }
    // For the main window the call reaches the handler: Inbox cannot be recoloured.
    let error = call(&main, "set_chat_color", args_for("set_chat_color")).unwrap_err();
    assert_eq!(error, json!({"code": "inbox_protected"}));
}

#[test]
fn a_picked_colour_shows_in_the_chat_list_search_and_trash() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    let chat = call(&main, "create_chat", json!({"title": "Проекты"})).unwrap();
    let id = chat["id"].as_str().unwrap();
    let automatic = chat["color"].clone();
    assert!(automatic.is_string() && automatic != "grey", "{automatic}");
    assert_eq!(colour_of(&main, INBOX), "grey");

    let picked = call(&main, "set_chat_color", json!({"id": id, "color": "grey"})).unwrap();
    assert_eq!(picked["color"], "grey");
    assert_eq!(colour_of(&main, id), "grey");

    call(
        &main,
        "send_message",
        json!({"chatId": id, "body": "цветнойпроверочный"}),
    )
    .unwrap();
    let found = call(
        &main,
        "search_messages",
        json!({"query": "цветнойпроверочный"}),
    )
    .unwrap();
    assert_eq!(found["hits"][0]["chatColor"], "grey");
    let by_name = call(&main, "search_messages", json!({"query": "Проекты"})).unwrap();
    assert_eq!(by_name["chats"][0]["color"], "grey");

    let message_id = found["hits"][0]["message"]["id"].as_str().unwrap();
    call(&main, "delete_message", json!({"id": message_id})).unwrap();
    let trash = call(&main, "list_trash", json!({})).unwrap();
    assert_eq!(trash[0]["chatColor"], "grey");
}

#[test]
fn garbage_arguments_are_errors_and_write_nothing() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    let chat = call(&main, "create_chat", json!({"title": "Проекты"})).unwrap();
    let id = chat["id"].as_str().unwrap().to_string();
    let before = chat_rows(&fixture);

    let long = "я".repeat(5_000);
    for color in [
        json!("red"),
        json!(""),
        json!("#fff"),
        json!(null),
        json!(1),
        json!(true),
        json!(["blue"]),
        json!({"name": "blue"}),
        json!("BLUE"),
        json!(" blue"),
        json!("blue\u{0}"),
        json!(long),
    ] {
        let error = call(&main, "set_chat_color", json!({"id": id, "color": color}))
            .expect_err("a colour that is not one of the seven names is refused");
        assert!(!is_acl_denial(&error), "{color}: {error}");
    }
    for args in [
        json!({}),
        json!({"id": id}),
        json!({"color": "blue"}),
        json!({"id": 5, "color": "blue"}),
        json!({"id": null, "color": "blue"}),
    ] {
        let error = call(&main, "set_chat_color", args).expect_err("missing or mistyped arguments");
        assert!(!is_acl_denial(&error), "{error}");
    }
    for bad_id in [
        json!(""),
        json!("нет-такого"),
        json!("00000000-0000-4000-8000-000000000000"),
        json!("я".repeat(5_000)),
        json!("'; DROP TABLE chat; --"),
    ] {
        let error = call(
            &main,
            "set_chat_color",
            json!({"id": bad_id, "color": "blue"}),
        )
        .expect_err("an unknown chat is refused");
        assert_eq!(error, json!({"code": "chat_not_found"}));
    }
    assert_eq!(chat_rows(&fixture), before, "nothing was written");
    assert_eq!(colour_of(&main, &id), chat["color"]);
}
