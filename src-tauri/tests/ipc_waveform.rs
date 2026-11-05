//! The local waveform of a voice note over IPC on the mock runtime with the real capabilities (slice 7a, ADR-0015): `send_voice` with
//! the recorder's peaks, `read_voice_for_waveform` and `set_attachment_waveform` for the main window only, and garbage as errors with
//! no write and no panic.

#![cfg(test)]

mod common;

use common::voice::*;
use common::*;
use serde_json::{Value, json};

fn code(result: Result<impl std::fmt::Debug, Value>) -> String {
    let error = result.expect_err("must be refused");
    error["code"].as_str().unwrap_or("?").to_string()
}

fn first_note(window: &tauri::WebviewWindow<tauri::test::MockRuntime>) -> Value {
    let feed = call(window, "list_messages", json!({"chatId": INBOX})).unwrap();
    feed["messages"][0]["attachments"][0].clone()
}

#[test]
fn the_peaks_of_a_recording_come_back_in_the_entry_and_in_the_feed() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    let sent = voice_with_waveform(&main, m4a(2048), &peaks_hex(5)).unwrap();
    let peaks = &sent["attachments"][0]["waveform"];
    assert_eq!(peaks.as_array().unwrap().len(), 64);
    assert_eq!(peaks[0], 5);
    assert_eq!(first_note(&main)["waveform"], *peaks);
}

#[test]
fn a_note_without_peaks_says_null_and_garbage_peaks_never_cost_the_note() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    let plain = voice(&main, m4a(256), "40000").unwrap();
    assert!(plain["attachments"][0]["waveform"].is_null());
    let long = "ab".repeat(3000);
    for (name, text) in [
        ("short", "abcd".to_string()),
        ("not hex", "zz".repeat(64)),
        ("odd", "a".repeat(127)),
        ("one too many", "ab".repeat(65)),
        ("oversized", long),
    ] {
        let sent = voice_with_waveform(&main, m4a(256), &text)
            .unwrap_or_else(|error| panic!("{name}: the note was refused: {error}"));
        assert!(sent["attachments"][0]["waveform"].is_null(), "{name}");
    }
    assert_eq!(feed_len(&main), 6);
}

#[test]
fn the_decoder_reads_the_stored_bytes_and_saves_its_peaks() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    let bytes = m4a(4096);
    let sent = voice(&main, bytes.clone(), "40000").unwrap();
    let id = sent["attachments"][0]["id"].as_str().unwrap().to_string();

    let read = bytes_call(
        &main,
        "read_voice_for_waveform",
        json!({"attachmentId": id}),
    )
    .unwrap();
    assert_eq!(read, bytes);

    call(
        &main,
        "set_attachment_waveform",
        json!({"attachmentId": id, "peaks": peaks_hex(9)}),
    )
    .unwrap();
    assert_eq!(first_note(&main)["waveform"][0], 9);
    // The first value wins.
    call(
        &main,
        "set_attachment_waveform",
        json!({"attachmentId": id, "peaks": peaks_hex(100)}),
    )
    .unwrap();
    assert_eq!(first_note(&main)["waveform"][0], 9);
}

#[test]
fn the_failed_marker_is_the_empty_text() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    let sent = voice(&main, m4a(256), "40000").unwrap();
    let id = sent["attachments"][0]["id"].as_str().unwrap().to_string();
    call(
        &main,
        "set_attachment_waveform",
        json!({"attachmentId": id, "peaks": ""}),
    )
    .unwrap();
    assert_eq!(first_note(&main)["waveform"], json!([]));
}

#[test]
fn garbage_for_the_decoder_commands_is_an_error_and_writes_nothing() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    let sent = voice(&main, m4a(256), "40000").unwrap();
    let id = sent["attachments"][0]["id"].as_str().unwrap().to_string();

    for (name, peaks) in [
        ("one byte", "ab".to_string()),
        ("not hex", "zz".repeat(64)),
        ("odd", "a".repeat(127)),
        ("one too many", "ab".repeat(65)),
        ("huge", "ab".repeat(100_000)),
        ("wide", "９".repeat(64)),
        ("sign", "+1".repeat(64)),
    ] {
        let result = call(
            &main,
            "set_attachment_waveform",
            json!({"attachmentId": id, "peaks": peaks}),
        );
        assert_eq!(code(result), "attachment_unreadable", "{name}");
    }
    for bad_id in [
        "",
        "x",
        "../x",
        "00000000-0000-4000-8000-0000000000aa",
        "нет",
    ] {
        assert_eq!(
            code(call(
                &main,
                "set_attachment_waveform",
                json!({"attachmentId": bad_id, "peaks": peaks_hex(1)})
            )),
            "attachment_not_found",
            "{bad_id:?}"
        );
        assert_eq!(
            code(bytes_call(
                &main,
                "read_voice_for_waveform",
                json!({"attachmentId": bad_id})
            )),
            "attachment_not_found",
            "{bad_id:?}"
        );
    }
    // Wrong argument shapes: an error, never a panic.
    for args in [
        json!({}),
        json!({"attachmentId": id}),
        json!({"attachmentId": id, "peaks": 5}),
        json!({"attachmentId": null, "peaks": null}),
        json!({"attachmentId": [1, 2], "peaks": {"a": 1}}),
    ] {
        assert!(call(&main, "set_attachment_waveform", args).is_err());
    }
    assert!(
        first_note(&main)["waveform"].is_null(),
        "nothing was written"
    );
}

#[test]
fn only_the_main_window_may_read_the_bytes_or_save_peaks() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    let sent = voice(&main, m4a(256), "40000").unwrap();
    let id = sent["attachments"][0]["id"].as_str().unwrap().to_string();
    for label in ["capture", "settings"] {
        let denied = window(&fixture, label);
        let error = bytes_call(
            &denied,
            "read_voice_for_waveform",
            json!({"attachmentId": id}),
        )
        .expect_err("denied");
        assert!(is_acl_denial(&error), "{label}: {error}");
        let error = call(
            &denied,
            "set_attachment_waveform",
            json!({"attachmentId": id, "peaks": peaks_hex(1)}),
        )
        .expect_err("denied");
        assert!(is_acl_denial(&error), "{label}: {error}");
    }
    assert!(
        first_note(&main)["waveform"].is_null(),
        "a denied call wrote nothing"
    );
}
