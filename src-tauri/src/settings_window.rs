//! The "Settings" window (slice 4b, ADR-0010): a separate Tauri window, the app menu (Cmd+,) and the icon menu, events to windows, a bridge to
//! global shortcuts for the settings file. The file rules live in `kinige-store`; here only the window and the platform.

use crate::capture::{MAIN_LABEL, show_main};
use crate::hotkeys::Hotkeys;
use kinige_domain::{ApiError, ErrorCode, SettingsFileState, ShortcutKind};
use kinige_store::{SettingsFile, SettingsFileOptions, ShortcutApplier, ShortcutRefusal, Store};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};
use tauri::{AppHandle, Emitter, Manager, Runtime, State, WebviewUrl, WebviewWindowBuilder};

pub const SETTINGS_LABEL: &str = "settings";
/// An event for the `main` and `settings` windows: the settings changed (by a window, another window or the file). No data: the windows read on their own.
pub const SETTINGS_CHANGED: &str = "settings-changed";
/// An event for the main window: open an entry in the feed (from the access log in the settings window).
pub const OPEN_MESSAGE: &str = "open-message";

/// Window sizes: default and smallest (logical px). They match the `layout.settings.window-*` tokens (checked by
/// `tests/config.rs`): the page is designed for exactly such windows.
pub const WIDTH: f64 = 860.0;
pub const HEIGHT: f64 = 600.0;
pub const MIN_WIDTH: f64 = 640.0;
pub const MIN_HEIGHT: f64 = 440.0;
/// The identifier of the "Settings..." item in the app menu.
pub const ID_APP_SETTINGS: &str = "app-settings";
/// Keeps two fast presses from creating two windows.
static OPEN_LOCK: Mutex<()> = Mutex::new(());

/// The "Settings..." item of the app menu: enabled when the journal is open.
pub struct AppSettingsItem<R: Runtime>(pub tauri::menu::MenuItem<R>);

/// The app menu on macOS: the standard one (it includes "Edit" -- without it Cmd+C and Cmd+V do not work in WKWebView; the default menu
/// is built by `tauri::menu::Menu::default`, checked against the tauri 2.12.4 sources) and in the app menu the "Settings..." item with Cmd+,.
/// While the journal is not open, the item is unavailable (`set_available`).
#[cfg(target_os = "macos")]
pub fn app_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<tauri::menu::Menu<R>> {
    use tauri::menu::{Menu, MenuItem, MenuItemKind, PredefinedMenuItem};
    let menu = Menu::default(app)?;
    let labels = crate::tray::labels(crate::commands::get_language());
    let item = MenuItem::with_id(
        app,
        ID_APP_SETTINGS,
        &labels.settings,
        false,
        Some("CmdOrCtrl+,"),
    )?;
    // The first submenu is the app menu: "About", a separator, ... The item is placed after the separator.
    if let Some(MenuItemKind::Submenu(first)) = menu.items()?.into_iter().next() {
        first.insert(&item, 2)?;
        first.insert(&PredefinedMenuItem::separator(app)?, 3)?;
    }
    app.manage(AppSettingsItem(item));
    Ok(menu)
}

/// Shows the settings window (creates it on first open). While the journal is not open, it does nothing. A second call raises
/// the same window. From the background it sets the regular activation policy, like `show_main`.
pub fn open<R: Runtime>(app: &AppHandle<R>) {
    if app.try_state::<Store>().is_none() {
        return;
    }
    let _guard = OPEN_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    #[cfg(target_os = "macos")]
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
    if let Some(window) = app.get_webview_window(SETTINGS_LABEL) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }
    let title = crate::tray::window_title(crate::commands::get_language());
    let built = WebviewWindowBuilder::new(
        app,
        SETTINGS_LABEL,
        WebviewUrl::App("index.html#settings".into()),
    )
    .title(title)
    .inner_size(WIDTH, HEIGHT)
    .min_inner_size(MIN_WIDTH, MIN_HEIGHT)
    .resizable(true)
    .build();
    match built {
        Ok(window) => {
            let _ = window.set_focus();
        }
        Err(error) => log::error!("settings window was not created: {error}"),
    }
}

/// The settings window is closed (destroyed): the shortcut recording mode is cleared, otherwise the hotkeys would stay disabled.
pub fn on_destroyed<R: Runtime>(app: &AppHandle<R>) {
    if let Some(hotkeys) = app.try_state::<Hotkeys>() {
        hotkeys.set_recording(false);
    }
}

/// Tells both windows that the settings changed. Does not block: it is also called from the file thread.
pub fn notify_changed<R: Runtime>(app: &AppHandle<R>) {
    let _ = app.emit_to(MAIN_LABEL, SETTINGS_CHANGED, ());
    let _ = app.emit_to(SETTINGS_LABEL, SETTINGS_CHANGED, ());
}

/// "Settings..." is available when the journal is open (not in recovery mode and not with "database is newer").
pub fn set_available<R: Runtime>(app: &AppHandle<R>, available: bool) {
    if let Some(item) = app.try_state::<AppSettingsItem<R>>() {
        let _ = item.0.set_enabled(available);
    }
}

/// Hotkeys for the settings file: the same steps as when changing a shortcut in the window (check, register, save).
struct HotkeyBridge<R: Runtime> {
    app: AppHandle<R>,
}

impl<R: Runtime> ShortcutApplier for HotkeyBridge<R> {
    fn apply(
        &self,
        store: &Store,
        kind: ShortcutKind,
        accelerator: &str,
    ) -> Result<(), ShortcutRefusal> {
        let Some(hotkeys) = self.app.try_state::<Hotkeys>() else {
            return Err(ShortcutRefusal::Invalid);
        };
        hotkeys
            .change(store, kind, accelerator)
            .map_err(|code| match code {
                ErrorCode::ShortcutTaken => ShortcutRefusal::Taken,
                ErrorCode::ShortcutDuplicate => ShortcutRefusal::Duplicate,
                _ => ShortcutRefusal::Invalid,
            })
    }
}

/// Starts the settings file thread. The file is applied synchronously, before registering shortcuts and before the MCP listener.
pub fn start_file<R: Runtime>(
    app: &AppHandle<R>,
    store: &Store,
    data_dir: PathBuf,
) -> SettingsFile {
    let handle = app.clone();
    let mut options = SettingsFileOptions::new(Arc::new(move || notify_changed(&handle)));
    options.shortcuts = Some(Arc::new(HotkeyBridge { app: app.clone() }));
    SettingsFile::start(store.clone(), data_dir, options)
}

/// Opens the settings window from the main window (profile -> "Settings"). On Windows the command is `async`: a window
/// created on the main thread from a handler stays white (a deadlock in WebView2 creation, see the `WebviewWindowBuilder`
/// documentation). Elsewhere nothing changes.
#[cfg_attr(windows, tauri::command(async))]
#[cfg_attr(not(windows), tauri::command)]
pub fn open_settings<R: Runtime>(app: AppHandle<R>) {
    open(&app);
}

/// The same from menu and tray event handlers: they run on the main thread, so on Windows the window is created on another thread.
pub fn open_from_event<R: Runtime>(app: &AppHandle<R>) {
    #[cfg(windows)]
    {
        let app = app.clone();
        tauri::async_runtime::spawn_blocking(move || open(&app));
    }
    #[cfg(not(windows))]
    open(app);
}

/// The file path, its state and key errors. The command is not `async`, that is, it runs on the main thread: it only reads
/// the state under a short lock and never waits for the file thread. The file thread, on the contrary, waits for the main thread (registering
/// shortcuts in the plugin goes through `run_on_main_thread`), so no main thread command may wait for it --
/// otherwise a deadlock would result (`open_settings`, `shortcut_recording` and this command do not wait).
#[tauri::command]
pub fn get_settings_file(file: State<'_, SettingsFile>) -> SettingsFileState {
    file.state()
}

/// Open an entry from the access log in the main window. The entry is checked here: deleted forever or in the trash gives
/// `message_not_found`, and the window shows "Entry deleted". The entry content is not given to the settings window.
#[tauri::command(async)]
pub fn settings_open_entry<R: Runtime>(
    app: AppHandle<R>,
    store: State<'_, Store>,
    id: String,
) -> Result<(), ApiError> {
    let chat_id = store.live_message_chat(&id).map_err(|error| {
        log::warn!(
            "entry for the settings window not found: {}",
            error.category()
        );
        ApiError::from(error.code())
    })?;
    show_main(&app);
    let _ = app.emit_to(
        MAIN_LABEL,
        OPEN_MESSAGE,
        serde_json::json!({"chatId": chat_id, "messageId": id}),
    );
    Ok(())
}
