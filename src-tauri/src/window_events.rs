//! What the windows' own events do (close, blur, drop, destroy). Moved out of `lib.rs` (KINIG-138) so that file stays under the size cap.

use crate::capture::{self, CAPTURE_LABEL, MAIN_LABEL};
use crate::{attachments, settings_window};
use tauri::{Manager, Runtime, WindowEvent};

/// Closing the main window hides it, the app lives in the menu bar; closing the capture window is the same as Esc.
/// Public so that the IPC tests can send a window a drop (only `main` leaves a token).
pub fn on_window_event<R: Runtime>(window: &tauri::Window<R>, event: &WindowEvent) {
    #[cfg(feature = "screens-driver")]
    crate::screens_driver::on_window_event(window, event);
    match (window.label(), event) {
        (MAIN_LABEL, WindowEvent::CloseRequested { api, .. }) => {
            api.prevent_close();
            let _ = window.hide();
        }
        // Files dragged over the main window: the overlay and, on a drop, a token (slice 7). Other windows ignore drops.
        (MAIN_LABEL, WindowEvent::DragDrop(event)) => {
            attachments::on_drag_drop(window.app_handle(), event)
        }
        (CAPTURE_LABEL, WindowEvent::CloseRequested { api, .. }) => {
            api.prevent_close();
            capture::request_hide(window.app_handle());
        }
        // A click into another window or another app: the capture window hides, the draft is saved.
        (CAPTURE_LABEL, WindowEvent::Focused(false)) => {
            capture::on_capture_blur(window.app_handle())
        }
        // The settings window closes completely; a shortcut recording, if in progress, is cancelled.
        (settings_window::SETTINGS_LABEL, WindowEvent::Destroyed) => {
            settings_window::on_destroyed(window.app_handle())
        }
        _ => {}
    }
}
