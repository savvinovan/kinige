//! Attachment commands of the main window (slice 7, PRD §5, §12.3, ADR-0014): the paperclip dialog, a paste, a file drop,
//! removing a tile and opening a file in its system app. There are no business rules here (they are in `kinige-domain` and
//! `kinige-store`). The capture window and the settings window are granted none of these commands.
//!
//! Paths never reach the frontend. The dialog runs here; a drop is read from the window event (`drops.rs`) and JS gets a single-use
//! token that expires; a paste crosses IPC as a raw body.

pub use crate::drops::{
    ATTACH_DRAG, ATTACH_DROPPED, DropTokens, Dropped, on_drag_drop, on_drag_drop_from,
};

use crate::drag_source::DragImage;
use crate::drag_tiff::tiff_to_png;
use kinige_domain::attachment::MAX_PASTE_BYTES;
use kinige_domain::{ApiError, AttachOutcome, AttachmentView, ErrorCode};
use kinige_store::{Now, Store, StoreError};
use std::path::PathBuf;
use tauri::ipc::{InvokeBody, Request};
use tauri::{AppHandle, Runtime, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

type ApiResult<T> = Result<T, ApiError>;

/// The longest header value we read: a chat id or a percent-encoded file name.
const MAX_HEADER_BYTES: usize = 2048;

pub(crate) fn api(error: StoreError) -> ApiError {
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

/// Claims a drop: the token comes from the `attachments-dropped` event. Folders in the drop are skipped and counted. A drop with no
/// file but an image (a picture dragged from Telegram or a browser) is attached like a pasted image; a drop with neither says
/// `drop_not_supported` in the outcome. An unknown, spent or expired token is `attachment_not_found`. Runs off the UI thread, which
/// is where a dragged TIFF is decoded.
#[tauri::command(async)]
pub fn attach_dropped(
    store: State<'_, Store>,
    tokens: State<'_, DropTokens>,
    token: String,
    chat_id: String,
) -> ApiResult<AttachOutcome> {
    let dropped = tokens.take(&token).ok_or(ErrorCode::AttachmentNotFound)?;
    Ok(attach_dropped_content(&store, &chat_id, dropped))
}

/// The web view says whether a chat is open to receive an attachment. Only then is the image of a drag read when it enters the window:
/// with the trash open, or no chat, nothing is read and nothing is held. Takes a flag and the number of the call, which only grows:
/// a call older than the newest one seen is ignored, so the order of arrival cannot matter.
#[tauri::command]
pub fn set_drop_target(tokens: State<'_, DropTokens>, active: bool, seq: u64) {
    tokens.set_accepting(active, seq);
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

/// What a drop gives: its files; else its image, the way a pasted image goes (same size limit, same checks, a name made by Rust);
/// else `drop_not_supported`. A dragged TIFF becomes a PNG inside the decoder limits, or, if it cannot, is attached as it is (the
/// store keeps a TIFF as a file card). Nothing is written unless something is attached.
fn attach_dropped_content(store: &Store, chat_id: &str, dropped: Dropped) -> AttachOutcome {
    if !dropped.paths.is_empty() {
        return attach_paths(store, chat_id, &dropped.paths);
    }
    let mut outcome = AttachOutcome {
        attached: Vec::new(),
        skipped_folders: 0,
        error: None,
    };
    let bytes = match dropped.image {
        Some(DragImage::Bytes(bytes)) => bytes,
        Some(DragImage::Tiff(tiff)) => tiff_to_png(&tiff).unwrap_or(tiff),
        Some(DragImage::TooLarge) => {
            outcome.error = Some(ErrorCode::AttachmentTooLarge);
            return outcome;
        }
        None => {
            outcome.error = Some(ErrorCode::DropNotSupported);
            return outcome;
        }
    };
    match store.attach_bytes(chat_id, None, &bytes, &Now::system()) {
        Ok(view) => outcome.attached.push(view),
        Err(error) => outcome.error = Some(api(error).code),
    }
    outcome
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
pub(crate) fn header_text(request: &Request<'_>, name: &str) -> Option<String> {
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
