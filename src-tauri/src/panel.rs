//! The capture window as a non-activating NSPanel (macOS, ADR-0006).
//!
//! A regular Tauri window on macOS 14+ gets input unreliably while another app is active: the system "cooperatively" refuses
//! activation, and the window gets focus and loses it, while key presses go not to the page. A panel with the
//! `NonactivatingPanel` flag becomes the key window without activating the app (this is how Spotlight, Raycast, Alfred work): the previous
//! app stays active and gets focus back by itself after the panel hides, the main window does not pop up.
//! The library is `tauri-nspanel` 2.1.1 (it requires the `macOSPrivateApi` flag).

#![allow(clippy::unused_unit)] // the `panel_event!` macro writes `-> ()`

use tauri::{AppHandle, Manager, Runtime};
use tauri_nspanel::{
    CollectionBehavior, ManagerExt, PanelLevel, StyleMask, WebviewWindowExt, tauri_panel,
};

tauri_panel! {
    panel!(CapturePanel {
        config: {
            can_become_key_window: true,
            can_become_main_window: false,
            is_floating_panel: true
        }
    })

    panel_event!(CapturePanelEvents {
        window_did_become_key(notification: &NSNotification) -> (),
        window_did_resign_key(notification: &NSNotification) -> (),
        window_should_close(window: &NSWindow) -> Bool
    })
}

/// Turns the hidden capture window created at startup into a panel: without activation, above the others, on all desktops and
/// over full-screen apps. Losing key status means hiding (`on_resign`), closing the window (Cmd+W) is the same,
/// but the panel is not destroyed.
pub fn convert<R: Runtime>(
    app: &AppHandle<R>,
    label: &str,
    on_become: impl Fn(&AppHandle<R>) + 'static,
    on_resign: impl Fn(&AppHandle<R>) + 'static,
    on_close: impl Fn(&AppHandle<R>) + 'static,
) -> tauri::Result<()> {
    let window = app
        .get_webview_window(label)
        .ok_or(tauri::Error::WindowNotFound)?;
    let panel = window.to_panel::<CapturePanel<R>>()?;
    panel
        .add_style_mask(StyleMask::empty().nonactivating_panel().value())
        .map_err(|error| tauri::Error::Anyhow(error.into()))?;
    panel.set_level(PanelLevel::Floating.value());
    panel.set_hides_on_deactivate(false);
    panel.set_collection_behavior(
        CollectionBehavior::new()
            .can_join_all_spaces()
            .full_screen_auxiliary()
            .value(),
    );

    let handler = CapturePanelEvents::new();
    let resign_app = app.clone();
    let become_app = app.clone();
    handler.window_did_become_key(move |_| on_become(&become_app));
    handler.window_did_resign_key(move |_| on_resign(&resign_app));
    let close_app = app.clone();
    handler.window_should_close(move |_| {
        // Closing the window does not destroy the panel: the window just hides (through the page, to save the draft).
        on_close(&close_app);
        tauri_nspanel::objc2::runtime::Bool::NO
    });
    panel.set_event_handler(Some(handler.as_ref()));
    Ok(())
}

/// The window's panel, if the plugin is attached and the window is already turned into a panel (tests with a mock runtime have no plugin).
fn panel_of<R: Runtime>(app: &AppHandle<R>, label: &str) -> Option<tauri_nspanel::PanelHandle<R>> {
    app.try_state::<tauri_nspanel::WebviewPanelManager<R>>()?;
    app.get_webview_panel(label).ok()
}

/// Shows the panel and makes it key without activating the app.
pub fn show<R: Runtime>(app: &AppHandle<R>, label: &str) -> bool {
    panel_of(app, label).is_some_and(|panel| {
        panel.show_and_make_key();
        true
    })
}

/// Hides the panel.
pub fn hide<R: Runtime>(app: &AppHandle<R>, label: &str) -> bool {
    panel_of(app, label).is_some_and(|panel| {
        panel.hide();
        true
    })
}
