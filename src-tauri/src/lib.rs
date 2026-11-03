//! The thin Tauri layer: windows and commands. Business logic lives in `kinige-domain`, storage in `kinige-store`.

mod actions;
mod agent_access;
pub mod attachments;
pub mod capture;
mod command_names;
mod commands;
pub mod guard;
pub mod hotkeys;
pub mod import;
pub mod logging;
pub mod media_protocol;
// The MCP server is a Unix socket (ADR-0009). Without Unix (Windows, slice 10a) a stub with the same surface stands in:
// there is no socket and the "AI agent access" page sees `unsupported`.
#[cfg(unix)]
pub mod mcp;
#[cfg(not(unix))]
#[path = "mcp_unsupported.rs"]
pub mod mcp;
#[cfg(target_os = "macos")]
mod panel;
pub mod panic_hook;
pub mod recovery;
pub mod settings_window;
pub mod startup;
pub mod tray;

use capture::{CAPTURE_LABEL, CaptureShell, MAIN_LABEL};
use hotkeys::{Hotkeys, PluginRegistrar};
use kinige_domain::ShortcutKind;
use kinige_store::{Store, StoreError, StoreOptions};
use recovery::Recovery;
use startup::{
    exit_after_failure, fail_fast, guarded, is_background_launch, launch_args,
    should_show_main_on_launch,
};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tauri::webview::PageLoadEvent;
use tauri::{AppHandle, Manager, Runtime, WindowEvent};
use tauri_plugin_global_shortcut::{Shortcut, ShortcutState};

pub use command_names::{CAPTURE_COMMANDS, COMMANDS, MAIN_COMMANDS, SETTINGS_COMMANDS};

/// The app constructor; shared by `run` and by the IPC tests (there with a mock runtime and their own `Store`).
/// The opener and dialog plugins are attached only for the Rust API: the window has no JS permissions for them (the capability does not grant them).
/// Global shortcuts, the tray and single-instance are attached only by `run`: they have no place in tests.
pub fn builder<R: Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    // The `kinige-media` protocol (slice 7) answers only the `main` webview; it is outside the capability system.
    media_protocol::register(builder)
        .plugin(guard::plugin())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(guarded::<R>(tauri::generate_handler![
            commands::app_info,
            commands::get_language,
            commands::list_chats,
            commands::list_messages,
            commands::send_message,
            attachments::attach_pick,
            attachments::attach_paste,
            attachments::attach_dropped,
            attachments::remove_draft_attachment,
            attachments::open_attachment,
            commands::get_settings,
            commands::set_text_size,
            commands::choose_mirror_folder,
            commands::open_external_url,
            commands::get_draft,
            commands::save_draft,
            commands::get_on_this_day,
            commands::dismiss_on_this_day,
            commands::get_forest_year,
            commands::get_forest_day_target,
            commands::set_forest_view,
            commands::set_shortcut,
            commands::reset_shortcut,
            commands::shortcut_recording,
            settings_window::open_settings,
            settings_window::get_settings_file,
            settings_window::settings_open_entry,
            actions::create_chat,
            actions::rename_chat,
            actions::set_chat_icon,
            actions::set_chat_pinned,
            actions::set_chat_archived,
            actions::delete_message,
            actions::restore_message,
            actions::edit_message,
            actions::toggle_task_item,
            actions::set_message_flag,
            actions::set_message_date,
            actions::move_message,
            actions::list_trash,
            actions::purge_message,
            actions::set_sidebar_collapsed,
            actions::add_recent_emoji,
            recovery::startup_state,
            recovery::restore_backup,
            recovery::restart_for_recovery,
            import::import_telegram_open,
            import::import_telegram_run,
            import::import_telegram_stop,
            import::import_telegram_close,
            import::search_messages,
            agent_access::get_agent_access,
            agent_access::set_agent_enabled,
            agent_access::set_agent_mode,
            agent_access::set_agent_chats,
            agent_access::list_agent_log,
            agent_access::list_agent_log_entries,
            capture::capture_state,
            capture::capture_send,
            capture::capture_append,
            capture::capture_save_draft,
            capture::capture_hide,
            capture::capture_resize,
            capture::capture_add_recent_emoji,
        ]))
}

/// Where the data lives. `KINIGE_DATA_DIR` and `KINIGE_MIRROR_DIR` are for checks on a clean machine and for autotests; a regular
/// launch does not set them: the database is in the app data directory, the copy is in `~/Documents/Kinige`.
fn db_path<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let data_dir = match std::env::var_os("KINIGE_DATA_DIR") {
        Some(dir) => PathBuf::from(dir),
        None => default_data_dir(app)?,
    };
    Ok(data_dir.join("kinige.db"))
}

/// The default data directory. On Windows it is the local one (`%LOCALAPPDATA%\me.savvinov.kinige`): a SQLite database
/// with its journal and WAL does not belong in the roaming profile (`%APPDATA%`), which is synced and copied around.
/// Elsewhere it is the app data directory as before (macOS: `~/Library/Application Support/me.savvinov.kinige`).
fn default_data_dir<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, tauri::Error> {
    if cfg!(windows) {
        app.path().app_local_data_dir()
    } else {
        app.path().app_data_dir()
    }
}

/// The default folder of the Markdown copy: `~/Documents/Kinige` (or `KINIGE_MIRROR_DIR`).
fn default_mirror_dir<R: Runtime>(app: &AppHandle<R>) -> Option<PathBuf> {
    match std::env::var_os("KINIGE_MIRROR_DIR") {
        Some(dir) => Some(PathBuf::from(dir)),
        None => app
            .path()
            .document_dir()
            .ok()
            .map(|documents| documents.join("Kinige")),
    }
}

/// Opens the store. Corruption found by the background integrity check or by any command on any thread (it
/// read a broken page) sets a mark, the process flag and tells the main window
/// (`recovery::corruption_hook`).
fn open_store<R: Runtime>(
    app: &AppHandle<R>,
    db_path: PathBuf,
    corruption: Arc<AtomicBool>,
) -> Result<Store, StoreError> {
    let mut options = StoreOptions::new(db_path.clone(), default_mirror_dir(app));
    options.on_corruption = Some(recovery::corruption_hook(app.clone(), db_path, corruption));
    // The first maintenance pass waits for the main window's page (`begin_maintenance` in `on_page_load`), so it is not on the
    // cold start path.
    options.defer_maintenance = true;
    Store::open(options)
}

/// Everything that works on top of the open store: shortcuts, the capture window as a panel, the menu bar icon. Called at
/// launch and, if the database was corrupt, right after restoring from a copy.
///
/// The store (and everything else that makes "the journal open") is handed to the app LAST, when everything that can fail
/// (the panel, the icon) has already succeeded: otherwise on a failure the restore would count as failed while the store already stood in the
/// app state, and a repeated "Restore" would replace the database file under an open writer.
fn start_services<R: Runtime>(
    app: &AppHandle<R>,
    store: Store,
) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "macos")]
    panel::convert(
        app,
        CAPTURE_LABEL,
        capture::on_capture_key,
        capture::on_capture_blur,
        capture::request_hide,
    )?;
    tray::setup(app, commands::get_language())?;
    // The settings file is applied first (ADR-0010): before registering shortcuts and before the MCP listener.
    let db = db_path(app)?;
    let data_dir = db.parent().ok_or("no data directory")?.to_path_buf();
    let settings_file = settings_window::start_file(app, &store, data_dir);
    let hotkeys = Hotkeys::new(Box::new(PluginRegistrar { app: app.clone() }));
    // A shortcut the system did not accept stays "unassigned" in the settings; the rest works.
    hotkeys.register_saved(&store);
    app.manage(store);
    app.manage(hotkeys);
    app.manage(settings_file);
    settings_window::set_available(app, true);
    app.manage(CaptureShell::default());
    app.manage(import::ImportState::default());
    app.manage(attachments::DropTokens::default());
    // The icon was built without the store (it was not yet in the app state): the recent chats now.
    tray::refresh(app);
    // The MCP server goes last, after `app.manage(store)`: a socket failure does not stop the journal (the error is visible in the settings).
    if let Ok(db_path) = db_path(app)
        && let Some(data_dir) = db_path.parent()
    {
        mcp::start(app, data_dir);
    }
    Ok(())
}

/// After restoring from a copy: the database opens, the services start on the main thread (the icon and the panel are AppKit).
pub(crate) fn resume_after_restore<R: Runtime>(app: &AppHandle<R>) -> Result<(), StoreError> {
    let db_path = db_path(app).map_err(|e| StoreError::Internal(e.to_string()))?;
    let corruption = app
        .try_state::<Recovery>()
        .map(|recovery| recovery.corruption_flag())
        .unwrap_or_default();
    let store = open_store(app, db_path, corruption)?;
    let (tx, rx) = std::sync::mpsc::channel();
    let handle = app.clone();
    app.run_on_main_thread(move || {
        let _ = tx.send(start_services(&handle, store).map_err(|e| e.to_string()));
    })
    .map_err(|e| StoreError::Internal(e.to_string()))?;
    rx.recv()
        .map_err(|e| StoreError::Internal(e.to_string()))?
        .map_err(StoreError::Internal)
}

/// A global shortcut press. While recording a new shortcut ("Settings"), presses are ignored.
fn on_shortcut<R: Runtime>(app: &tauri::AppHandle<R>, pressed: &Shortcut) {
    let Some(hotkeys) = app.try_state::<Hotkeys>() else {
        return;
    };
    if hotkeys.is_recording() {
        return;
    }
    let kind = hotkeys.kind_for(|accelerator| {
        accelerator
            .parse::<Shortcut>()
            .is_ok_and(|shortcut| shortcut == *pressed)
    });
    match kind {
        Some(ShortcutKind::Capture) => capture::toggle_capture(app),
        Some(ShortcutKind::Clipboard) => capture::save_clipboard(app),
        None => {}
    }
}

/// Closing the main window hides it, the app lives in the menu bar; closing the capture window is the same as Esc.
/// Public so that the IPC tests can send a window a drop (only `main` leaves a token).
pub fn on_window_event<R: Runtime>(window: &tauri::Window<R>, event: &WindowEvent) {
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

pub fn run() {
    panic_hook::install();
    let configured = builder(
        tauri::Builder::default()
            // Must go first: a second launch shows the main window of the existing instance and exits.
            .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
                // The MCP bridge starts the app with `--background`: an already running app does not need to show a window.
                if !is_background_launch(args) {
                    capture::show_main(app);
                }
            }))
            // After single-instance: the second instance exits without opening the log file.
            .plugin(logging::plugin())
            .plugin(
                tauri_plugin_global_shortcut::Builder::new()
                    .with_handler(|app, shortcut, event| {
                        if event.state == ShortcutState::Pressed {
                            on_shortcut(app, shortcut);
                        }
                    })
                    .build(),
            ),
    );
    // The capture window is an NSPanel without activating the app (ADR-0006).
    #[cfg(target_os = "macos")]
    let configured = configured.plugin(tauri_nspanel::init());
    // The app menu with "Settings..." (Cmd+,), slice 4b.
    #[cfg(target_os = "macos")]
    let configured = configured
        .menu(settings_window::app_menu)
        .on_menu_event(|app, event| {
            if event.id().as_ref() == settings_window::ID_APP_SETTINGS {
                settings_window::open(app);
            }
        });
    let setup = |app: &mut tauri::App| -> Result<(), Box<dyn std::error::Error>> {
        log::info!("starting Kinige {}", env!("CARGO_PKG_VERSION"));
        let handle = app.handle().clone();
        let db_path = db_path(&handle)?;
        // The journal does not open, and the window shows the copies (`recovery`), if: the restore of the previous launch is not
        // finished; the mark of the background check was confirmed by a repeated synchronous check (if not confirmed,
        // the mark is cleared and the database opens as usual); the database file is missing but copies exist; the database does not open as
        // corrupt. A database newer than the app (§10.4) gets a separate message. Any other open error is not
        // a reason to offer copies: the app does not start and says so.
        let mirror = default_mirror_dir(&handle);
        let mut too_new = false;
        // The "corruption found" flag is needed by the check hook, which starts together with the store, before `Recovery`.
        let corruption = Arc::new(AtomicBool::new(false));
        let store = if kinige_store::startup_check(&db_path) == kinige_store::StartupCheck::Recovery
        {
            None
        } else {
            match open_store(&handle, db_path.clone(), corruption.clone()) {
                Ok(store) => Some(store),
                Err(StoreError::Corrupt(_)) => {
                    log::error!("database is corrupt, offering copies");
                    None
                }
                Err(StoreError::TooNew(found)) => {
                    log::warn!("database was created by a newer version (schema {found})");
                    too_new = true;
                    None
                }
                Err(error) => return Err(Box::new(error)),
            }
        };
        let recovery = if too_new {
            Recovery::database_too_new(db_path)
        } else {
            Recovery::new(db_path, store.is_none())
        };
        app.manage(
            recovery
                .with_mirror(mirror)
                .with_corruption_flag(corruption),
        );
        let store_open = store.is_some();
        if let Some(store) = store {
            start_services(&handle, store)?;
        }
        // The main window is created hidden (`visible: false`) so that a background launch does not flash a window: we show it here
        // if the app was not started by the MCP bridge (`--background`, a menu bar icon without a window) or if the journal is not open
        // (the recovery screen must not be hidden). `show_main` also restores the regular activation policy (the Dock icon).
        if should_show_main_on_launch(is_background_launch(launch_args()), store_open) {
            capture::show_main(&handle);
        }
        Ok(())
    };
    let built = configured
        .setup(move |app| fail_fast(setup(app)))
        .on_window_event(on_window_event)
        .on_page_load(|webview, payload| {
            // Cold start measurement (`scripts/measure-cold-start.sh`): a mark for when the main window page has loaded.
            // Without `KINIGE_TIMING=1` the level of the `kinige-timing` target is off, and nothing is written.
            if webview.label() == MAIN_LABEL && payload.event() == PageLoadEvent::Finished {
                log::info!(target: logging::TIMING_TARGET, "page-loaded");
                // The first maintenance pass (integrity check, the day's backup, the attachment sweep) starts only now.
                if let Some(store) = webview.app_handle().try_state::<Store>() {
                    store.begin_maintenance();
                }
            }
        })
        .build(tauri::generate_context!());
    let mut app = match built {
        Ok(app) => app,
        Err(_) => {
            log::error!("building the app failed");
            exit_after_failure()
        }
    };

    // Launch by the bridge: no Dock icon while there is no window (the policy changes to regular in `capture::show_main`).
    #[cfg(target_os = "macos")]
    if is_background_launch(launch_args()) {
        app.set_activation_policy(tauri::ActivationPolicy::Accessory);
    }
    #[cfg(not(target_os = "macos"))]
    let _ = &mut app;
    app.run(|app, event| {
        // The MCP socket file must not outlive the app.
        if let tauri::RunEvent::Exit = event {
            mcp::shutdown(app);
        }
        // A click on the Dock icon when there are no windows on the screen shows the main window (the event exists only on macOS).
        #[cfg(target_os = "macos")]
        if let tauri::RunEvent::Reopen { .. } = event {
            capture::show_main(app);
        }
        #[cfg(not(target_os = "macos"))]
        let _ = (app, event);
    });
}
