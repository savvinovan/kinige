//! Slice 7 over IPC on the mock runtime with the real capabilities: the attachment commands belong to the main window only (the
//! capture window keeps exactly seven commands, the settings window is denied), garbage comes back as errors without a panic and
//! without writing anything, the drop token is single-use, and the `kinige-media` protocol answers only `main`.

// A whole integration-test root is test code: `cfg(test)` gives helpers outside #[test] the same clippy.toml allowances
// (unwrap/expect/panic/indexing) as the tests in src/.
#![cfg(test)]

mod common;

use common::*;
use kinige_lib::attachments::DropTokens;
use kinige_lib::media_protocol::answer;
use kinige_lib::{CAPTURE_COMMANDS, MAIN_COMMANDS, SETTINGS_COMMANDS};
use kinige_store::Store;
use serde_json::{Value, json};
use std::path::PathBuf;
use tauri::http::{Method, Response, StatusCode, header};
use tauri::ipc::InvokeBody;
use tauri::test::{INVOKE_KEY, MockRuntime, get_ipc_response};
use tauri::{DragDropEvent, Manager, WebviewWindow, WindowEvent};

const INBOX: &str = kinige_domain::groups::INBOX_ID;

/// The commands of slice 7. The capture window and the settings window are denied each of them.
const SLICE_SEVEN: &[&str] = &[
    "attach_pick",
    "attach_paste",
    "attach_dropped",
    "remove_draft_attachment",
    "open_attachment",
];

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

fn png_bytes() -> Vec<u8> {
    let mut bytes = Vec::new();
    image::codecs::png::PngEncoder::new(&mut bytes)
        .write_image(&[200u8; 4 * 4 * 3], 4, 4, image::ExtendedColorType::Rgb8)
        .unwrap();
    bytes
}

use image::ImageEncoder;

fn wait_until(what: &str, mut condition: impl FnMut() -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while std::time::Instant::now() < deadline {
        if condition() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(15));
    }
    panic!("timed out waiting for: {what}");
}

#[test]
fn slice_seven_commands_are_main_only_and_the_capture_set_is_unchanged() {
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
    for command in SLICE_SEVEN {
        assert!(MAIN_COMMANDS.contains(command), "{command}");
        assert!(!CAPTURE_COMMANDS.contains(command), "{command}");
        assert!(!SETTINGS_COMMANDS.contains(command), "{command}");
        for denied in [&capture, &settings] {
            let error = call(denied, command, args_for(command))
                .expect_err(&format!("{command} must be denied to this window"));
            assert!(is_acl_denial(&error), "{command}: {error}");
            let error = raw_call(denied, command, &[("x-chat-id", INBOX)], png_bytes())
                .expect_err(&format!("{command} (raw) must be denied to this window"));
            assert!(is_acl_denial(&error), "{command}: {error}");
        }
        // The dialog cannot be opened in a test; the others reach their handler for the main window.
        if !NEEDS_NATIVE_UI.contains(command)
            && let Err(error) = call(&main, command, args_for(command))
        {
            assert!(!is_acl_denial(&error), "{command}: {error}");
        }
    }
}

#[test]
fn a_pasted_image_becomes_a_tile_and_is_sent_with_the_entry() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    let tile = raw_call(
        &main,
        "attach_paste",
        &[
            ("x-chat-id", INBOX),
            ("x-name-hint", "%D1%81%D0%BA%D1%80%D0%B8%D0%BD.png"),
        ],
        png_bytes(),
    )
    .unwrap();
    assert_eq!(tile["filename"], "скрин.png");
    assert_eq!(tile["kind"], "image");
    assert_eq!(
        (tile["width"].clone(), tile["height"].clone()),
        (json!(4), json!(4))
    );
    assert!(
        tile.get("blobKey").is_none()
            && tile.get("remoteBlobId").is_none()
            && tile.get("fileName").is_none()
    );
    let draft = call(&main, "get_draft", json!({"chatId": INBOX})).unwrap();
    assert_eq!(draft["attachments"][0]["id"], tile["id"]);

    // Sending with no text at all works because of the attachment.
    let sent = call(
        &main,
        "send_message",
        json!({"chatId": INBOX, "body": "", "attachmentIds": [tile["id"]]}),
    )
    .unwrap();
    assert_eq!(sent["bodyMd"], "");
    assert_eq!(sent["attachments"][0]["id"], tile["id"]);
    let page = call(&main, "list_messages", json!({"chatId": INBOX})).unwrap();
    assert_eq!(
        page["messages"][0]["attachments"][0]["filename"],
        "скрин.png"
    );
    assert_eq!(
        call(&main, "get_draft", json!({"chatId": INBOX})).unwrap()["attachments"],
        json!([])
    );

    // Without an attachment an empty entry is still an error.
    let error = call(
        &main,
        "send_message",
        json!({"chatId": INBOX, "body": "  "}),
    )
    .unwrap_err();
    assert_eq!(error, json!({"code": "empty_message"}));
}

#[test]
fn garbage_pastes_and_ids_are_errors_without_a_panic() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    // Not a raw body.
    let error = call(&main, "attach_paste", json!({"chatId": INBOX})).unwrap_err();
    assert_eq!(error, json!({"code": "attachment_unreadable"}));
    // No chat header, an unknown chat, a header that is not text, a megabyte of header.
    let no_chat = raw_call(&main, "attach_paste", &[], png_bytes()).unwrap_err();
    assert_eq!(no_chat, json!({"code": "chat_not_found"}));
    for chat in [
        "",
        "нет-такого",
        "'; DROP TABLE chat; --",
        &"x".repeat(1 << 12),
    ] {
        let result = raw_call(&main, "attach_paste", &[("x-chat-id", chat)], png_bytes());
        assert_eq!(
            result.unwrap_err(),
            json!({"code": "chat_not_found"}),
            "{chat:.20}"
        );
    }
    // Empty, random and huge-name pastes are accepted as files or refused cleanly, never a panic.
    let weird_names = [
        "%00",
        "%2e%2e%2f%2e%2e%2fetc%2fpasswd",
        &"%D1%8F".repeat(600),
        "%ZZ",
        "%E2%80%AE.exe",
    ];
    for name in weird_names {
        let result = raw_call(
            &main,
            "attach_paste",
            &[("x-chat-id", INBOX), ("x-name-hint", name)],
            vec![0u8; 16],
        );
        if let Ok(tile) = result {
            let shown = tile["filename"].as_str().unwrap();
            assert!(
                shown.len() <= 255 && !shown.contains(['/', '\\', '\0']),
                "{shown:?}"
            );
        }
    }
    let empty = raw_call(&main, "attach_paste", &[("x-chat-id", INBOX)], Vec::new()).unwrap();
    assert_eq!(empty["byteSize"], 0);
    // Over the paste limit.
    let big = vec![0u8; kinige_domain::attachment::MAX_PASTE_BYTES + 1];
    let error = raw_call(&main, "attach_paste", &[("x-chat-id", INBOX)], big).unwrap_err();
    assert_eq!(error, json!({"code": "attachment_too_large"}));

    // Garbage ids on send, remove and open.
    let long = "z".repeat(1 << 20);
    for ids in [
        json!(["нет"]),
        json!([long]),
        json!([""]),
        json!(["00000000-0000-4000-8000-0000000000aa"]),
        json!((0..11).map(|_| "x").collect::<Vec<_>>()),
    ] {
        let result = call(
            &main,
            "send_message",
            json!({"chatId": INBOX, "body": "текст", "attachmentIds": ids}),
        );
        let error = result.unwrap_err();
        assert!(
            error == json!({"code": "attachment_not_found"})
                || error == json!({"code": "attachment_limit"}),
            "{error}"
        );
    }
    for bad in [
        json!({"chatId": INBOX, "body": "x", "attachmentIds": "not an array"}),
        json!({"chatId": INBOX, "body": "x", "attachmentIds": [1, 2]}),
    ] {
        assert!(call(&main, "send_message", bad).is_err());
    }
    let missing = call(
        &main,
        "remove_draft_attachment",
        json!({"chatId": INBOX, "attachmentId": "'; --"}),
    )
    .unwrap_err();
    assert_eq!(missing, json!({"code": "attachment_not_found"}));
    let open = call(
        &main,
        "open_attachment",
        json!({"attachmentId": "../../etc/passwd"}),
    )
    .unwrap_err();
    assert_eq!(open, json!({"code": "attachment_not_found"}));
    // The failures wrote nothing: every file in `attachments/` belongs to a tile, and no entry was created.
    let store = fixture.app.state::<Store>();
    let tiles = store
        .chat_draft(INBOX, &kinige_store::Now::system())
        .unwrap()
        .attachments
        .len();
    let files = std::fs::read_dir(fixture.data.path().join("attachments"))
        .unwrap()
        .flatten()
        .filter(|entry| !entry.file_name().to_string_lossy().starts_with('.'))
        .count();
    assert_eq!(files, tiles, "no file without a tile");
    let page = call(&main, "list_messages", json!({"chatId": INBOX})).unwrap();
    assert_eq!(page["messages"], json!([]));
}

#[test]
fn a_drop_leaves_a_single_use_token_and_only_the_main_window_counts() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    let capture = window(&fixture, "capture");
    let settings = window(&fixture, "settings");
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("dropped.png");
    std::fs::write(&file, png_bytes()).unwrap();
    std::fs::create_dir(dir.path().join("folder")).unwrap();
    let drop = |paths: Vec<PathBuf>| {
        WindowEvent::DragDrop(DragDropEvent::Drop {
            paths,
            position: tauri::PhysicalPosition::new(1.0, 1.0),
        })
    };
    let tokens = fixture.app.state::<DropTokens>();

    // A drop on the capture window or the settings window does nothing.
    for other in [&capture, &settings] {
        kinige_lib::on_window_event(&other.as_ref().window(), &drop(vec![file.clone()]));
    }
    assert_eq!(tokens.pending_for_test(), 0);

    // The main window keeps the paths in Rust and tells JS only a token.
    let listened = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    {
        let listened = listened.clone();
        use tauri::Listener;
        main.listen("attachments-dropped", move |event| {
            listened.lock().unwrap().push(event.payload().to_string())
        });
    }
    kinige_lib::on_window_event(
        &main.as_ref().window(),
        &drop(vec![file.clone(), dir.path().join("folder")]),
    );
    assert_eq!(tokens.pending_for_test(), 1);
    wait_until("the event", || !listened.lock().unwrap().is_empty());
    let payload = listened.lock().unwrap()[0].clone();
    assert!(
        !payload.contains("dropped.png") && !payload.contains(dir.path().to_str().unwrap()),
        "no path in the event: {payload}"
    );
    let token: String = serde_json::from_str(&payload).unwrap();

    // Another token, an old token, and a made-up one fail; the real one works once.
    for bad in [
        "",
        "нет",
        "00000000-0000-4000-8000-0000000000aa",
        &"t".repeat(1 << 16),
    ] {
        let error = call(
            &main,
            "attach_dropped",
            json!({"token": bad, "chatId": INBOX}),
        )
        .unwrap_err();
        assert_eq!(error, json!({"code": "attachment_not_found"}));
    }
    let outcome = call(
        &main,
        "attach_dropped",
        json!({"token": token, "chatId": INBOX}),
    )
    .unwrap();
    assert_eq!(outcome["attached"].as_array().unwrap().len(), 1);
    assert_eq!(
        outcome["skippedFolders"], 1,
        "a folder is skipped, not attached"
    );
    assert_eq!(outcome["error"], Value::Null);
    let again = call(
        &main,
        "attach_dropped",
        json!({"token": token, "chatId": INBOX}),
    )
    .unwrap_err();
    assert_eq!(again, json!({"code": "attachment_not_found"}), "single use");

    // An old token has expired.
    let old = tokens.put_aged_for_test(vec![file.clone()], std::time::Duration::from_secs(61));
    let expired = call(
        &main,
        "attach_dropped",
        json!({"token": old, "chatId": INBOX}),
    )
    .unwrap_err();
    assert_eq!(expired, json!({"code": "attachment_not_found"}));
    // The tokens are capped.
    for _ in 0..40 {
        tokens.put(vec![file.clone()]);
    }
    assert!(tokens.pending_for_test() <= 8);
    // A drop of something that is not a file (a path that does not exist) is an error in the outcome, with the folder count intact.
    let token = tokens.put(vec![dir.path().join("gone.png")]);
    let outcome = call(
        &main,
        "attach_dropped",
        json!({"token": token, "chatId": INBOX}),
    )
    .unwrap();
    assert_eq!(outcome["error"], "attachment_unreadable");
    assert_eq!(outcome["attached"], json!([]));
}

fn status_of(store: &Store, label: &str, method: Method, path: &str) -> Response<Vec<u8>> {
    let (tx, rx) = std::sync::mpsc::channel();
    answer(store, label, &method, path, move |response| {
        tx.send(response).unwrap()
    });
    rx.recv_timeout(std::time::Duration::from_secs(20)).unwrap()
}

#[test]
fn the_media_protocol_answers_only_main_and_only_exact_paths() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    let png = |name: &str| {
        raw_call(
            &main,
            "attach_paste",
            &[("x-chat-id", INBOX), ("x-name-hint", name)],
            png_bytes(),
        )
        .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string()
    };
    let (photo, svg) = (png("p.png"), {
        let tile = raw_call(
            &main,
            "attach_paste",
            &[("x-chat-id", INBOX), ("x-name-hint", "logo.svg")],
            b"<svg onload='x'/>".to_vec(),
        )
        .unwrap();
        tile["id"].as_str().unwrap().to_string()
    });
    call(
        &main,
        "send_message",
        json!({"chatId": INBOX, "body": "", "attachmentIds": [photo, svg]}),
    )
    .unwrap();
    let store = fixture.app.state::<Store>();

    for variant in ["thumb", "full"] {
        let ok = status_of(&store, "main", Method::GET, &format!("/{variant}/{photo}"));
        assert_eq!(ok.status(), StatusCode::OK, "{variant}");
        assert_eq!(
            ok.headers()[header::CONTENT_TYPE],
            if variant == "full" {
                "image/png"
            } else {
                "image/jpeg"
            }
        );
        assert_eq!(ok.headers()[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
        assert!(
            ok.headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .is_none()
        );
        assert!(!ok.body().is_empty());
        for other in ["capture", "settings", "", "main2", "Main"] {
            let denied = status_of(&store, other, Method::GET, &format!("/{variant}/{photo}"));
            assert_eq!(denied.status(), StatusCode::FORBIDDEN, "{other}");
            assert!(denied.body().is_empty(), "a refusal carries no body");
            assert!(
                denied
                    .headers()
                    .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                    .is_none()
            );
        }
    }
    let not_found = [
        "/".to_string(),
        "/thumb".to_string(),
        "/thumb/..".to_string(),
        "/thumb/%2e%2e".to_string(),
        "/thumb/%2e%2e%2f%2e%2e%2fetc%2fpasswd".to_string(),
        "/../thumb/".to_string() + &photo,
        format!("/thumb/{}", photo.to_uppercase()),
        format!("/thumb/{photo}/extra"),
        format!("/other/{photo}"),
        "/thumb/00000000-0000-4000-8000-0000000000aa".to_string(),
        format!("/thumb/{}", "a".repeat(1 << 16)),
    ];
    for path in not_found {
        let response = status_of(&store, "main", Method::GET, &path);
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path:.60}");
        assert!(response.body().is_empty());
    }
    // SVG is stored but never served, not even by id.
    for variant in ["thumb", "full"] {
        assert_eq!(
            status_of(&store, "main", Method::GET, &format!("/{variant}/{svg}")).status(),
            StatusCode::NOT_FOUND
        );
    }
    assert_eq!(
        status_of(&store, "main", Method::POST, &format!("/thumb/{photo}")).status(),
        StatusCode::METHOD_NOT_ALLOWED
    );
    // A purged id is a 404.
    let id: String = serde_json::from_value(
        call(&main, "list_messages", json!({"chatId": INBOX})).unwrap()["messages"][0]["id"]
            .clone(),
    )
    .unwrap();
    call(&main, "delete_message", json!({"id": id})).unwrap();
    call(&main, "purge_message", json!({"id": id})).unwrap();
    assert_eq!(
        status_of(&store, "main", Method::GET, &format!("/full/{photo}")).status(),
        StatusCode::NOT_FOUND
    );
}
