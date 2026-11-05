//! Shared by the slice 7a IPC tests: a raw-body command call, a synthetic recording, `send_voice`, the feed length, the stored files
//! and a request to the `kinige-media` handler.

use super::*;
use kinige_lib::media_protocol::answer_range;
use kinige_store::Store;
use serde_json::{Value, json};
use tauri::WebviewWindow;
use tauri::http::{Method, Response};
use tauri::ipc::InvokeBody;
use tauri::test::{INVOKE_KEY, MockRuntime, get_ipc_response};

pub const INBOX: &str = kinige_domain::groups::INBOX_ID;

pub fn raw_call(
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
pub fn m4a(len: usize) -> Vec<u8> {
    let mut bytes = vec![0, 0, 0, 0x20];
    bytes.extend_from_slice(b"ftypM4A ");
    bytes.resize(len.max(12), 0x5a);
    bytes
}

pub fn voice(
    window: &WebviewWindow<MockRuntime>,
    bytes: Vec<u8>,
    ms: &str,
) -> Result<Value, Value> {
    raw_call(
        window,
        "send_voice",
        &[("x-chat-id", INBOX), ("x-duration-ms", ms)],
        bytes,
    )
}

pub fn feed_len(window: &WebviewWindow<MockRuntime>) -> usize {
    call(window, "list_messages", json!({"chatId": INBOX})).unwrap()["messages"]
        .as_array()
        .unwrap()
        .len()
}

pub fn stored_files(fixture: &Fixture) -> Vec<String> {
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

pub fn get(
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

/// A command whose answer is a raw body (`read_voice_for_waveform`).
pub fn bytes_call(
    window: &WebviewWindow<MockRuntime>,
    cmd: &str,
    args: Value,
) -> Result<Vec<u8>, Value> {
    get_ipc_response(window, request(cmd, args)).map(|body: tauri::ipc::InvokeResponseBody| {
        match body {
            tauri::ipc::InvokeResponseBody::Raw(bytes) => bytes,
            tauri::ipc::InvokeResponseBody::Json(text) => text.into_bytes(),
        }
    })
}

/// `send_voice` with the recorder's peaks in `x-waveform`.
pub fn voice_with_waveform(
    window: &WebviewWindow<MockRuntime>,
    bytes: Vec<u8>,
    waveform: &str,
) -> Result<Value, Value> {
    raw_call(
        window,
        "send_voice",
        &[
            ("x-chat-id", INBOX),
            ("x-duration-ms", "40000"),
            ("x-waveform", waveform),
        ],
        bytes,
    )
}

/// Peaks as the 128 hex digits the wire carries.
pub fn peaks_hex(seed: u8) -> String {
    (0..64u8)
        .map(|i| format!("{:02x}", seed.wrapping_add(i.wrapping_mul(3))))
        .collect()
}
