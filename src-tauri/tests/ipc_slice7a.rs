//! Slice 7a (voice notes) over IPC on the mock runtime with the real capabilities: `send_voice` and `open_microphone_settings` belong to
//! the main window only (the capture window keeps its seven commands, the settings window is denied), garbage comes back as errors
//! without a panic and without writing anything, and the `audio` route of the `kinige-media` protocol answers only `main`, only
//! `audio` rows with the right bytes, with correct `Range` answers.

// A whole integration-test root is test code: `cfg(test)` gives helpers outside #[test] the same clippy.toml allowances
// (unwrap/expect/panic/indexing) as the tests in src/.
#![cfg(test)]

mod common;

use common::*;
use kinige_lib::audio_route::{self, MAX_ANSWER_BYTES};
use kinige_lib::media_protocol::answer_range;
use kinige_lib::{CAPTURE_COMMANDS, MAIN_COMMANDS, SETTINGS_COMMANDS};
use kinige_store::{AudioFile, Store};
use serde_json::{Value, json};
use std::fs::OpenOptions;
use std::io::Write;
use tauri::http::{Method, Response, StatusCode, header};
use tauri::ipc::InvokeBody;
use tauri::test::{INVOKE_KEY, MockRuntime, get_ipc_response};
use tauri::{Manager, WebviewWindow};

const INBOX: &str = kinige_domain::groups::INBOX_ID;
const COMMANDS_7A: &[&str] = &["send_voice", "open_microphone_settings"];

fn raw_call(
    window: &WebviewWindow<MockRuntime>,
    cmd: &str,
    headers: &[(&str, &str)],
    body: Vec<u8>,
) -> Result<Value, Value> {
    let mut map = tauri::http::HeaderMap::new();
    for (name, value) in headers {
        map.insert(
            tauri::http::HeaderName::from_bytes(name.as_bytes()).unwrap(),
            tauri::http::HeaderValue::from_str(value).unwrap(),
        );
    }
    let mut request = request(cmd, json!({}));
    request.body = InvokeBody::Raw(body);
    request.headers = map;
    request.invoke_key = INVOKE_KEY.to_string();
    get_ipc_response(window, request)
        .map(|body: tauri::ipc::InvokeResponseBody| body.deserialize().unwrap())
}

/// AAC in MP4 as far as the store looks: `ftyp` at offset 4.
fn m4a(len: usize) -> Vec<u8> {
    let mut bytes = vec![0, 0, 0, 0x20];
    bytes.extend_from_slice(b"ftypM4A ");
    bytes.resize(len.max(12), 0x5a);
    bytes
}

fn voice(window: &WebviewWindow<MockRuntime>, bytes: Vec<u8>, ms: &str) -> Result<Value, Value> {
    raw_call(
        window,
        "send_voice",
        &[("x-chat-id", INBOX), ("x-duration-ms", ms)],
        bytes,
    )
}

fn feed_len(window: &WebviewWindow<MockRuntime>) -> usize {
    call(window, "list_messages", json!({"chatId": INBOX})).unwrap()["messages"]
        .as_array()
        .unwrap()
        .len()
}

fn stored_files(fixture: &Fixture) -> Vec<String> {
    std::fs::read_dir(fixture.data.path().join("attachments"))
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .filter(|name| !name.starts_with('.'))
                .collect()
        })
        .unwrap_or_default()
}

fn get(
    store: &Store,
    label: &str,
    method: Method,
    path: &str,
    range: Option<&str>,
) -> Response<Vec<u8>> {
    let (tx, rx) = std::sync::mpsc::channel();
    let range = range.map(str::to_string);
    answer_range(
        store,
        label,
        &method,
        path,
        range.as_deref(),
        move |response| tx.send(response).unwrap(),
    );
    rx.recv_timeout(std::time::Duration::from_secs(20)).unwrap()
}

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
        assert!(NEEDS_NATIVE_UI.contains(command) || *command == "send_voice");
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

#[test]
fn the_audio_route_answers_only_main_and_only_audio_with_correct_ranges() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    let size = 2 * 1024 * 1024 + 345;
    let sent = voice(&main, m4a(size), "40000").unwrap();
    let id = sent["attachments"][0]["id"].as_str().unwrap().to_string();
    let store = fixture.app.state::<Store>();
    let path = format!("/audio/{id}");

    // No Range: 200 with the whole file, Content-Length and Accept-Ranges.
    let whole = get(&store, "main", Method::GET, &path, None);
    assert_eq!(whole.status(), StatusCode::OK);
    assert_eq!(whole.body().len(), size);
    assert_eq!(whole.headers()[header::CONTENT_TYPE], "audio/mp4");
    assert_eq!(whole.headers()[header::CONTENT_LENGTH], size.to_string());
    assert_eq!(whole.headers()[header::ACCEPT_RANGES], "bytes");
    assert_eq!(whole.headers()[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
    assert!(
        whole.headers()[header::CONTENT_SECURITY_POLICY]
            .to_str()
            .unwrap()
            .contains("sandbox")
    );
    assert!(
        whole
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .is_none()
    );
    assert_eq!(&whole.body()[4..8], b"ftyp");
    // A unit other than bytes is ignored, as is a header that is not a range.
    for other in ["items=0-5", "bytes=abc", "nonsense"] {
        let response = get(&store, "main", Method::GET, &path, Some(other));
        assert_eq!(response.status(), StatusCode::OK, "{other}");
        assert_eq!(response.body().len(), size, "{other}");
    }

    // One byte, a window, an open range (cut at 1 MiB), a suffix (also larger than the file), the last byte.
    let part = |range: &str| get(&store, "main", Method::GET, &path, Some(range));
    let first = part("bytes=0-0");
    assert_eq!(first.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(first.body().len(), 1);
    assert_eq!(
        first.headers()[header::CONTENT_RANGE],
        format!("bytes 0-0/{size}")
    );
    assert_eq!(first.headers()[header::CONTENT_LENGTH], "1");
    let window_range = part("bytes=4-7");
    assert_eq!(window_range.body().as_slice(), b"ftyp");
    assert_eq!(
        window_range.headers()[header::CONTENT_RANGE],
        format!("bytes 4-7/{size}")
    );
    let open = part("bytes=0-");
    assert_eq!(open.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        open.body().len() as u64,
        MAX_ANSWER_BYTES,
        "one answer is capped at 1 MiB"
    );
    assert_eq!(
        open.headers()[header::CONTENT_LENGTH],
        MAX_ANSWER_BYTES.to_string()
    );
    assert_eq!(
        open.headers()[header::CONTENT_RANGE],
        format!("bytes 0-{}/{size}", MAX_ANSWER_BYTES - 1)
    );
    let tail = part("bytes=-10");
    assert_eq!(tail.body().len(), 10);
    assert_eq!(
        tail.headers()[header::CONTENT_RANGE],
        format!("bytes {}-{}/{size}", size - 10, size - 1)
    );
    let huge_suffix = part("bytes=-99999999999");
    assert_eq!(huge_suffix.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(huge_suffix.body().len() as u64, MAX_ANSWER_BYTES);
    let last = part(&format!("bytes={}-", size - 1));
    assert_eq!(last.body().len(), 1);
    let past_end = part(&format!("bytes={}-{}", size - 5, size + 1000));
    assert_eq!(past_end.body().len(), 5, "the end is clipped to the file");

    // 416: past the end, reversed, a multi-range, an empty suffix. It carries the length and no body.
    for range in [
        format!("bytes={size}-"),
        format!("bytes={}-", size + 1),
        "bytes=9-3".to_string(),
        "bytes=0-0,5-9".to_string(),
        "bytes=-0".to_string(),
        "bytes=99999999999999999999999-".to_string(),
    ] {
        let response = part(&range);
        assert_eq!(
            response.status(),
            StatusCode::RANGE_NOT_SATISFIABLE,
            "{range}"
        );
        assert_eq!(
            response.headers()[header::CONTENT_RANGE],
            format!("bytes */{size}"),
            "{range}"
        );
        assert!(response.body().is_empty(), "{range}");
        assert!(
            response
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .is_none()
        );
    }

    // Other windows, other methods, other paths.
    for other in ["capture", "settings", "", "main2", "Main"] {
        let denied = get(&store, other, Method::GET, &path, None);
        assert_eq!(denied.status(), StatusCode::FORBIDDEN, "{other}");
        assert!(denied.body().is_empty());
        assert!(
            denied
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .is_none()
        );
        let denied = get(&store, other, Method::GET, &path, Some("bytes=0-1"));
        assert_eq!(
            denied.status(),
            StatusCode::FORBIDDEN,
            "{other} with a range"
        );
    }
    assert_eq!(
        get(&store, "main", Method::POST, &path, None).status(),
        StatusCode::METHOD_NOT_ALLOWED
    );
    let bad_paths = [
        "/audio".to_string(),
        "/audio/".to_string(),
        "/audio/..".to_string(),
        "/audio/%2e%2e".to_string(),
        format!("/audio/{}", id.to_uppercase()),
        format!("/audio/{id}/"),
        format!("/audio/{id}/extra"),
        format!("/audio/{id}.m4a"),
        format!("/../audio/{id}"),
        format!("/Audio/{id}"),
        "/audio/00000000-0000-4000-8000-0000000000aa".to_string(),
        format!("/audio/{}", "a".repeat(1 << 16)),
    ];
    for bad in bad_paths {
        let response = get(&store, "main", Method::GET, &bad, None);
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{bad:.60}");
        assert!(response.body().is_empty());
    }
}

#[test]
fn the_audio_route_refuses_what_is_not_a_playable_audio_row() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    let store = fixture.app.state::<Store>();
    let note = |name: &str| -> (String, std::path::PathBuf) {
        let sent = voice(&main, m4a(4096), "5000").unwrap();
        let id = sent["attachments"][0]["id"].as_str().unwrap().to_string();
        let path = fixture.data.path().join(format!("attachments/{id}.m4a"));
        assert!(path.is_file(), "{name}");
        (id, path)
    };
    let status =
        |id: &str| get(&store, "main", Method::GET, &format!("/audio/{id}"), None).status();

    // An image sent as a photo is not audio.
    let tile = raw_call(
        &main,
        "attach_paste",
        &[("x-chat-id", INBOX), ("x-name-hint", "p.png")],
        {
            let mut bytes = Vec::new();
            image::ImageEncoder::write_image(
                image::codecs::png::PngEncoder::new(&mut bytes),
                &[200u8; 4 * 4 * 3],
                4,
                4,
                image::ExtendedColorType::Rgb8,
            )
            .unwrap();
            bytes
        },
    )
    .unwrap();
    let photo = tile["id"].as_str().unwrap().to_string();
    call(
        &main,
        "send_message",
        json!({"chatId": INBOX, "body": "", "attachmentIds": [photo]}),
    )
    .unwrap();
    assert_eq!(
        status(&photo),
        StatusCode::NOT_FOUND,
        "an image on the audio route"
    );

    // A file whose first bytes are an HTML page, under a row that says audio/mp4.
    let (html, html_path) = note("html");
    std::fs::write(&html_path, b"<html><script>alert(1)</script></html>").unwrap();
    assert_eq!(status(&html), StatusCode::NOT_FOUND);

    // A file that is gone.
    let (gone, gone_path) = note("gone");
    std::fs::remove_file(&gone_path).unwrap();
    assert_eq!(status(&gone), StatusCode::NOT_FOUND);

    // An empty file.
    let (empty, empty_path) = note("empty");
    std::fs::write(&empty_path, b"").unwrap();
    assert_eq!(status(&empty), StatusCode::NOT_FOUND);

    // A file over 64 MiB (sparse), whatever its row claims.
    let (big, big_path) = note("big");
    OpenOptions::new()
        .write(true)
        .open(&big_path)
        .unwrap()
        .set_len(64 * 1024 * 1024 + 1)
        .unwrap();
    assert_eq!(status(&big), StatusCode::NOT_FOUND);
    // And at exactly 64 MiB it is served (a range keeps the answer small).
    OpenOptions::new()
        .write(true)
        .open(&big_path)
        .unwrap()
        .set_len(64 * 1024 * 1024)
        .unwrap();
    let ok = get(
        &store,
        "main",
        Method::GET,
        &format!("/audio/{big}"),
        Some("bytes=0-3"),
    );
    assert_eq!(ok.status(), StatusCode::PARTIAL_CONTENT);

    // A row whose MIME type is off the allowlist.
    let (odd, _) = note("odd mime");
    rusqlite::Connection::open(fixture.data.path().join("kinige.db"))
        .unwrap()
        .execute(
            "UPDATE attachment SET mime = 'text/html' WHERE id = ?1",
            [&odd],
        )
        .unwrap();
    assert_eq!(status(&odd), StatusCode::NOT_FOUND);

    // A row marked `missing` by the sweep.
    let (missing, _) = note("missing");
    rusqlite::Connection::open(fixture.data.path().join("kinige.db"))
        .unwrap()
        .execute(
            "UPDATE attachment_local SET state = 'missing' WHERE attachment_id = ?1",
            [&missing],
        )
        .unwrap();
    assert_eq!(status(&missing), StatusCode::NOT_FOUND);

    // A purged id.
    let (purged, _) = note("purged");
    let entry = call(&main, "list_messages", json!({"chatId": INBOX})).unwrap()["messages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["attachments"][0]["id"] == purged.as_str())
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    call(&main, "delete_message", json!({"id": entry})).unwrap();
    call(&main, "purge_message", json!({"id": entry})).unwrap();
    assert_eq!(status(&purged), StatusCode::NOT_FOUND);
}

#[test]
fn a_file_that_shrinks_under_the_handler_is_a_416_never_a_short_body() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.m4a");
    let mut file = std::fs::File::create(&path).unwrap();
    file.write_all(&m4a(100)).unwrap();
    drop(file);
    // The handler believes the file has 1000 bytes (it had, a moment ago); it has 100.
    let audio = |len| AudioFile {
        file: std::fs::File::open(&path).unwrap(),
        len,
        content_type: "audio/mp4",
    };
    for range in [
        None,
        Some("bytes=0-"),
        Some("bytes=200-300"),
        Some("bytes=-500"),
    ] {
        let response = audio_route::respond(audio(1000), range);
        assert_eq!(
            response.status(),
            StatusCode::RANGE_NOT_SATISFIABLE,
            "{range:?}"
        );
        assert!(response.body().is_empty(), "{range:?}");
    }
    // The truth is still served.
    let response = audio_route::respond(audio(100), Some("bytes=0-"));
    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(response.body().len(), 100);
}
