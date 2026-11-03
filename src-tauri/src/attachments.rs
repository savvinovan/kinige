//! Attachment commands of the main window (slice 7, PRD §5, §12.3, ADR-0014): the paperclip dialog, a paste, a file drop,
//! removing a tile and opening a file in its system app. There are no business rules here (they are in `kinige-domain` and
//! `kinige-store`). The capture window and the settings window are granted none of these commands.
//!
//! Paths never reach the frontend. The dialog runs here; a drop is read from the window event and JS gets a single-use token
//! that expires; a paste crosses IPC as a raw body.

use crate::capture::MAIN_LABEL;
use kinige_domain::attachment::MAX_PASTE_BYTES;
use kinige_domain::{ApiError, AttachOutcome, AttachmentView, ErrorCode};
use kinige_store::{Now, Store, StoreError};
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};
use tauri::ipc::{InvokeBody, Request};
use tauri::{AppHandle, DragDropEvent, Emitter, Manager, Runtime, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;
use uuid::Uuid;

type ApiResult<T> = Result<T, ApiError>;

/// The event for the main window while files are dragged over it: `true` on entering, `false` on leaving or dropping. It shows the
/// "Drop to attach" overlay.
pub const ATTACH_DRAG: &str = "attachments-drag";
/// The event for the main window after a drop: the one-time token for `attach_dropped`.
pub const ATTACH_DROPPED: &str = "attachments-dropped";

/// How long a drop token lives.
const TOKEN_TTL: Duration = Duration::from_secs(60);
/// How many drops are remembered at once: older ones are forgotten.
const TOKEN_CAP: usize = 8;
/// How many dropped paths one drop may carry: a bigger one is cut (an entry takes ten attachments anyway).
const MAX_DROPPED_PATHS: usize = 64;
/// The longest header value we read: a chat id or a percent-encoded file name.
const MAX_HEADER_BYTES: usize = 2048;

/// Dropped files waiting for the window to claim them. Only the Rust side ever sees the paths.
#[derive(Default)]
pub struct DropTokens {
    drops: Mutex<Vec<Drop>>,
}

struct Drop {
    token: String,
    paths: Vec<PathBuf>,
    at: Instant,
}

impl DropTokens {
    fn lock(&self) -> MutexGuard<'_, Vec<Drop>> {
        self.drops.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Remembers the paths of a drop and returns its token (a UUID v4).
    pub fn put(&self, paths: Vec<PathBuf>) -> String {
        let token = Uuid::new_v4().to_string();
        let mut drops = self.lock();
        drops.retain(|drop| drop.at.elapsed() < TOKEN_TTL);
        while drops.len() >= TOKEN_CAP {
            drops.remove(0);
        }
        let mut paths = paths;
        paths.truncate(MAX_DROPPED_PATHS);
        drops.push(Drop {
            token: token.clone(),
            paths,
            at: Instant::now(),
        });
        token
    }

    /// Tests only: a token that is `age` old (expiry cannot be waited for).
    #[doc(hidden)]
    pub fn put_aged_for_test(&self, paths: Vec<PathBuf>, age: Duration) -> String {
        let token = self.put(paths);
        if let Some(drop) = self.lock().iter_mut().find(|drop| drop.token == token) {
            drop.at = Instant::now().checked_sub(age).unwrap_or(drop.at);
        }
        token
    }

    /// Tests only: how many drops are waiting.
    #[doc(hidden)]
    pub fn pending_for_test(&self) -> usize {
        self.lock().len()
    }

    /// The paths of a drop, once: the token is spent. An unknown, spent or expired token gives `None`.
    pub fn take(&self, token: &str) -> Option<Vec<PathBuf>> {
        let mut drops = self.lock();
        let index = drops.iter().position(|drop| drop.token == token)?;
        let drop = drops.remove(index);
        (drop.at.elapsed() < TOKEN_TTL).then_some(drop.paths)
    }
}

fn api(error: StoreError) -> ApiError {
    // Only the category goes to the process log: never a file name or a path.
    log::error!("store error: {}", error.category());
    error.code().into()
}

/// The paperclip dialog: the user picks files, each becomes a tile of the chat's draft. The dialog is called here, from Rust, and
/// is blocking (the command runs in a thread pool, not on the main thread). Cancelling attaches nothing.
#[tauri::command(async)]
pub fn attach_pick<R: Runtime>(
    app: AppHandle<R>,
    store: State<'_, Store>,
    chat_id: String,
) -> ApiResult<AttachOutcome> {
    let picked = app.dialog().file().blocking_pick_files();
    let paths: Vec<PathBuf> = picked
        .unwrap_or_default()
        .into_iter()
        .filter_map(|file| file.into_path().ok())
        .collect();
    Ok(attach_paths(&store, &chat_id, &paths))
}

/// A paste: the bytes of an image (or a file) from the clipboard as the raw body, the chat id and a name hint as headers
/// (`x-chat-id`; `x-name-hint`, percent-encoded, optional). The name is only a hint and is cleaned in Rust.
#[tauri::command(async)]
pub fn attach_paste(store: State<'_, Store>, request: Request<'_>) -> ApiResult<AttachmentView> {
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err(ErrorCode::AttachmentUnreadable.into());
    };
    if bytes.len() > MAX_PASTE_BYTES {
        return Err(ErrorCode::AttachmentTooLarge.into());
    }
    let chat_id = header_text(&request, "x-chat-id").ok_or(ErrorCode::ChatNotFound)?;
    let hint = header_text(&request, "x-name-hint");
    store
        .attach_bytes(&chat_id, hint.as_deref(), bytes, &Now::system())
        .map_err(api)
}

/// Claims a drop: the token comes from the `attachments-dropped` event. Folders in the drop are skipped and counted. An unknown, spent
/// or expired token is `attachment_not_found`.
#[tauri::command(async)]
pub fn attach_dropped(
    store: State<'_, Store>,
    tokens: State<'_, DropTokens>,
    token: String,
    chat_id: String,
) -> ApiResult<AttachOutcome> {
    let paths = tokens.take(&token).ok_or(ErrorCode::AttachmentNotFound)?;
    Ok(attach_paths(&store, &chat_id, &paths))
}

/// Removes a tile of the chat's draft (and its file).
#[tauri::command(async)]
pub fn remove_draft_attachment(
    store: State<'_, Store>,
    chat_id: String,
    attachment_id: String,
) -> ApiResult<()> {
    store
        .remove_draft_attachment(&chat_id, &attachment_id)
        .map_err(api)
}

/// Opens an attachment in its system app: the file is cloned and the clone is opened, so the stored file never changes. Only
/// documents on the allowlist are opened; executables, scripts and everything unknown are shown in the file manager and never run.
#[tauri::command(async)]
pub fn open_attachment<R: Runtime>(
    app: AppHandle<R>,
    store: State<'_, Store>,
    attachment_id: String,
) -> ApiResult<()> {
    let target = store.prepare_open(&attachment_id).map_err(api)?;
    let opened = if target.reveal_only {
        app.opener().reveal_item_in_dir(&target.path)
    } else {
        app.opener()
            .open_path(target.path.to_string_lossy().into_owned(), None::<&str>)
    };
    opened.map_err(|_| {
        // The opener error text may hold the path.
        log::warn!("could not open an attachment in the system app");
        ApiError::from(ErrorCode::Internal)
    })
}

/// A drag over the main window: the overlay goes on and off, a drop leaves a token. Other windows are not asked: a drop on them
/// does nothing.
pub fn on_drag_drop<R: Runtime>(app: &AppHandle<R>, event: &DragDropEvent) {
    match event {
        DragDropEvent::Enter { .. } => {
            let _ = app.emit_to(MAIN_LABEL, ATTACH_DRAG, true);
        }
        DragDropEvent::Leave => {
            let _ = app.emit_to(MAIN_LABEL, ATTACH_DRAG, false);
        }
        DragDropEvent::Drop { paths, .. } => {
            let _ = app.emit_to(MAIN_LABEL, ATTACH_DRAG, false);
            if paths.is_empty() {
                return;
            }
            if let Some(tokens) = app.try_state::<DropTokens>() {
                let token = tokens.put(paths.clone());
                let _ = app.emit_to(MAIN_LABEL, ATTACH_DROPPED, token);
            }
        }
        _ => {}
    }
}

/// Attaches files one after another. A folder is skipped and counted; the first error stops the rest and is reported with the tiles
/// that made it.
fn attach_paths(store: &Store, chat_id: &str, paths: &[PathBuf]) -> AttachOutcome {
    let mut outcome = AttachOutcome {
        attached: Vec::new(),
        skipped_folders: 0,
        error: None,
    };
    for path in paths {
        // `symlink_metadata`: a link to a folder is not followed here, the store refuses a link on its own.
        if std::fs::symlink_metadata(path).is_ok_and(|meta| meta.is_dir()) {
            outcome.skipped_folders = outcome.skipped_folders.saturating_add(1);
            continue;
        }
        match store.attach_path(chat_id, path, &Now::system()) {
            Ok(view) => outcome.attached.push(view),
            Err(error) => {
                outcome.error = Some(api(error).code);
                break;
            }
        }
    }
    // The files of the drop were attached; the folders were not, and the person is told ("Folders are not attached").
    if outcome.error.is_none() && outcome.skipped_folders > 0 {
        outcome.error = Some(ErrorCode::FoldersNotAttached);
    }
    outcome
}

/// A header as text: ASCII, not empty, bounded, percent-decoded (`encodeURIComponent` on the JS side).
fn header_text(request: &Request<'_>, name: &str) -> Option<String> {
    let value = request.headers().get(name)?;
    if value.len() > MAX_HEADER_BYTES {
        return None;
    }
    let raw = value.to_str().ok()?;
    let decoded: String = url::form_urlencoded::parse(format!("v={raw}").as_bytes())
        .next()
        .map(|(_, value)| value.into_owned())?;
    (!decoded.is_empty()).then_some(decoded)
}
