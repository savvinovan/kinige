//! Slice 7a (voice notes) over IPC on the mock runtime with the real capabilities: `send_voice` and `open_microphone_settings` belong to
//! the main window only (the capture window keeps its seven commands, the settings window is denied), and garbage comes back as errors
//! without a panic and without writing anything. The `audio` route is in `ipc_slice7a_route.rs`.

// A whole integration-test root is test code: `cfg(test)` gives helpers outside #[test] the same clippy.toml allowances
// (unwrap/expect/panic/indexing) as the tests in src/.
#![cfg(test)]

mod common;

use common::voice::*;
use common::*;
use kinige_lib::{CAPTURE_COMMANDS, MAIN_COMMANDS, SETTINGS_COMMANDS};
use serde_json::{Value, json};

const COMMANDS_7A: &[&str] = &[
    "send_voice",
    "open_microphone_settings",
    "read_voice_for_waveform",
    "set_attachment_waveform",
];

#[test]
fn the_voice_commands_are_main_only_and_the_capture_set_is_unchanged() {
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
    let capture = window(&fixture, "capture");
    let settings = window(&fixture, "settings");
    for command in COMMANDS_7A {
        assert!(MAIN_COMMANDS.contains(command), "{command}");
        assert!(!CAPTURE_COMMANDS.contains(command), "{command}");
        assert!(!SETTINGS_COMMANDS.contains(command), "{command}");
        // Only the settings page opens a native thing; the others are plain commands (the waveform ones are in `ipc_waveform.rs`).
        assert_eq!(
            NEEDS_NATIVE_UI.contains(command),
            *command == "open_microphone_settings",
            "{command}"
        );
        for denied in [&capture, &settings] {
            let error = call(denied, command, json!({}))
                .expect_err(&format!("{command} must be denied to this window"));
            assert!(is_acl_denial(&error), "{command}: {error}");
            let error = raw_call(
                denied,
                command,
                &[("x-chat-id", INBOX), ("x-duration-ms", "5000")],
                m4a(64),
            )
            .expect_err(&format!("{command} (raw) must be denied to this window"));
            assert!(is_acl_denial(&error), "{command}: {error}");
        }
    }
    // The denied raw body wrote nothing.
    let main = window(&fixture, "main");
    assert_eq!(feed_len(&main), 0);
    assert!(stored_files(&fixture).is_empty());
}

#[test]
fn a_recording_is_sent_as_an_entry_and_listed_in_the_feed() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    let sent = voice(&main, m4a(2048), "40000").unwrap();
    assert_eq!(sent["bodyMd"], "");
    let note = &sent["attachments"][0];
    assert_eq!(note["kind"], "audio");
    assert_eq!(note["mime"], "audio/mp4");
    assert_eq!(note["durationMs"], 40000);
    assert_eq!(note["byteSize"], 2048);
    assert_eq!(note["state"], "local");
    assert!(note["filename"].as_str().unwrap().starts_with("voice-"));
    assert!(
        note.get("blobKey").is_none()
            && note.get("remoteBlobId").is_none()
            && note.get("fileName").is_none(),
        "no key, no remote id, no stored name"
    );
    assert_eq!(feed_len(&main), 1);
    assert_eq!(stored_files(&fixture).len(), 1);
}

#[test]
fn garbage_is_an_error_without_a_panic_and_writes_nothing() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    let code = |result: Result<Value, Value>| -> String {
        let error = result.expect_err("must be refused");
        error["code"].as_str().unwrap_or("?").to_string()
    };

    // A JSON body is not a recording.
    assert_eq!(
        code(call(&main, "send_voice", json!({"chatId": INBOX}))),
        "attachment_unreadable"
    );
    // The bytes.
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
            code(voice(&main, bytes, "5000")),
            "attachment_unreadable",
            "{name}"
        );
    }
    // The duration header.
    for (name, ms) in [
        ("negative", "-5"),
        ("a sign", "+5000"),
        ("text", "abc"),
        ("exponent", "1e3"),
        ("fraction", "4.5"),
        ("hex", "0x1388"),
        ("overflow", "99999999999999999999"),
        ("past i64", "9223372036854775808"),
        ("spaces", " 5000"),
    ] {
        assert_eq!(
            code(voice(&main, m4a(64), ms)),
            "attachment_unreadable",
            "{name}"
        );
    }
    assert_eq!(
        code(raw_call(
            &main,
            "send_voice",
            &[("x-chat-id", INBOX)],
            m4a(64)
        )),
        "attachment_unreadable",
        "no duration header"
    );
    assert_eq!(code(voice(&main, m4a(64), "0")), "voice_too_short");
    assert_eq!(code(voice(&main, m4a(64), "999")), "voice_too_short");
    assert_eq!(code(voice(&main, m4a(64), "1805001")), "voice_too_long");
    assert_eq!(
        code(voice(&main, m4a(64), "9223372036854775807")),
        "voice_too_long"
    );
    // The chat and the reply target.
    assert_eq!(
        code(raw_call(
            &main,
            "send_voice",
            &[("x-duration-ms", "5000")],
            m4a(64)
        )),
        "chat_not_found",
        "no chat header"
    );
    assert_eq!(
        code(raw_call(
            &main,
            "send_voice",
            &[
                ("x-chat-id", "00000000-0000-4000-8000-0000000000aa"),
                ("x-duration-ms", "5000")
            ],
            m4a(64)
        )),
        "chat_not_found"
    );
    for target in [
        "00000000-0000-4000-8000-0000000000bb",
        "%2e%2e",
        "not-an-id",
    ] {
        assert_eq!(
            code(raw_call(
                &main,
                "send_voice",
                &[
                    ("x-chat-id", INBOX),
                    ("x-duration-ms", "5000"),
                    ("x-reply-to", target)
                ],
                m4a(64)
            )),
            "reply_target_not_found",
            "{target}"
        );
    }
    // Over the 64 MiB cap.
    assert_eq!(
        code(voice(&main, m4a(64 * 1024 * 1024 + 1), "5000")),
        "attachment_too_large"
    );

    assert_eq!(feed_len(&main), 0, "no entry");
    assert!(stored_files(&fixture).is_empty(), "no file");
    // An oversized header is not read.
    let long = "9".repeat(5000);
    assert_eq!(code(voice(&main, m4a(64), &long)), "attachment_unreadable");
    assert_eq!(feed_len(&main), 0);
}
