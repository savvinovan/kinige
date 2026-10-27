//! Общее для тестов IPC: приложение на mock-рантайме с настоящими capabilities и CSP из `tauri.conf.json`, окна, вызов
//! команды и нормальные аргументы каждой команды.

#![allow(dead_code)]

use kinige_lib::builder;
use kinige_lib::capture::CaptureShell;
use kinige_lib::hotkeys::{Hotkeys, Refused, Registrar};
use kinige_lib::recovery::Recovery;
use kinige_store::{Store, StoreOptions};
use serde_json::{Value, json};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use tauri::test::{INVOKE_KEY, MockRuntime, get_ipc_response, mock_builder};
use tauri::webview::InvokeRequest;
use tauri::{App, Manager, WebviewWindow, WebviewWindowBuilder, ipc::CallbackFn, ipc::InvokeBody};

pub fn request(cmd: &str, args: Value) -> InvokeRequest {
    InvokeRequest {
        cmd: cmd.into(),
        callback: CallbackFn(0),
        error: CallbackFn(1),
        url: "tauri://localhost".parse().unwrap(),
        body: InvokeBody::Json(args),
        headers: Default::default(),
        invoke_key: INVOKE_KEY.to_string(),
    }
}

/// Реестр сочетаний без системы: «занятые» задаёт тест.
#[derive(Default)]
pub struct FakeRegistrar {
    pub taken: Mutex<HashSet<String>>,
}

/// Общий реестр: тест держит одну ссылку, приложение — другую.
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

/// Приложение с настоящими capabilities. `generate_context!` вызывается в бинарнике один раз (иначе повтор вшитого
/// Info.plist).
fn new_app() -> App<MockRuntime> {
    builder(mock_builder())
        .build(tauri::generate_context!())
        .expect("не удалось собрать приложение")
}

pub struct Fixture {
    pub app: App<MockRuntime>,
    pub registrar: Arc<FakeRegistrar>,
    pub data: tempfile::TempDir,
}

pub fn fixture() -> Fixture {
    fixture_with_taken(&[])
}

pub fn fixture_with_taken(taken: &[&str]) -> Fixture {
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
    app.manage(Recovery::new(db_path, false));
    Fixture {
        app,
        registrar,
        data,
    }
}

/// Как `fixture`, но хранилище открыто с настоящим хуком повреждения приложения (флаг, отметка, событие окну): так, как его
/// открывает `open_store`.
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
    app.manage(Recovery::new(db_path, false).with_corruption_flag(flag));
    Fixture {
        app,
        registrar,
        data,
    }
}

/// Приложение, в котором хранилище не открыто (база повреждена): управляемо только состояние восстановления.
pub fn fixture_without_store(db_path: &std::path::Path) -> Fixture {
    let app = new_app();
    app.manage(Recovery::new(db_path.to_path_buf(), true));
    Fixture {
        app,
        registrar: Arc::new(FakeRegistrar::default()),
        data: tempfile::tempdir().unwrap(),
    }
}

/// Приложение, в котором база новее приложения (§10.4): хранилище не открыто, копии не предлагаются.
pub fn fixture_too_new(db_path: &std::path::Path) -> Fixture {
    let app = new_app();
    app.manage(Recovery::database_too_new(db_path.to_path_buf()));
    Fixture {
        app,
        registrar: Arc::new(FakeRegistrar::default()),
        data: tempfile::tempdir().unwrap(),
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

/// Нормальные аргументы каждой команды, чтобы вызов дошёл до обработчика, если ACL пропустил.
pub fn args_for(command: &str) -> Value {
    let inbox = kinige_domain::groups::INBOX_ID;
    match command {
        "list_messages" => json!({"chatId": inbox}),
        "send_message" => json!({"chatId": inbox, "body": "проверка"}),
        "set_text_size" => json!({"size": "normal"}),
        "open_external_url" => json!({"url": "file:///etc/passwd"}),
        "get_draft" => json!({"chatId": inbox}),
        "save_draft" => json!({"chatId": inbox, "body": "черновик"}),
        "set_shortcut" => json!({"kind": "capture", "accelerator": "Ctrl+Alt+KeyN"}),
        "reset_shortcut" => json!({"kind": "capture"}),
        "shortcut_recording" => json!({"active": false}),
        "capture_send" | "capture_append" | "capture_save_draft" => json!({"body": "из окна"}),
        "capture_resize" => json!({"height": 200.0}),
        "create_chat" => json!({"title": "Книги", "icon": "📚"}),
        "rename_chat" => json!({"id": inbox, "title": "Другое"}),
        "set_chat_icon" => json!({"id": inbox, "icon": "🎬"}),
        "set_chat_pinned" => json!({"id": inbox, "pinned": true}),
        "set_chat_archived" => json!({"id": inbox, "archived": true}),
        "delete_message" | "restore_message" | "purge_message" => json!({"id": "нет-такой"}),
        "edit_message" => json!({"id": "нет-такой", "body": "правка"}),
        "toggle_task_item" => json!({"id": "нет-такой", "index": 0, "checked": true}),
        "move_message" => json!({"id": "нет-такой", "chatId": inbox}),
        "set_message_flag" => json!({"id": "нет-такой", "flag": "task", "on": true}),
        "set_message_date" => json!({"id": "нет-такой", "journalDate": "2026-10-01"}),
        "set_sidebar_collapsed" => json!({"collapsed": true}),
        "add_recent_emoji" | "capture_add_recent_emoji" => json!({"emoji": "📚"}),
        "restore_backup" => json!({"name": "daily-2026-10-21.db"}),
        _ => json!({}),
    }
}

/// Эти команды нельзя вызвать в mock-рантайме по-настоящему: диалог ждёт главный поток ОС, перезапуск завершает процесс.
pub const NEEDS_NATIVE_UI: &[&str] = &["choose_mirror_folder", "restart_for_recovery"];
