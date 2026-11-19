//! Slice 7b over IPC on the mock runtime with the real capabilities: the two details commands belong to the main window only (the
//! capture window keeps exactly seven commands, the settings window is denied both), the details travel with the draft and the sent
//! entry, and garbage comes back as an error without a panic and without writing anything.

// A whole integration-test root is test code: `cfg(test)` gives helpers outside #[test] the same clippy.toml allowances
// (unwrap/expect/panic/indexing) as the tests in src/.
#![cfg(test)]

mod common;

use common::*;
use image::ImageEncoder;
use kinige_lib::{CAPTURE_COMMANDS, MAIN_COMMANDS, SETTINGS_COMMANDS};
use kinige_store::{Now, Store};
use serde_json::{Value, json};
use tauri::Manager;

const INBOX: &str = kinige_domain::groups::INBOX_ID;
const COMMANDS: &[&str] = &["set_draft_attachment_details", "set_attachment_details"];

fn png_bytes() -> Vec<u8> {
    let mut bytes = Vec::new();
    image::codecs::png::PngEncoder::new(&mut bytes)
        .write_image(&[200u8; 4 * 4 * 3], 4, 4, image::ExtendedColorType::Rgb8)
        .unwrap();
    bytes
}

/// A draft tile of the photo `IMG_0001.png` in Inbox, made through the store (the paste is `ipc_slice7.rs`'s).
fn photo_tile(fixture: &Fixture) -> String {
    fixture
        .app
        .state::<Store>()
        .attach_bytes(INBOX, Some("IMG_0001.png"), &png_bytes(), &Now::system())
        .unwrap()
        .id
}

fn groups_of(fixture: &Fixture, id: &str) -> Option<String> {
    let store = fixture.app.state::<Store>();
    let _ = &store;
    let conn = rusqlite::Connection::open(fixture.data.path().join("kinige.db")).ok()?;
    conn.query_row(
        "SELECT groups_raw FROM attachment WHERE id = ?1",
        [id],
        |r| r.get(0),
    )
    .ok()
}

#[test]
fn the_details_commands_are_main_only_and_the_capture_set_is_unchanged() {
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
    for command in COMMANDS {
        assert!(MAIN_COMMANDS.contains(command), "{command}");
        assert!(!CAPTURE_COMMANDS.contains(command), "{command}");
        assert!(!SETTINGS_COMMANDS.contains(command), "{command}");
        for denied in [&capture, &settings] {
            let error = call(denied, command, args_for(command))
                .expect_err(&format!("{command} must be denied to this window"));
            assert!(is_acl_denial(&error), "{command}: {error}");
        }
        // The main window reaches the handler: an unknown attachment is the store's answer, not the ACL's.
        let error = call(&main, command, args_for(command)).unwrap_err();
        assert_eq!(error, json!({"code": "attachment_not_found"}), "{command}");
    }
}

#[test]
fn details_travel_with_the_draft_and_the_sent_entry_and_an_edit_does_not_make_it_edited() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    let tile = photo_tile(&fixture);

    let edited = call(
        &main,
        "set_draft_attachment_details",
        json!({"attachmentId": tile, "changes": {"name": "пляж", "description": "Пляж, июль", "spoiler": true}}),
    )
    .unwrap();
    assert_eq!(
        (
            &edited["name"],
            &edited["description"],
            &edited["spoiler"],
            &edited["filename"]
        ),
        (
            &json!("пляж.png"),
            &json!("Пляж, июль"),
            &json!(true),
            &json!("IMG_0001.png")
        )
    );
    let draft = call(&main, "get_draft", json!({"chatId": INBOX})).unwrap();
    assert_eq!(draft["attachments"][0], edited);

    let sent = call(
        &main,
        "send_message",
        json!({"chatId": INBOX, "body": "", "attachmentIds": [tile]}),
    )
    .unwrap();
    assert_eq!(sent["attachments"][0], edited);

    // After sending: only the spoiler is sent (the context menu), the name and the description stay.
    let shown = call(
        &main,
        "set_attachment_details",
        json!({"attachmentId": tile, "changes": {"spoiler": false}}),
    )
    .unwrap();
    assert_eq!(
        (&shown["name"], &shown["description"], &shown["spoiler"]),
        (&json!("пляж.png"), &json!("Пляж, июль"), &json!(false))
    );
    let page = call(&main, "list_messages", json!({"chatId": INBOX})).unwrap();
    assert_eq!(page["messages"][0]["attachments"][0], shown);
    assert_eq!(page["messages"][0]["id"], sent["id"]);
    // The entry's own text and groups were not touched: nothing in the view says "edited", and its bytes are what the send wrote.
    let store = fixture.app.state::<Store>();
    let revisions = store.search("пляж", &Default::default(), None, 5).unwrap();
    assert_eq!(revisions.hits.len(), 1, "found by the new name");
    assert!(groups_of(&fixture, &tile).is_some_and(|text| text.contains("пляж.png")));
}

#[test]
fn garbage_arguments_are_errors_without_a_panic_and_write_nothing() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    let tile = photo_tile(&fixture);
    let sent = call(
        &main,
        "send_message",
        json!({"chatId": INBOX, "body": "", "attachmentIds": [tile]}),
    )
    .unwrap();
    let before = groups_of(&fixture, &tile);
    let draft_tile = photo_tile(&fixture);
    let draft_before = call(&main, "get_draft", json!({"chatId": INBOX})).unwrap();

    let megabyte = "x".repeat(1 << 20);
    let too_long = "я".repeat(1025);
    let long_id = "z".repeat(1 << 16);
    // Rules of Rust: `attachment_details_invalid`.
    for changes in [
        json!({"name": "a/b"}),
        json!({"name": "a\\b"}),
        json!({"name": "a\u{0}b"}),
        json!({"name": "evil\u{202e}gnp"}),
        json!({"description": megabyte}),
        json!({"description": too_long}),
        json!({"description": "a\u{0}b"}),
        json!({"description": "\u{1b}[2J"}),
    ] {
        for (command, id) in [
            ("set_attachment_details", &tile),
            ("set_draft_attachment_details", &draft_tile),
        ] {
            let result = call(
                &main,
                command,
                json!({"attachmentId": id, "changes": changes}),
            );
            let expected = if command == "set_draft_attachment_details" && id == &tile {
                "attachment_not_found"
            } else {
                "attachment_details_invalid"
            };
            let error = result.unwrap_err();
            let label = changes.to_string().chars().take(30).collect::<String>();
            assert_eq!(error, json!({"code": expected}), "{command} {label}");
        }
    }
    // A spoiler on a file card is refused by Rust too: send a PDF and try.
    let store = fixture.app.state::<Store>();
    let pdf = store
        .attach_bytes(INBOX, Some("menu.pdf"), b"%PDF-1.4", &Now::system())
        .unwrap();
    let error = call(
        &main,
        "set_draft_attachment_details",
        json!({"attachmentId": pdf.id, "changes": {"spoiler": true}}),
    )
    .unwrap_err();
    assert_eq!(error, json!({"code": "attachment_details_invalid"}));

    // Ids that are not a draft or an attachment.
    for id in [
        json!(""),
        json!("'; DROP TABLE attachment; --"),
        json!("../../etc"),
        json!(long_id),
        json!("00000000-0000-4000-8000-0000000000aa"),
    ] {
        for command in COMMANDS {
            let error = call(
                &main,
                command,
                json!({"attachmentId": id, "changes": {"name": "x"}}),
            )
            .unwrap_err();
            assert_eq!(error, json!({"code": "attachment_not_found"}), "{command}");
        }
    }
    // Arguments that do not read: `spoiler: null`, an unknown key, a mistyped value, not an object, a missing argument.
    for bad in [
        json!({"attachmentId": tile, "changes": {"spoiler": null}}),
        json!({"attachmentId": tile, "changes": {"spoiler": "yes"}}),
        json!({"attachmentId": tile, "changes": {"spoiler": 1}}),
        json!({"attachmentId": tile, "changes": {"name": 7}}),
        json!({"attachmentId": tile, "changes": {"description": ["a"]}}),
        json!({"attachmentId": tile, "changes": {"blobKey": "x"}}),
        json!({"attachmentId": tile, "changes": {"filename": "x.png"}}),
        json!({"attachmentId": tile, "changes": "name"}),
        json!({"attachmentId": tile, "changes": ["name"]}),
        json!({"attachmentId": tile, "changes": null}),
        json!({"attachmentId": tile}),
        json!({"changes": {"name": "x"}}),
        json!({"attachmentId": 7, "changes": {}}),
        json!({}),
    ] {
        for command in COMMANDS {
            assert!(
                call(&main, command, bad.clone()).is_err(),
                "{command} {bad}"
            );
        }
    }
    // Nothing was written: the sent attachment's bytes and the draft are what they were.
    assert_eq!(groups_of(&fixture, &tile), before);
    let draft_after = call(&main, "get_draft", json!({"chatId": INBOX})).unwrap();
    let kept: Vec<&Value> = draft_after["attachments"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|a| a["id"] != json!(pdf.id))
        .collect();
    assert_eq!(
        json!(kept),
        draft_before["attachments"],
        "the other tiles are what they were"
    );
    let page = call(&main, "list_messages", json!({"chatId": INBOX})).unwrap();
    assert_eq!(page["messages"][0]["id"], sent["id"]);
    assert_eq!(
        page["messages"][0]["attachments"][0]["name"],
        "IMG_0001.png"
    );
}
