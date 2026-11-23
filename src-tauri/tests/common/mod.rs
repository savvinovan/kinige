//! Shared by the IPC tests: an app on a mock runtime with the real capabilities and the CSP from `tauri.conf.json`, windows, a command
//! call and normal arguments for each command.

#![allow(dead_code)]

// The client of the MCP server over the Unix socket; there is no server on Windows (slice 10a).
pub mod bot_env;
pub mod fake_bot;
#[cfg(unix)]
pub mod mcp;
pub mod voice;

use kinige_lib::builder;
use kinige_lib::capture::CaptureShell;
use kinige_lib::hotkeys::{Hotkeys, Refused, Registrar};
use kinige_lib::recovery::Recovery;
use kinige_lib::stickers::secret::{MemorySecrets, OsSecrets, SecretStore};
use kinige_lib::stickers::{BotClient, StickerShell};
use kinige_store::{Store, StoreOptions};
use serde_json::{Value, json};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use tauri::test::{INVOKE_KEY, MockRuntime, get_ipc_response, mock_builder};
use tauri::webview::InvokeRequest;
use tauri::{App, Manager, WebviewWindow, WebviewWindowBuilder, ipc::CallbackFn, ipc::InvokeBody};

/// The address of the app page for Tauri: `http://tauri.localhost` on Windows, `tauri://localhost` elsewhere; the runtime
/// uses it to decide that a call comes from the app's own page (a capability with `local`).
#[cfg(windows)]
const APP_ORIGIN: &str = "http://tauri.localhost";
#[cfg(not(windows))]
const APP_ORIGIN: &str = "tauri://localhost";

pub fn request(cmd: &str, args: Value) -> InvokeRequest {
    InvokeRequest {
        cmd: cmd.into(),
        callback: CallbackFn(0),
        error: CallbackFn(1),
        url: APP_ORIGIN.parse().unwrap(),
        body: InvokeBody::Json(args),
        headers: Default::default(),
        invoke_key: INVOKE_KEY.to_string(),
    }
}

/// A shortcut registry without the system: the "taken" ones are set by the test.
#[derive(Default)]
pub struct FakeRegistrar {
    pub taken: Mutex<HashSet<String>>,
}

/// A shared registry: the test holds one reference, the app the other.
struct Shared(Arc<FakeRegistrar>);

impl Registrar for Shared {
    fn register(&self, accelerator: &str) -> Result<(), Refused> {
        if self.0.taken.lock().unwrap().contains(accelerator) {
            Err(Refused)
        } else {
            Ok(())
        }
    }

    fn unregister(&self, _accelerator: &str) {}
}

/// An app with the real capabilities. `generate_context!` is called once in the binary (otherwise a repeat of the embedded
/// Info.plist).
fn new_app() -> App<MockRuntime> {
    builder(mock_builder())
        .build(tauri::generate_context!())
        .expect("could not build the app")
}

pub struct Fixture {
    pub app: App<MockRuntime>,
    pub registrar: Arc<FakeRegistrar>,
    pub data: tempfile::TempDir,
    /// The bot token store of slice 5c, in memory and counting its calls (the secret store of the OS is never touched in a test).
    pub secrets: Arc<MemorySecrets>,
}

/// Manages the sticker state with an in-memory token store and a client whose base is a port nobody listens on: a test that reaches the
/// network by mistake fails with "unreachable", it never reaches Telegram.
pub fn manage_stickers(app: &App<MockRuntime>) -> Arc<MemorySecrets> {
    manage_stickers_over(app, false)
}

/// `real_store`: the shell holds the secret store of this platform (`OsSecrets`) instead of the in-memory one. Only for a test that
/// runs where that store answers without touching anything (Windows and Linux: "unsupported"); never on macOS, where it is the keychain.
fn manage_stickers_over(app: &App<MockRuntime>, real_store: bool) -> Arc<MemorySecrets> {
    let secrets = Arc::new(MemorySecrets::default());
    let store: Arc<dyn SecretStore> = if real_store {
        Arc::new(OsSecrets)
    } else {
        secrets.clone()
    };
    app.manage(StickerShell::new(
        store,
        Box::new(|| BotClient::with_base("http://127.0.0.1:9", false)),
    ));
    secrets
}

pub fn fixture() -> Fixture {
    fixture_with_taken(&[])
}

/// A fixture whose sticker shell holds the platform's own secret store. Compiled everywhere, called only off macOS.
#[cfg_attr(target_os = "macos", allow(dead_code))]
pub fn fixture_with_os_secrets() -> Fixture {
    fixture_built(&[], true)
}

pub fn fixture_with_taken(taken: &[&str]) -> Fixture {
    fixture_built(taken, false)
}

fn fixture_built(taken: &[&str], real_store: bool) -> Fixture {
    let data = tempfile::tempdir().unwrap();
    let db_path = data.path().join("kinige.db");
    let mut options = StoreOptions::new(&db_path, Some(data.path().join("mirror")));
    options.start_mirror = false;
    options.start_maintenance = false;
    let store = Store::open(options).unwrap();
    let registrar = Arc::new(FakeRegistrar::default());
    registrar
        .taken
        .lock()
        .unwrap()
        .extend(taken.iter().map(|s| s.to_string()));
    let hotkeys = Hotkeys::new(Box::new(Shared(registrar.clone())));
    hotkeys.register_saved(&store);
    let app = new_app();
    app.manage(store);
    app.manage(hotkeys);
    app.manage(CaptureShell::default());
    app.manage(kinige_lib::import::ImportState::default());
    app.manage(kinige_lib::attachments::DropTokens::default());
    app.manage(Recovery::new(db_path, false));
    let secrets = manage_stickers_over(&app, real_store);
    Fixture {
        app,
        registrar,
        data,
        secrets,
    }
}

/// Like `fixture`, but the store is opened with the app's real corruption hook (a flag, a mark, an event to the window): the way
/// `open_store` opens it.
pub fn fixture_with_corruption_hook() -> Fixture {
    let data = tempfile::tempdir().unwrap();
    let db_path = data.path().join("kinige.db");
    let app = new_app();
    let flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut options = StoreOptions::new(&db_path, Some(data.path().join("mirror")));
    options.start_mirror = false;
    options.start_maintenance = false;
    options.on_corruption = Some(kinige_lib::recovery::corruption_hook(
        app.handle().clone(),
        db_path.clone(),
        flag.clone(),
    ));
    let store = Store::open(options).unwrap();
    let registrar = Arc::new(FakeRegistrar::default());
    let hotkeys = Hotkeys::new(Box::new(Shared(registrar.clone())));
    hotkeys.register_saved(&store);
    app.manage(store);
    app.manage(hotkeys);
    app.manage(CaptureShell::default());
    app.manage(kinige_lib::import::ImportState::default());
    app.manage(kinige_lib::attachments::DropTokens::default());
    app.manage(Recovery::new(db_path, false).with_corruption_flag(flag));
    let secrets = manage_stickers(&app);
    Fixture {
        app,
        registrar,
        data,
        secrets,
    }
}

/// An app in which the store is not open (the database is corrupt): only the recovery state is managed.
pub fn fixture_without_store(db_path: &std::path::Path) -> Fixture {
    let app = new_app();
    app.manage(Recovery::new(db_path.to_path_buf(), true));
    Fixture {
        app,
        registrar: Arc::new(FakeRegistrar::default()),
        data: tempfile::tempdir().unwrap(),
        secrets: Arc::default(),
    }
}

/// An app in which the database is newer than the app (§10.4): the store is not open, copies are not offered.
pub fn fixture_too_new(db_path: &std::path::Path) -> Fixture {
    let app = new_app();
    app.manage(Recovery::database_too_new(db_path.to_path_buf()));
    Fixture {
        app,
        registrar: Arc::new(FakeRegistrar::default()),
        data: tempfile::tempdir().unwrap(),
        secrets: Arc::default(),
    }
}

pub fn window(fixture: &Fixture, label: &str) -> WebviewWindow<MockRuntime> {
    WebviewWindowBuilder::new(&fixture.app, label, Default::default())
        .build()
        .unwrap()
}

pub fn call(window: &WebviewWindow<MockRuntime>, cmd: &str, args: Value) -> Result<Value, Value> {
    get_ipc_response(window, request(cmd, args)).map(|body| body.deserialize().unwrap())
}

pub fn is_acl_denial(error: &Value) -> bool {
    error.as_str().is_some_and(|e| e.contains("not allowed"))
}

/// Normal arguments for each command, so that the call reaches the handler if the ACL lets it through.
pub fn args_for(command: &str) -> Value {
    let inbox = kinige_domain::groups::INBOX_ID;
    match command {
        "list_messages" => json!({"chatId": inbox}),
        "send_message" => json!({"chatId": inbox, "body": "проверка"}),
        "set_text_size" => json!({"size": "normal"}),
        "settings_open_entry" => json!({"id": "нет-такой"}),
        "open_external_url" => json!({"url": "file:///etc/passwd"}),
        "get_draft" => json!({"chatId": inbox}),
        "save_draft" => json!({"chatId": inbox, "body": "черновик"}),
        "dismiss_on_this_day" => json!({"date": "2026-10-01"}),
        "get_forest_year" => json!({"year": 2026, "firstWeekday": "monday"}),
        "get_forest_day_target" => json!({"date": "2026-10-16", "currentChatId": inbox}),
        "set_forest_view" => json!({"view": "strip", "mode": "forest"}),
        "set_shortcut" => json!({"kind": "capture", "accelerator": "Ctrl+Alt+KeyN"}),
        "reset_shortcut" => json!({"kind": "capture"}),
        "shortcut_recording" => json!({"active": false}),
        "capture_send" | "capture_append" | "capture_save_draft" => json!({"body": "из окна"}),
        "capture_resize" => json!({"height": 200.0}),
        "create_chat" => json!({"title": "Книги", "icon": "📚"}),
        "rename_chat" => json!({"id": inbox, "title": "Другое"}),
        "set_chat_icon" => json!({"id": inbox, "icon": "🎬"}),
        "set_chat_color" => json!({"id": inbox, "color": "grey"}),
        "set_chat_pinned" => json!({"id": inbox, "pinned": true}),
        "set_chat_archived" => json!({"id": inbox, "archived": true}),
        "delete_message" | "restore_message" | "purge_message" => json!({"id": "нет-такой"}),
        "edit_message" => json!({"id": "нет-такой", "body": "правка"}),
        "toggle_task_item" => json!({"id": "нет-такой", "index": 0, "checked": true}),
        "move_message" => json!({"id": "нет-такой", "chatId": inbox}),
        "set_message_flag" => json!({"id": "нет-такой", "flag": "task", "on": true}),
        "set_message_reaction" => json!({"id": "нет-такой", "emoji": "🔥", "on": true}),
        "set_message_date" => json!({"id": "нет-такой", "journalDate": "2026-10-01"}),
        "set_sidebar_collapsed" => json!({"collapsed": true}),
        "add_recent_emoji" | "capture_add_recent_emoji" => json!({"emoji": "📚"}),
        "restore_backup" => json!({"name": "daily-2026-10-21.db"}),
        "import_telegram_run" => json!({"session": 1, "chatIds": []}),
        "search_messages" => json!({"query": "проверка"}),
        "set_agent_enabled" => json!({"enabled": false}),
        "set_agent_mode" => json!({"mode": "read"}),
        "set_agent_chats" => json!({"chatIds": []}),
        "list_agent_log_entries" => json!({"logId": 1}),
        "attach_dropped" => json!({"token": "нет-такого", "chatId": inbox}),
        "set_drop_target" => json!({"active": true, "seq": 1}),
        "remove_draft_attachment" => json!({"chatId": inbox, "attachmentId": "нет-такого"}),
        "open_attachment" | "read_voice_for_waveform" => json!({"attachmentId": "нет-такого"}),
        "set_draft_attachment_details" | "set_attachment_details" => {
            json!({"attachmentId": "нет-такого", "changes": {"name": "пляж"}})
        }
        "set_attachment_waveform" => json!({"attachmentId": "нет-такого", "peaks": ""}),
        "add_sticker_pack" => json!({"link": "TestPack"}),
        "remove_sticker_pack" => json!({"packId": "нет-такого"}),
        "send_sticker" => json!({"chatId": inbox, "stickerId": "нет-такого"}),
        "send_gif_again" => json!({"chatId": inbox, "attachmentId": "нет-такого"}),
        "set_telegram_bot_token" => json!({"token": "42:fake-token-for-tests"}),
        "microphone_hold" | "capture_hold_microphone" => json!({"held": false}),
        // `send_voice` takes a raw body: a JSON call is the "garbage" case and must be an error, never a panic.
        _ => json!({}),
    }
}

/// The capture window's whole command set: the seven of slices 2 and 3 and, since KINIG-153, three for voice notes (recording to Inbox, the
/// microphone claim, the settings page of the microphone). Every test that states the set compares with this one.
pub const CAPTURE_SET: [&str; 10] = [
    "capture_state",
    "capture_send",
    "capture_append",
    "capture_save_draft",
    "capture_hide",
    "capture_resize",
    "capture_add_recent_emoji",
    "capture_send_voice",
    "capture_hold_microphone",
    "capture_open_microphone_settings",
];

/// These commands cannot be called for real in a mock runtime: a dialog waits for the OS main thread, a restart ends the process.
pub const NEEDS_NATIVE_UI: &[&str] = &[
    "choose_mirror_folder",
    "restart_for_recovery",
    "import_telegram_open",
    // The paperclip dialog.
    "attach_pick",
    // Opens the system settings page of the microphone: never from a test (slice 7a).
    "open_microphone_settings",
    "capture_open_microphone_settings",
];
