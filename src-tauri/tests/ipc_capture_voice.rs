//! Voice notes from the quick capture window (KINIG-153) over IPC on the mock runtime with the real capabilities: the capture window has three
//! commands of its own for them (`capture_send_voice`, `capture_hold_microphone`, `capture_open_microphone_settings`), writes only to
//! Inbox, leaves its text draft alone and shares one microphone with the main window; the other windows are denied; garbage comes back as
//! errors without a panic and writes nothing.

// A whole integration-test root is test code: `cfg(test)` gives helpers outside #[test] the same clippy.toml allowances as the tests in src/.
#![cfg(test)]

mod common;

use common::voice::*;
use common::*;
use serde_json::{Value, json};

const VOICE_SET: [&str; 3] = [
    "capture_send_voice",
    "capture_hold_microphone",
    "capture_open_microphone_settings",
];

fn code(result: Result<Value, Value>) -> String {
    let error = result.expect_err("must be refused");
    error["code"].as_str().unwrap_or("?").to_string()
}

fn capture_voice(
    window: &tauri::WebviewWindow<tauri::test::MockRuntime>,
    bytes: Vec<u8>,
    ms: &str,
) -> Result<Value, Value> {
    raw_call(
        window,
        "capture_send_voice",
        &[("x-duration-ms", ms)],
        bytes,
    )
}

#[test]
fn the_three_voice_commands_belong_to_the_capture_window_alone() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    let settings = window(&fixture, "settings");
    for command in VOICE_SET {
        assert!(kinige_lib::CAPTURE_COMMANDS.contains(&command), "{command}");
        assert!(!kinige_lib::MAIN_COMMANDS.contains(&command), "{command}");
        assert!(
            !kinige_lib::SETTINGS_COMMANDS.contains(&command),
            "{command}"
        );
        for denied in [&main, &settings] {
            let error = call(denied, command, json!({"held": true}))
                .expect_err(&format!("{command} must be denied to this window"));
            assert!(is_acl_denial(&error), "{command}: {error}");
            let error = raw_call(denied, command, &[("x-duration-ms", "5000")], m4a(64))
                .expect_err(&format!("{command} (raw) must be denied to this window"));
            assert!(is_acl_denial(&error), "{command}: {error}");
        }
    }
    // The microphone claim of the main window is denied to the capture window and the settings window, and the other way round.
    let capture = window(&fixture, "capture");
    for denied in [&capture, &settings] {
        let error = call(denied, "microphone_hold", json!({"held": true})).unwrap_err();
        assert!(is_acl_denial(&error), "{error}");
    }
    // The commands that read, search or name a chat stay out of reach, voice or not.
    for command in [
        "send_voice",
        "list_messages",
        "search_messages",
        "read_voice_for_waveform",
    ] {
        let error = call(&capture, command, args_for(command)).unwrap_err();
        assert!(is_acl_denial(&error), "{command}: {error}");
    }
    assert_eq!(feed_len(&main), 0);
}

#[test]
fn a_capture_recording_lands_in_inbox_and_the_text_draft_stays() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    let capture = window(&fixture, "capture");
    call(
        &capture,
        "capture_save_draft",
        json!({"body": "незаконченная мысль"}),
    )
    .unwrap();

    let sent = capture_voice(&capture, m4a(2048), "7000").unwrap();
    assert_eq!(sent["bodyMd"], "");
    assert_eq!(sent["chatId"], INBOX);
    let note = &sent["attachments"][0];
    assert_eq!(note["kind"], "audio");
    assert_eq!(note["mime"], "audio/mp4");
    assert_eq!(note["durationMs"], 7000);
    assert_eq!(
        feed_len(&main),
        1,
        "the note is in the Inbox feed the main window reads"
    );
    assert_eq!(stored_files(&fixture).len(), 1);

    let state = call(&capture, "capture_state", json!({})).unwrap();
    assert_eq!(
        state["draft"], "незаконченная мысль",
        "the text draft is not touched"
    );
    let context = state["context"].as_array().unwrap();
    assert_eq!(context.len(), 1, "the new note is in the window's own list");
    assert_eq!(context[0]["attachments"][0]["kind"], "audio");
}

#[test]
fn the_capture_window_cannot_aim_a_recording_at_another_chat_or_a_reply() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    let capture = window(&fixture, "capture");
    let other = call(
        &main,
        "create_chat",
        json!({"title": "Книги", "icon": "📚"}),
    )
    .unwrap();
    let other_id = other["id"].as_str().unwrap().to_string();
    let first = capture_voice(&capture, m4a(256), "5000").unwrap();
    // A page that adds the headers of `send_voice` gets nothing from them.
    let sent = raw_call(
        &capture,
        "capture_send_voice",
        &[
            ("x-duration-ms", "5000"),
            ("x-chat-id", other_id.as_str()),
            ("x-reply-to", first["id"].as_str().unwrap()),
        ],
        m4a(256),
    )
    .unwrap();
    assert_eq!(sent["chatId"], INBOX);
    assert!(sent["replyTo"].is_null(), "no reply: {sent}");
    let other_feed = call(&main, "list_messages", json!({"chatId": other_id})).unwrap();
    assert_eq!(other_feed["messages"].as_array().unwrap().len(), 0);
    assert_eq!(feed_len(&main), 2);
}

#[test]
fn garbage_sent_from_the_capture_window_is_an_error_and_writes_nothing() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    let capture = window(&fixture, "capture");

    // A JSON body is not a recording.
    assert_eq!(
        code(call(&capture, "capture_send_voice", json!({}))),
        "attachment_unreadable"
    );
    let noise: Vec<u8> = (0..1_048_576u32)
        .map(|i| (i.wrapping_mul(2_654_435_761) >> 24) as u8)
        .collect();
    for (name, bytes) in [
        ("empty", Vec::new()),
        ("one byte", vec![0x1A]),
        ("noise", noise),
        ("a PNG", b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec()),
        ("html", b"<html><script>1</script>".to_vec()),
    ] {
        assert_eq!(
            code(capture_voice(&capture, bytes, "5000")),
            "attachment_unreadable",
            "{name}"
        );
    }
    let long = "9".repeat(5000);
    for (name, ms) in [
        ("negative", "-5"),
        ("a sign", "+5000"),
        ("text", "abc"),
        ("exponent", "1e3"),
        ("fraction", "4.5"),
        ("hex", "0x1388"),
        ("overflow", "99999999999999999999"),
        ("spaces", " 5000"),
        ("long", long.as_str()),
    ] {
        assert_eq!(
            code(capture_voice(&capture, m4a(64), ms)),
            "attachment_unreadable",
            "{name}"
        );
    }
    assert_eq!(
        code(raw_call(&capture, "capture_send_voice", &[], m4a(64))),
        "attachment_unreadable",
        "no duration header"
    );
    assert_eq!(
        code(capture_voice(&capture, m4a(64), "999")),
        "voice_too_short"
    );
    assert_eq!(
        code(capture_voice(&capture, m4a(64), "1805001")),
        "voice_too_long"
    );
    assert_eq!(
        code(capture_voice(&capture, m4a(64 * 1024 * 1024 + 1), "5000")),
        "attachment_too_large"
    );
    assert_eq!(feed_len(&main), 0, "no entry");
    assert!(stored_files(&fixture).is_empty(), "no file");
}

#[test]
fn one_recording_at_a_time_across_the_two_windows() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    let capture = window(&fixture, "capture");
    let hold =
        |window: &tauri::WebviewWindow<tauri::test::MockRuntime>, command: &str, held: bool| {
            call(window, command, json!({ "held": held }))
                .unwrap()
                .as_bool()
                .unwrap()
        };

    // The main window records: the capture window is refused, whatever it asks, and its release changes nothing.
    assert!(hold(&main, "microphone_hold", true));
    assert!(
        hold(&main, "microphone_hold", true),
        "holding it again is fine"
    );
    assert!(!hold(&capture, "capture_hold_microphone", true));
    assert!(
        hold(&capture, "capture_hold_microphone", false),
        "letting go is always allowed"
    );
    assert!(
        !hold(&capture, "capture_hold_microphone", true),
        "...and does not free the main window's claim"
    );
    // The main window lets go: now the capture window may, and the main window is refused in turn.
    assert!(hold(&main, "microphone_hold", false));
    assert!(hold(&capture, "capture_hold_microphone", true));
    assert!(!hold(&main, "microphone_hold", true));
    assert!(
        hold(&main, "microphone_hold", false),
        "...and its release does not free the capture window's claim"
    );
    assert!(!hold(&main, "microphone_hold", true));
    assert!(hold(&capture, "capture_hold_microphone", false));
    assert!(hold(&main, "microphone_hold", true));
}

#[test]
fn the_claim_commands_refuse_garbage_arguments_without_a_panic() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    let capture = window(&fixture, "capture");
    for (window, command) in [
        (&main, "microphone_hold"),
        (&capture, "capture_hold_microphone"),
    ] {
        for args in [
            json!({}),
            json!({"held": "yes"}),
            json!({"held": 1}),
            json!({"held": null}),
            json!([]),
            json!("held"),
        ] {
            assert!(
                call(window, command, args.clone()).is_err(),
                "{command} {args}"
            );
        }
    }
    // None of that took the microphone.
    assert!(
        call(&capture, "capture_hold_microphone", json!({"held": true}))
            .unwrap()
            .as_bool()
            .unwrap()
    );
}
