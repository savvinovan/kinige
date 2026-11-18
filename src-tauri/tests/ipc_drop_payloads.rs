//! What a drop on the main window can carry (KINIG-133, slice 7): files (Finder), a picture with no file (Telegram Desktop, a browser:
//! image data on the drag pasteboard), or nothing the app understands. The drag is read through a fake `DragSource`, so no window and
//! no pasteboard are needed; the drop event is the real one, sent to the real handler, and the token is claimed over real IPC.

// A whole integration-test root is test code (see `ipc_slice7.rs`).
#![cfg(test)]

mod common;

use common::*;
use kinige_lib::attachments::on_drag_drop_from;
use kinige_lib::drag_source::{DragImage, DragSource};
use serde_json::{Value, json};
use std::cell::Cell;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::test::MockRuntime;
use tauri::{DragDropEvent, Listener, PhysicalPosition, WebviewWindow};

const INBOX: &str = kinige_domain::groups::INBOX_ID;

/// A drag that carries what the test says, and counts how often it was asked.
#[derive(Default)]
struct FakeDrag {
    paths: Vec<PathBuf>,
    image: Option<DragImage>,
    path_reads: Cell<u32>,
    image_reads: Cell<u32>,
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
}

fn png_bytes() -> Vec<u8> {
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

fn enter(paths: Vec<PathBuf>) -> DragDropEvent {
    DragDropEvent::Enter {
        paths,
        position: PhysicalPosition::new(1.0, 1.0),
    }
}

fn drop_of(paths: Vec<PathBuf>) -> DragDropEvent {
    DragDropEvent::Drop {
        paths,
        position: PhysicalPosition::new(1.0, 1.0),
    }
}

/// Sends the events to the handler and returns the token the window was told about by the last drop.
struct Dropper {
    fixture: Fixture,
    main: WebviewWindow<MockRuntime>,
    tokens: Arc<Mutex<Vec<String>>>,
}

impl Dropper {
    fn new() -> Self {
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

    fn send(&self, event: &DragDropEvent, source: &FakeDrag) {
        on_drag_drop_from(self.fixture.app.handle(), event, source);
    }

    /// The token of the `n`-th drop (1-based), once the event has arrived.
    fn token(&self, n: usize) -> String {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while std::time::Instant::now() < deadline {
            if let Some(token) = self.tokens.lock().unwrap().get(n - 1) {
                return token.clone();
            }
            std::thread::sleep(std::time::Duration::from_millis(15));
        }
        panic!("no drop event {n}");
    }

    fn claim(&self, token: &str) -> Result<Value, Value> {
        call(
            &self.main,
            "attach_dropped",
            json!({"token": token, "chatId": INBOX}),
        )
    }

    fn draft_tiles(&self) -> usize {
        call(&self.main, "get_draft", json!({"chatId": INBOX})).unwrap()["attachments"]
            .as_array()
            .unwrap()
            .len()
    }
}

#[test]
fn a_picture_with_no_file_is_read_when_the_drag_enters_and_attached_like_a_paste() {
    let dropper = Dropper::new();
    let source = FakeDrag {
        image: Some(DragImage::Bytes(png_bytes())),
        ..FakeDrag::default()
    };
    dropper.send(&enter(vec![]), &source);
    assert_eq!(source.image_reads.get(), 1, "read on entering");
    dropper.send(&drop_of(vec![]), &source);
    assert_eq!(source.image_reads.get(), 1, "the drop uses what was read");

    let outcome = dropper.claim(&dropper.token(1)).unwrap();
    let attached = outcome["attached"].as_array().unwrap();
    assert_eq!(attached.len(), 1);
    assert_eq!(attached[0]["kind"], "image");
    assert!(
        attached[0]["filename"]
            .as_str()
            .unwrap()
            .starts_with("pasted-"),
        "named by Rust like a paste: {}",
        attached[0]["filename"]
    );
    assert_eq!(outcome["error"], Value::Null);
    assert_eq!(dropper.draft_tiles(), 1);
}

#[test]
fn a_drop_without_an_enter_reads_the_drag_at_the_drop() {
    let dropper = Dropper::new();
    let source = FakeDrag {
        image: Some(DragImage::Bytes(png_bytes())),
        ..FakeDrag::default()
    };
    dropper.send(&drop_of(vec![]), &source);
    assert_eq!(source.image_reads.get(), 1);
    assert_eq!(
        dropper.claim(&dropper.token(1)).unwrap()["attached"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn leaving_forgets_the_image_so_that_a_later_drop_reads_again() {
    let dropper = Dropper::new();
    let source = FakeDrag {
        image: Some(DragImage::Bytes(png_bytes())),
        ..FakeDrag::default()
    };
    dropper.send(&enter(vec![]), &source);
    dropper.send(&DragDropEvent::Leave, &source);
    dropper.send(&drop_of(vec![]), &source);
    assert_eq!(
        source.image_reads.get(),
        2,
        "once on entering, once at the drop"
    );
}

#[test]
fn files_listed_by_tauri_win_and_the_pasteboard_is_not_asked() {
    let dropper = Dropper::new();
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("photo.png");
    std::fs::write(&file, png_bytes()).unwrap();
    let source = FakeDrag {
        image: Some(DragImage::Bytes(vec![1, 2, 3])),
        ..FakeDrag::default()
    };
    dropper.send(&enter(vec![file.clone()]), &source);
    dropper.send(&drop_of(vec![file]), &source);
    assert_eq!(
        (source.image_reads.get(), source.path_reads.get()),
        (0, 0),
        "a drop from Finder never touches the image path"
    );
    let outcome = dropper.claim(&dropper.token(1)).unwrap();
    assert_eq!(outcome["attached"][0]["filename"], "photo.png");
}

#[test]
fn file_urls_that_tauri_missed_are_taken_from_the_drag() {
    let dropper = Dropper::new();
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("from-the-pasteboard.png");
    std::fs::write(&file, png_bytes()).unwrap();
    let source = FakeDrag {
        paths: vec![file],
        image: Some(DragImage::Bytes(vec![1, 2, 3])),
        ..FakeDrag::default()
    };
    dropper.send(&drop_of(vec![]), &source);
    assert_eq!(
        source.image_reads.get(),
        0,
        "a file is preferred to image data"
    );
    let outcome = dropper.claim(&dropper.token(1)).unwrap();
    assert_eq!(
        outcome["attached"][0]["filename"],
        "from-the-pasteboard.png"
    );
}

#[test]
fn dropped_bytes_get_the_checks_of_a_paste() {
    // The same bytes through the paste command and through a drop give the same kind, the same size and the same error.
    let dropper = Dropper::new();
    for (index, bytes) in [vec![], b"not an image at all".to_vec(), png_bytes()]
        .into_iter()
        .enumerate()
    {
        let source = FakeDrag {
            image: Some(DragImage::Bytes(bytes.clone())),
            ..FakeDrag::default()
        };
        dropper.send(&drop_of(vec![]), &source);
        let dropped = dropper.claim(&dropper.token(index + 1)).unwrap();
        let pasted = raw_paste(&dropper.main, bytes);
        match pasted {
            Ok(tile) => {
                let drop_tile = &dropped["attached"][0];
                assert_eq!(drop_tile["kind"], tile["kind"]);
                assert_eq!(drop_tile["byteSize"], tile["byteSize"]);
                assert_eq!(dropped["error"], Value::Null);
            }
            Err(error) => assert_eq!(dropped["error"], error["code"]),
        }
    }
}

#[test]
fn a_picture_over_the_paste_limit_is_refused_and_nothing_is_written() {
    let dropper = Dropper::new();
    let source = FakeDrag {
        image: Some(DragImage::TooLarge),
        ..FakeDrag::default()
    };
    dropper.send(&enter(vec![]), &source);
    dropper.send(&drop_of(vec![]), &source);
    let outcome = dropper.claim(&dropper.token(1)).unwrap();
    assert_eq!(outcome["error"], "attachment_too_large");
    assert_eq!(outcome["attached"], json!([]));
    assert_eq!(dropper.draft_tiles(), 0);
}

#[test]
fn an_empty_drop_says_so_and_writes_nothing() {
    let dropper = Dropper::new();
    let source = FakeDrag::default();
    dropper.send(&enter(vec![]), &source);
    dropper.send(&drop_of(vec![]), &source);
    let outcome = dropper.claim(&dropper.token(1)).unwrap();
    assert_eq!(outcome["error"], "drop_not_supported");
    assert_eq!(outcome["attached"], json!([]));
    assert_eq!(outcome["skippedFolders"], 0);
    assert_eq!(dropper.draft_tiles(), 0);
    // The token is spent like any other.
    assert_eq!(
        dropper.claim(&dropper.token(1)).unwrap_err(),
        json!({"code": "attachment_not_found"})
    );
}

#[test]
fn a_dropped_picture_obeys_the_limit_of_ten_and_the_chat_check() {
    let dropper = Dropper::new();
    for _ in 0..10 {
        raw_paste(&dropper.main, png_bytes()).unwrap();
    }
    let source = FakeDrag {
        image: Some(DragImage::Bytes(png_bytes())),
        ..FakeDrag::default()
    };
    dropper.send(&drop_of(vec![]), &source);
    let outcome = dropper.claim(&dropper.token(1)).unwrap();
    assert_eq!(outcome["error"], "attachment_limit");
    assert_eq!(dropper.draft_tiles(), 10);

    dropper.send(&drop_of(vec![]), &source);
    let nowhere = call(
        &dropper.main,
        "attach_dropped",
        json!({"token": dropper.token(2), "chatId": "00000000-0000-4000-8000-000000000000"}),
    )
    .unwrap();
    assert_eq!(nowhere["error"], "chat_not_found");
}

#[test]
fn the_event_to_the_window_has_a_token_and_never_the_bytes_or_a_path() {
    let dropper = Dropper::new();
    let dir = tempfile::tempdir().unwrap();
    let source = FakeDrag {
        image: Some(DragImage::Bytes(png_bytes())),
        ..FakeDrag::default()
    };
    let dropped_paths = vec![dir.path().join("secret-name.png")];
    dropper.send(&drop_of(dropped_paths), &source);
    let token = dropper.token(1);
    assert_eq!(token.len(), 36, "a UUID, nothing else: {token}");
    assert!(!token.contains("secret-name"));
}

fn raw_paste(window: &WebviewWindow<MockRuntime>, bytes: Vec<u8>) -> Result<Value, Value> {
    use tauri::ipc::InvokeBody;
    use tauri::test::{INVOKE_KEY, get_ipc_response};
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
