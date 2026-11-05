//! The `audio` route of the `kinige-media` protocol (slice 7a): only `main`, only `GET`, only `audio` rows in state `local` with the right
//! first bytes and a MIME type on the allowlist, correct `Range` answers, 404 above 64 MiB, and a file that shrinks under the handler is
//! a 416, never a short body.

#![cfg(test)]

mod common;

use common::voice::*;
use common::*;
use kinige_lib::audio_route::{self, MAX_ANSWER_BYTES};
use kinige_store::{AudioFile, Store};
use serde_json::json;
use std::fs::OpenOptions;
use std::io::Write;
use tauri::Manager;
use tauri::http::{Method, StatusCode, header};

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
