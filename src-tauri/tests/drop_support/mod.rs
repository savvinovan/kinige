//! Shared by `ipc_drop_payloads.rs` and `ipc_drop_limits.rs` (KINIG-133): a fake drag, the drop events, and a window that receives
//! them. The drag is read through a fake `DragSource`, so no window and no pasteboard are needed; the drop event goes to the real
//! handler, and the token is claimed over real IPC.

#![cfg(test)]
#![allow(dead_code)] // each test root uses a part of it

use crate::common::*;
use kinige_lib::attachments::on_drag_drop_from;
use kinige_lib::drag_source::{DragImage, DragSource};
use serde_json::{Value, json};
use std::cell::Cell;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::ipc::InvokeBody;
use tauri::test::{INVOKE_KEY, MockRuntime, get_ipc_response};
use tauri::{DragDropEvent, Listener, PhysicalPosition, WebviewWindow};

pub const INBOX: &str = kinige_domain::groups::INBOX_ID;

/// A drag that carries what the test says, and counts how often it was asked.
#[derive(Default)]
pub struct FakeDrag {
    pub paths: Vec<PathBuf>,
    pub image: Option<DragImage>,
    /// The pasteboard's change count: another drag has another one.
    pub generation: i64,
    pub path_reads: Cell<u32>,
    pub image_reads: Cell<u32>,
}

impl DragSource for FakeDrag {
    fn file_paths(&self) -> Vec<PathBuf> {
        self.path_reads.set(self.path_reads.get() + 1);
        self.paths.clone()
    }

    fn image(&self) -> Option<DragImage> {
        self.image_reads.set(self.image_reads.get() + 1);
        self.image.clone()
    }

    fn generation(&self) -> i64 {
        self.generation
    }

    fn type_names(&self) -> Vec<String> {
        vec!["public.utf8-plain-text".to_string()]
    }
}

pub fn png_bytes() -> Vec<u8> {
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
}

pub fn small_tiff() -> Vec<u8> {
    let mut bytes = Vec::new();
    image::DynamicImage::new_rgb8(24, 16)
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Tiff,
        )
        .unwrap();
    bytes
}

pub fn enter(paths: Vec<PathBuf>) -> DragDropEvent {
    DragDropEvent::Enter {
        paths,
        position: PhysicalPosition::new(1.0, 1.0),
    }
}

pub fn drop_of(paths: Vec<PathBuf>) -> DragDropEvent {
    DragDropEvent::Drop {
        paths,
        position: PhysicalPosition::new(1.0, 1.0),
    }
}

/// Sends the events to the handler and returns the token the window was told about by the last drop.
pub struct Dropper {
    pub fixture: Fixture,
    pub main: WebviewWindow<MockRuntime>,
    pub tokens: Arc<Mutex<Vec<String>>>,
}

impl Dropper {
    /// A window with a chat open (the web view has told Rust so).
    pub fn new() -> Self {
        let dropper = Self::without_chat();
        call(&dropper.main, "set_drop_target", json!({"active": true})).unwrap();
        dropper
    }

    /// A window with no chat open (the trash, or nothing yet): no image is read.
    pub fn without_chat() -> Self {
        let fixture = fixture();
        let main = window(&fixture, "main");
        let tokens = Arc::new(Mutex::new(Vec::<String>::new()));
        {
            let tokens = tokens.clone();
            main.listen("attachments-dropped", move |event| {
                let token: String = serde_json::from_str(event.payload()).unwrap();
                tokens.lock().unwrap().push(token);
            });
        }
        Dropper {
            fixture,
            main,
            tokens,
        }
    }

    pub fn send(&self, event: &DragDropEvent, source: &FakeDrag) {
        on_drag_drop_from(self.fixture.app.handle(), event, source);
    }

    /// The token of the `n`-th drop (1-based), once the event has arrived.
    pub fn token(&self, n: usize) -> String {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while std::time::Instant::now() < deadline {
            if let Some(token) = self.tokens.lock().unwrap().get(n - 1) {
                return token.clone();
            }
            std::thread::sleep(std::time::Duration::from_millis(15));
        }
        panic!("no drop event {n}");
    }

    pub fn claim(&self, token: &str) -> Result<Value, Value> {
        call(
            &self.main,
            "attach_dropped",
            json!({"token": token, "chatId": INBOX}),
        )
    }

    pub fn draft_tiles(&self) -> usize {
        call(&self.main, "get_draft", json!({"chatId": INBOX})).unwrap()["attachments"]
            .as_array()
            .unwrap()
            .len()
    }
}

pub fn raw_paste(window: &WebviewWindow<MockRuntime>, bytes: Vec<u8>) -> Result<Value, Value> {
    let mut headers = tauri::http::HeaderMap::new();
    headers.insert(
        tauri::http::HeaderName::from_static("x-chat-id"),
        tauri::http::HeaderValue::from_str(INBOX).unwrap(),
    );
    let mut request = request("attach_paste", json!({}));
    request.body = InvokeBody::Raw(bytes);
    request.headers = headers;
    request.invoke_key = INVOKE_KEY.to_string();
    get_ipc_response(window, request)
        .map(|body: tauri::ipc::InvokeResponseBody| body.deserialize().unwrap())
}
