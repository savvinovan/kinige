//! Closing the main window (PRD §5, §5.1, ADR-0015 round 5, KINIG-156): the app lives in the menu bar, so the red button and ⌘W hide the
//! window. A recording the window is making must not go on behind a hidden window (the macOS microphone indicator would stay on, and the quick
//! capture window could not open the device), so the page is told first and stops and keeps it. Minimizing and ⌘H are not closing: they keep
//! recording, like switching to another app.

use crate::capture::MAIN_LABEL;
use tauri::{CloseRequestApi, Emitter, Manager, Runtime, Window};

/// Rust -> `main`: the window is being hidden by a close. The page stops a recording that is running and keeps its bytes as "Recording stopped"
/// (`voice.park()`, the same as the capture window does). The event carries nothing. The name matches `web/src/api/tauriCall.ts`.
pub const MAIN_WINDOW_HIDING: &str = "main-window-hiding";

/// What the close request of the main window does: the window is not destroyed, the page is told, and the window is hidden at once without
/// waiting for the page. The page lets go of the microphone after its recorder has stopped, so the claim is never released by Rust while the
/// device is still open.
pub fn on_close_requested<R: Runtime>(window: &Window<R>, api: &CloseRequestApi) {
    api.prevent_close();
    hide(window);
}

/// Tells the page, then hides the window. Public for the IPC tests, which cannot build a `CloseRequestApi`.
pub fn hide<R: Runtime>(window: &Window<R>) {
    let _ = window
        .app_handle()
        .emit_to(MAIN_LABEL, MAIN_WINDOW_HIDING, ());
    let _ = window.hide();
}
