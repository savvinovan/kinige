//! The quick capture window (PRD §5.1): showing and hiding, window commands, the clipboard hotkey. The rules for appending and storage are in
//! `kinige-domain` and `kinige-store`; here only the window and the platform.

use kinige_domain::{ApiError, CaptureOutcome, CaptureState, ErrorCode};
use kinige_store::{Now, Store, StoreError};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, Runtime, State};

pub const CAPTURE_LABEL: &str = "capture";
pub const MAIN_LABEL: &str = "main";
/// An event for the main window: entries changed (capture, appending, clipboard). Only `main` listens.
pub const ENTRIES_CHANGED: &str = "entries-changed";

/// The bottom edge of the capture panel stands above the bottom of the monitor work area (without the Dock and menu bar) by this fraction of its height. The panel
/// is at the bottom of the screen, like a chat field: it does not cover what the person wants to write about, and the input field is always in the same place. The bottom
/// edge is pinned at any window height: context and multi-line text grow upward.
const BOTTOM_FRACTION: f64 = 0.12;
/// Capture window sizes in logical px. They match the `layout.capture.*` tokens (checked by `tests/config.rs`): the window
/// width = the panel width + two shadow margins; the height is fitted by the page with the `capture_resize` command within these limits.
pub const WINDOW_WIDTH: f64 = 584.0;
pub const SHADOW_MARGIN: f64 = 32.0;
pub const MIN_HEIGHT: f64 = 100.0;
pub const MAX_HEIGHT: f64 = 640.0;
/// How long to wait for the capture window to save the draft and hide itself before hiding it by force.
const HIDE_GRACE: Duration = Duration::from_millis(600);

type ApiResult<T> = Result<T, ApiError>;

fn api(error: StoreError) -> ApiError {
    // Only the category goes to the process log: the text of an internal error may contain pieces of data.
    log::error!("store error: {}", error.category());
    error.code().into()
}

/// The capture window state.
#[derive(Default)]
pub struct CaptureShell {
    /// The clipboard text and the error on a write failure: the window opens with them once.
    pending: Mutex<Option<Pending>>,
    /// Grows on every show: a deferred forced hide must not hide a window that was shown again.
    shown: AtomicU64,
    /// The window anchor on the screen at the last show: `(x of the left edge, y of the window's bottom edge)` in physical px. The bottom edge
    /// stays in place at any height: both on show and on `capture_resize` (the size on the main thread is set
    /// with a delay, so the position is computed from the anchor and not from the size read from the window).
    anchor: Mutex<Option<(i32, i32)>>,
}

struct Pending {
    text: String,
    error: ErrorCode,
}

impl CaptureShell {
    fn pending(&self) -> std::sync::MutexGuard<'_, Option<Pending>> {
        self.pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Shows the main window (the tray menu, the Dock icon, a second launch).
///
/// An app started by the MCP bridge lives without a Dock icon (the Accessory policy): when a window must be shown, the policy
/// becomes regular (on macOS), and the icon appears. Hiding the window does not touch the policy: the icon stays, as it was
/// for an app started normally (slices 2 and 3).
pub fn show_main<R: Runtime>(app: &AppHandle<R>) {
    #[cfg(target_os = "macos")]
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
    if let Some(window) = app.get_webview_window(MAIN_LABEL) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Tells the main window that entries changed, and refreshes "recent chats" in the icon menu.
pub fn notify_entries_changed<R: Runtime>(app: &AppHandle<R>) {
    let _ = app.emit_to(MAIN_LABEL, ENTRIES_CHANGED, ());
    crate::tray::refresh(app);
}

/// An event for the main window: open a chat (from the icon menu). Only `main` listens; the payload is the chat id.
pub const OPEN_CHAT: &str = "open-chat";

/// Shows the main window right away with this chat (the icon menu, slice 3).
pub fn open_chat<R: Runtime>(app: &AppHandle<R>, chat_id: &str) {
    show_main(app);
    let _ = app.emit_to(MAIN_LABEL, OPEN_CHAT, chat_id.to_string());
}

/// The window height from the page's request: within `MIN_HEIGHT ... MAX_HEIGHT`, a whole number of px.
pub fn clamp_height(height: f64) -> f64 {
    if height.is_finite() {
        height.round().clamp(MIN_HEIGHT, MAX_HEIGHT)
    } else {
        MIN_HEIGHT
    }
}

/// The window anchor for a monitor work area `(x, y, width, height)` (physical px): `(x of the left edge, y of the window's bottom edge)`.
/// Centered horizontally (`window_width` is the window width); the panel's bottom edge is above the bottom of the work area by `BOTTOM_FRACTION`
/// of its height, and the window is below the panel by the shadow margin `shadow_margin`.
pub fn anchor_for(work: (i32, i32, u32, u32), window_width: u32, shadow_margin: f64) -> (i32, i32) {
    let (work_x, work_y, work_width, work_height) = work;
    let x = f64::from(work_x) + (f64::from(work_width) - f64::from(window_width)) / 2.0;
    let panel_bottom = f64::from(work_y) + f64::from(work_height) * (1.0 - BOTTOM_FRACTION);
    (
        x.round() as i32,
        (panel_bottom + shadow_margin).round() as i32,
    )
}

/// The window top (physical px) at height `height` if the window's bottom edge is at `bottom`: the bottom edge is pinned.
pub fn top_for_bottom(bottom: i32, height: u32) -> i32 {
    bottom.saturating_sub(i32::try_from(height).unwrap_or(i32::MAX))
}

/// Places the window on the screen with the cursor at the bottom of the work area (without the Dock and menu bar), centered, and remembers the anchor. The window height
/// affects only the top of the window: the panel's bottom edge is always in the same place.
fn place_on_cursor_screen<R: Runtime>(app: &AppHandle<R>, window: &tauri::WebviewWindow<R>) {
    let monitor = app
        .cursor_position()
        .ok()
        .and_then(|cursor| app.monitor_from_point(cursor.x, cursor.y).ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten());
    let (Some(monitor), Ok(size)) = (monitor, window.outer_size()) else {
        let _ = window.center();
        return;
    };
    let work = monitor.work_area();
    let scale = window.scale_factor().unwrap_or(1.0);
    let (x, bottom) = anchor_for(
        (
            work.position.x,
            work.position.y,
            work.size.width,
            work.size.height,
        ),
        size.width,
        SHADOW_MARGIN * scale,
    );
    if let Some(shell) = app.try_state::<CaptureShell>() {
        *shell
            .anchor
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some((x, bottom));
    }
    trace(&format!(
        "place x={x} bottom={bottom} height={}",
        size.height
    ));
    let _ = window.set_position(PhysicalPosition::new(
        x,
        top_for_bottom(bottom, size.height),
    ));
}

/// A log of the capture window's life for checking in a live window: written to stderr only with `KINIGE_TRACE=1` (without entry texts).
pub fn trace(what: &str) {
    if log::log_enabled!(target: crate::logging::TRACE_TARGET, log::Level::Debug) {
        let ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() % 100_000);
        log::debug!(target: crate::logging::TRACE_TARGET, "{ms:05} {what}");
    }
}

/// Shows the capture window above all windows on the screen with the cursor. The window is created hidden at startup: showing is instant.
/// On macOS it is a panel without activating the app (`panel`): the previous app stays active, the window gets input.
///
/// Any AppKit call (panel, window) must be on the main thread only: the function is safe from any thread, the show is performed on the main one.
pub fn show_capture<R: Runtime>(app: &AppHandle<R>) {
    let main_app = app.clone();
    let _ = app.run_on_main_thread(move || show_capture_on_main(&main_app));
}

fn show_capture_on_main<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = app.get_webview_window(CAPTURE_LABEL) else {
        return;
    };
    if let Some(shell) = app.try_state::<CaptureShell>() {
        shell.shown.fetch_add(1, Ordering::SeqCst);
    }
    trace("show");
    place_on_cursor_screen(app, &window);
    #[cfg(target_os = "macos")]
    let as_panel = crate::panel::show(app, CAPTURE_LABEL);
    #[cfg(not(target_os = "macos"))]
    let as_panel = false;
    if !as_panel {
        let _ = window.show();
        let _ = window.set_focus();
        // The window is created hidden and unfocused: without this call the keyboard stays with the window, not with the page.
        let webview: &tauri::Webview<R> = window.as_ref();
        let _ = webview.set_focus();
    }
    // The page learns about the show without IPC (the window has no right to listen to events): it rereads the state and puts the cursor in the field.
    let _ = window.eval("window.dispatchEvent(new Event('kinige-capture-shown'))");
}

/// Hides the capture window. The app was not activated (a panel), so the previous app stays active and focus
/// returns to it by itself. Safe from any thread (the hide is performed on the main one).
pub fn hide_capture<R: Runtime>(app: &AppHandle<R>) {
    let main_app = app.clone();
    let _ = app.run_on_main_thread(move || hide_capture_on_main(&main_app));
}

fn hide_capture_on_main<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = app.get_webview_window(CAPTURE_LABEL) else {
        return;
    };
    if !window.is_visible().unwrap_or(false) {
        return;
    }
    trace("hide");
    #[cfg(target_os = "macos")]
    let as_panel = crate::panel::hide(app, CAPTURE_LABEL);
    #[cfg(not(target_os = "macos"))]
    let as_panel = false;
    if !as_panel {
        let _ = window.hide();
    }
}

/// Asks the window page to save the draft and hide (`capture_hide`); if the page did not respond, hides by force.
pub fn request_hide<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = app.get_webview_window(CAPTURE_LABEL) else {
        return;
    };
    // A hidden window does not need hiding (including when the loss of key status is a consequence of the hide itself).
    if !window.is_visible().unwrap_or(false) {
        return;
    }
    trace("request-hide");
    let _ = window.eval("window.dispatchEvent(new Event('kinige-capture-hide-request'))");
    let delayed = app.clone();
    let shown = app
        .try_state::<CaptureShell>()
        .map(|shell| shell.shown.load(Ordering::SeqCst));
    let spawned = std::thread::Builder::new()
        .name("kinige-capture-hide".into())
        .spawn(move || {
            std::thread::sleep(HIDE_GRACE);
            let same_showing = delayed
                .try_state::<CaptureShell>()
                .map(|shell| shell.shown.load(Ordering::SeqCst))
                == shown;
            if same_showing {
                hide_capture(&delayed);
            }
        });
    if spawned.is_err() {
        // There is nowhere to wait out the "grace time": hide at once (the page has already got the request to save the draft).
        log::warn!("the thread for the deferred hide of the capture window did not start");
        hide_capture(app);
    }
}

/// The capture window became key (only a mark in the `KINIGE_TRACE` log).
pub fn on_capture_key<R: Runtime>(_app: &AppHandle<R>) {
    trace("key");
}

/// The capture window lost key status (a click into another window, another app): it hides, saving the draft
/// (§5.1). If the window is already hidden, `request_hide` does nothing.
pub fn on_capture_blur<R: Runtime>(app: &AppHandle<R>) {
    trace("resign");
    request_hide(app);
}

/// The capture hotkey: the window is closed -- show it, open -- hide it, saving the draft.
pub fn toggle_capture<R: Runtime>(app: &AppHandle<R>) {
    let visible = app
        .get_webview_window(CAPTURE_LABEL)
        .is_some_and(|window| window.is_visible().unwrap_or(false));
    if visible {
        request_hide(app);
    } else {
        show_capture(app);
    }
}

/// The clipboard hotkey (PRD §5.1): text from the clipboard becomes a new entry in Inbox, without a window. Not text or empty -- nothing. A write failure
/// -- the capture window with the clipboard text and an error. Success -- the menu bar icon blinks. Runs on a separate thread: reading
/// the clipboard and writing to the database must not delay the event loop.
pub fn save_clipboard<R: Runtime>(app: &AppHandle<R>) {
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("kinige-clipboard".into())
        .spawn(move || {
            // Not text (an image, files -- slice 7), an empty clipboard or a clipboard held by another app: we write nothing.
            if let Ok(text) =
                arboard::Clipboard::new().and_then(|mut clipboard| clipboard.get_text())
            {
                save_clipboard_text(&app, text);
            }
        });
    if spawned.is_err() {
        log::error!("the clipboard hotkey thread did not start");
    }
}

/// Writing the clipboard text to Inbox. If there is no store (the journal is not open, recovery mode), nothing is written.
fn save_clipboard_text<R: Runtime>(app: &AppHandle<R>, text: String) {
    let Some(store) = app.try_state::<Store>() else {
        log::warn!("clipboard hotkey: the journal is not open");
        return;
    };
    match store.save_clipboard(&text, &Now::system()) {
        Ok(_) => {
            notify_entries_changed(app);
            crate::tray::flash(app);
        }
        Err(StoreError::Empty) => {}
        Err(error) => {
            log::error!("saving the clipboard failed: {}", error.category());
            open_with_failed_clipboard(app, text, error.code());
        }
    }
}

/// A clipboard write failure: the capture window opens with the clipboard text in the field and an error (PRD §1.2 item 9). The previous non-empty draft
/// of the window is not overwritten: the clipboard text goes under it (`joined_draft`), both into the draft and into memory -- the draft must not be lost
/// if the window is closed without fixing the error.
fn open_with_failed_clipboard<R: Runtime>(app: &AppHandle<R>, text: String, error: ErrorCode) {
    if let Some(shell) = app.try_state::<CaptureShell>() {
        let joined = match app.try_state::<Store>() {
            Some(store) => match store.merge_clipboard_into_capture_draft(&text, &Now::system()) {
                Ok((joined, _saved)) => joined,
                // The database does not write (perhaps that is why the failure happened): the field is assembled from what can be read.
                Err(_) => kinige_domain::draft::joined_draft(
                    &store.get_capture_draft().unwrap_or_default(),
                    &text,
                ),
            },
            // There is no store: no draft, the clipboard text is not lost -- it goes into the field as is.
            None => text,
        };
        *shell.pending() = Some(Pending {
            text: joined,
            error,
        });
    }
    show_capture(app);
}

/// The window state on show: the language, the text size, today's context, the appending target, the draft and, if the window is opened because of a
/// clipboard failure, the clipboard text with the error (once).
#[tauri::command(async)]
pub fn capture_state(
    store: State<'_, Store>,
    shell: State<'_, CaptureShell>,
) -> ApiResult<CaptureState> {
    let view = store.capture_view(&Now::system()).map_err(api)?;
    let settings = store.get_settings().map_err(api)?;
    let pending = shell.pending().take();
    let (draft, error) = match pending {
        Some(Pending { text, error }) => (text, Some(error)),
        None => (view.draft, None),
    };
    Ok(CaptureState {
        language: crate::commands::get_language(),
        text_size: settings.text_size,
        today: view.today,
        context: view.context,
        append_target: view.append_target,
        draft,
        error,
        recent_emoji: settings.recent_emoji,
    })
}

/// The emoji chosen in the picker is remembered in the shared "recent" list (the same as the main window's); the reply is the new list.
/// The command takes neither a chat nor an entry: it changes only this list (slice 3, acceptance decision of 27.10.2026).
#[tauri::command(async)]
pub fn capture_add_recent_emoji(store: State<'_, Store>, emoji: String) -> ApiResult<Vec<String>> {
    store.add_recent_emoji(&emoji).map_err(api)
}

/// Enter: a new entry in Inbox; the main window gets an event.
#[tauri::command(async)]
pub fn capture_send<R: Runtime>(
    app: AppHandle<R>,
    store: State<'_, Store>,
    body: String,
) -> ApiResult<CaptureOutcome> {
    let outcome = store.capture_send(&body, &Now::system()).map_err(api)?;
    notify_entries_changed(&app);
    Ok(outcome)
}

/// Cmd+Enter: append to the last entry or, if there is no target, a new entry; the reply says what was done.
#[tauri::command(async)]
pub fn capture_append<R: Runtime>(
    app: AppHandle<R>,
    store: State<'_, Store>,
    body: String,
) -> ApiResult<CaptureOutcome> {
    let outcome = store.capture_append(&body, &Now::system()).map_err(api)?;
    notify_entries_changed(&app);
    Ok(outcome)
}

#[tauri::command(async)]
pub fn capture_save_draft(store: State<'_, Store>, body: String) -> ApiResult<()> {
    store.set_capture_draft(&body, &Now::system()).map_err(api)
}

/// Hide the capture window (after sending, Esc, losing focus).
#[tauri::command]
pub fn capture_hide<R: Runtime>(app: AppHandle<R>) {
    hide_capture(&app);
}

/// Fit the window height to the page content (logical px, together with the shadow margin). The width is fixed, and the **bottom
/// edge of the window stays in place** (the window grows upward, like a chat: the field at the bottom, entries above it), so the input field is always in the same
/// place. The value is clamped to `MIN_HEIGHT ... MAX_HEIGHT`; redundant requests (the same height) are not executed by Rust.
#[tauri::command]
pub fn capture_resize<R: Runtime>(app: AppHandle<R>, height: f64) {
    let Some(window) = app.get_webview_window(CAPTURE_LABEL) else {
        return;
    };
    let height = clamp_height(height);
    let scale = window.scale_factor().unwrap_or(1.0);
    let current = window
        .inner_size()
        .map(|size| f64::from(size.height) / scale);
    if current.is_ok_and(|current| (current - height).abs() < 0.5) {
        return;
    }
    let _ = window.set_size(tauri::LogicalSize::new(WINDOW_WIDTH, height));
    // `set_size` on macOS sets the content size and does not promise which corner stays in place: the position is recomputed
    // explicitly from the show anchor (the bottom edge of the window and x). If there is no anchor (the window was not shown yet), the show will set the position.
    let anchor = app.try_state::<CaptureShell>().and_then(|shell| {
        *shell
            .anchor
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    });
    if let Some((x, bottom)) = anchor {
        let new_height = (height * scale).round() as u32;
        trace(&format!("resize height={height} bottom={bottom}"));
        let _ = window.set_position(PhysicalPosition::new(x, top_for_bottom(bottom, new_height)));
    }
}

#[cfg(test)]
#[path = "capture_tests.rs"]
mod tests;
