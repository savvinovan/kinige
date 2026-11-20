//! Slice 5c, the picture side over IPC (§12.3): `kinige-media://…/sticker/<id>` answers only `main` and only a library id, a sticker travels
//! through the library, the feed and the entry's own refusals, and the CSP is as it was before the slice (the web view never reaches Telegram).

#![cfg(test)]

mod common;

use common::bot_env::add_pack;
use common::*;
use kinige_lib::media_protocol::answer;
use kinige_store::Store;
use serde_json::{Value, json};
use tauri::Manager;
use tauri::http::{Method, Response, StatusCode};

const INBOX: &str = kinige_domain::groups::INBOX_ID;

fn status_of(store: &Store, label: &str, method: Method, path: &str) -> Response<Vec<u8>> {
    let (tx, rx) = std::sync::mpsc::channel();
    answer(store, label, &method, path, move |response| {
        tx.send(response).unwrap()
    });
    rx.recv_timeout(std::time::Duration::from_secs(20)).unwrap()
}

#[test]
fn the_sticker_path_of_the_protocol_answers_only_main_with_a_library_id() {
    let fixture = fixture();
    let id = add_pack(&fixture);
    let store = fixture.app.state::<Store>();
    let ok = status_of(&store, "main", Method::GET, &format!("/sticker/{id}"));
    assert_eq!(ok.status(), StatusCode::OK);
    assert_eq!(ok.headers().get("content-type").unwrap(), "image/webp");
    assert_eq!(
        ok.headers().get("x-content-type-options").unwrap(),
        "nosniff"
    );
    assert!(ok.body().starts_with(b"RIFF"));
    for label in ["capture", "settings", "other", ""] {
        assert_eq!(
            status_of(&store, label, Method::GET, &format!("/sticker/{id}")).status(),
            StatusCode::FORBIDDEN,
            "{label}"
        );
    }
    assert_eq!(
        status_of(&store, "main", Method::POST, &format!("/sticker/{id}")).status(),
        StatusCode::METHOD_NOT_ALLOWED
    );
    for bad in [
        "/sticker/".to_string(),
        "/sticker/..".to_string(),
        "/sticker/%2e%2e".to_string(),
        format!("/sticker/{}", id.to_uppercase()),
        format!("/sticker/{id}/"),
        format!("/sticker/{id}.webp"),
        format!("/sticker/{id}/extra"),
        "/sticker/00000000-0000-4000-8000-000000000000".to_string(),
        format!("/sticker/{}", "a".repeat(1 << 16)),
        "/stickers/x".to_string(),
    ] {
        assert_eq!(
            status_of(&store, "main", Method::GET, &bad).status(),
            StatusCode::NOT_FOUND,
            "{bad:.80}"
        );
    }
    // An attachment id is not a library id: the photo route is its own.
    let tile = store
        .send_sticker(INBOX, &id, None, &kinige_store::Now::system())
        .unwrap();
    let attachment = &tile.attachments[0].id;
    assert_eq!(
        status_of(
            &store,
            "main",
            Method::GET,
            &format!("/sticker/{attachment}")
        )
        .status(),
        StatusCode::NOT_FOUND
    );
}

#[test]
fn a_sticker_travels_through_the_feed_and_the_library_over_ipc() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    let id = add_pack(&fixture);
    let library = call(&main, "get_sticker_library", json!({})).unwrap();
    assert_eq!(library["packs"][0]["title"], "Test Pack");
    assert_eq!(library["packs"][0]["stickers"][0]["emoji"], "😺");
    assert_eq!(library["recent"], json!([]));
    let sent = call(
        &main,
        "send_sticker",
        json!({"chatId": INBOX, "stickerId": id}),
    )
    .unwrap();
    assert_eq!(sent["bodyMd"], "");
    assert_eq!(sent["attachments"][0]["sticker"], true);
    assert_eq!(sent["attachments"][0]["stickerEmoji"], "😺");
    let page = call(&main, "list_messages", json!({"chatId": INBOX})).unwrap();
    assert_eq!(page["messages"][0]["attachments"][0]["sticker"], true);
    assert_eq!(
        call(&main, "get_sticker_library", json!({})).unwrap()["recent"][0]["id"],
        id.as_str()
    );
    // The entry is not edited, and "Edit" has nothing to change.
    let edit = call(
        &main,
        "edit_message",
        json!({"id": sent["id"], "body": "x"}),
    );
    assert_eq!(
        edit.unwrap_err(),
        json!({"code": "sticker_entry_read_only"})
    );
    let pack = library["packs"][0]["id"].as_str().unwrap();
    call(&main, "remove_sticker_pack", json!({"packId": pack})).unwrap();
    assert_eq!(
        call(&main, "get_sticker_library", json!({})).unwrap()["packs"],
        json!([])
    );
    // Cancel with no job is not an error, twice.
    call(&main, "cancel_sticker_pack", json!({})).unwrap();
    call(&main, "cancel_sticker_pack", json!({})).unwrap();
    assert_eq!(
        call(&main, "list_journal_gifs", json!({})).unwrap(),
        Value::Array(vec![])
    );
}

#[test]
fn the_csp_is_as_before_the_slice() {
    let config: Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
    let csp = &config["app"]["security"]["csp"];
    assert_eq!(csp["connect-src"], "ipc: http://ipc.localhost");
    assert_eq!(
        csp["img-src"],
        "'self' data: kinige-media: http://kinige-media.localhost"
    );
    assert_eq!(
        csp["media-src"],
        "'self' kinige-media: http://kinige-media.localhost"
    );
    assert_eq!(csp["default-src"], "'self'");
    let all = csp.to_string();
    assert!(
        !all.contains("telegram") && !all.contains("https:"),
        "{all}"
    );
}
