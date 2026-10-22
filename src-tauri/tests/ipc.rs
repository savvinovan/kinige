//! Тест IPC с настоящими capabilities и CSP из `tauri.conf.json` (mock-рантайм): у каждого окна свой набор команд, чужие и
//! команды плагинов недоступны; плагины opener и dialog из JS недоступны; ссылки проходят проверку схемы; окно захвата
//! пишет только в Inbox и не читает ленту; событие «записи изменились» получает только главное окно.

use kinige_lib::capture::{CaptureShell, ENTRIES_CHANGED};
use kinige_lib::hotkeys::{Hotkeys, Refused, Registrar};
use kinige_lib::{CAPTURE_COMMANDS, COMMANDS, MAIN_COMMANDS, builder};
use kinige_store::{Store, StoreOptions};
use serde_json::{Value, json};
use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tauri::test::{INVOKE_KEY, MockRuntime, get_ipc_response, mock_builder};
use tauri::webview::InvokeRequest;
use tauri::{
    App, Listener, Manager, WebviewWindow, WebviewWindowBuilder, ipc::CallbackFn, ipc::InvokeBody,
};

fn request(cmd: &str, args: Value) -> InvokeRequest {
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
struct FakeRegistrar {
    taken: Mutex<HashSet<String>>,
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

struct Fixture {
    app: App<MockRuntime>,
    registrar: Arc<FakeRegistrar>,
    _data: tempfile::TempDir,
}

fn fixture() -> Fixture {
    fixture_with_taken(&[])
}

fn fixture_with_taken(taken: &[&str]) -> Fixture {
    let data = tempfile::tempdir().unwrap();
    let mut options = StoreOptions::new(
        data.path().join("kinige.db"),
        Some(data.path().join("mirror")),
    );
    options.start_mirror = false;
    let store = Store::open(options).unwrap();
    let registrar = Arc::new(FakeRegistrar::default());
    registrar
        .taken
        .lock()
        .unwrap()
        .extend(taken.iter().map(|s| s.to_string()));
    let hotkeys = Hotkeys::new(Box::new(Shared(registrar.clone())));
    hotkeys.register_saved(&store);
    let app = builder(mock_builder())
        .build(tauri::generate_context!())
        .expect("не удалось собрать приложение");
    app.manage(store);
    app.manage(hotkeys);
    app.manage(CaptureShell::default());
    Fixture {
        app,
        registrar,
        _data: data,
    }
}

fn window(fixture: &Fixture, label: &str) -> WebviewWindow<MockRuntime> {
    WebviewWindowBuilder::new(&fixture.app, label, Default::default())
        .build()
        .unwrap()
}

fn call(window: &WebviewWindow<MockRuntime>, cmd: &str, args: Value) -> Result<Value, Value> {
    get_ipc_response(window, request(cmd, args)).map(|body| body.deserialize().unwrap())
}

fn is_acl_denial(error: &Value) -> bool {
    error.as_str().is_some_and(|e| e.contains("not allowed"))
}

/// Нормальные аргументы каждой команды, чтобы вызов дошёл до обработчика, если ACL пропустил.
fn args_for(command: &str) -> Value {
    let inbox = kinige_domain::groups::INBOX_ID;
    match command {
        "list_messages" => json!({"chatId": inbox}),
        "send_message" => json!({"chatId": inbox, "body": "проверка"}),
        "set_text_size" => json!({"size": "normal"}),
        "open_external_url" => json!({"url": "file:///etc/passwd"}),
        "get_draft" => json!({"chatId": inbox}),
        "save_draft" => json!({"chatId": inbox, "body": "черновик"}),
        "set_shortcut" => json!({"kind": "capture", "accelerator": "Alt+Space"}),
        "reset_shortcut" => json!({"kind": "capture"}),
        "shortcut_recording" => json!({"active": false}),
        "capture_send" | "capture_append" | "capture_save_draft" => json!({"body": "из окна"}),
        _ => json!({}),
    }
}

/// Эти команды нельзя вызвать в mock-рантайме по-настоящему: диалог ждёт главный поток ОС.
const NEEDS_NATIVE_UI: &[&str] = &["choose_mirror_folder"];

#[test]
fn main_window_can_call_every_command_of_its_set() {
    let fixture = fixture();
    let window = window(&fixture, "main");
    for command in MAIN_COMMANDS
        .iter()
        .filter(|c| !NEEDS_NATIVE_UI.contains(c))
    {
        let result = call(&window, command, args_for(command));
        if let Err(error) = &result {
            assert!(
                !is_acl_denial(error),
                "{command}: окну main команда должна быть разрешена, получили: {error}"
            );
        }
    }
}

#[test]
fn capture_window_can_call_every_command_of_its_set() {
    let fixture = fixture();
    let window = window(&fixture, "capture");
    for command in CAPTURE_COMMANDS {
        let result = call(&window, command, args_for(command));
        if let Err(error) = &result {
            assert!(
                !is_acl_denial(error),
                "{command}: окну capture команда должна быть разрешена, получили: {error}"
            );
        }
    }
}

#[test]
fn every_command_is_denied_to_a_window_with_another_label() {
    let fixture = fixture();
    let other = window(&fixture, "not-main");
    for command in COMMANDS {
        let error = call(&other, command, args_for(command))
            .expect_err(&format!("{command}: окну без capability нельзя"));
        assert!(
            is_acl_denial(&error),
            "{command}: ожидали отказ ACL, получили: {error}"
        );
    }
}

#[test]
fn capture_window_cannot_call_anything_but_its_five_commands() {
    let fixture = fixture();
    let capture = window(&fixture, "capture");
    // Чужие команды: лента, чаты, отправка по chat_id, настройки, диалог, ссылки, черновик главного окна, сочетания.
    for command in MAIN_COMMANDS {
        let error = call(&capture, command, args_for(command))
            .expect_err(&format!("{command} недоступна окну захвата"));
        assert!(
            is_acl_denial(&error),
            "{command}: ожидали отказ ACL, получили: {error}"
        );
    }
    for command in [
        "list_messages",
        "list_chats",
        "send_message",
        "get_settings",
        "choose_mirror_folder",
        "open_external_url",
        "get_draft",
        "save_draft",
    ] {
        assert!(MAIN_COMMANDS.contains(&command), "{command}");
    }
    // События и плагины: ни подписки, ни отправки событий, ни окон, ни ссылок, ни диалогов, ни сочетаний.
    for cmd in [
        "plugin:event|listen",
        "plugin:event|unlisten",
        "plugin:event|emit",
        "plugin:event|emit_to",
        "plugin:app|version",
        "plugin:window|close",
        "plugin:window|hide",
        "plugin:window|show",
        "plugin:webview|create_webview_window",
        "plugin:fs|read_file",
        "plugin:opener|open_url",
        "plugin:opener|open_path",
        "plugin:dialog|open",
        "plugin:dialog|message",
        "plugin:clipboard-manager|read_text",
        "plugin:global-shortcut|register",
        "plugin:global-shortcut|unregister_all",
        "plugin:shell|open",
        "no_such_command",
    ] {
        let error = call(
            &capture,
            cmd,
            json!({"url": "https://example.com", "path": "/tmp", "event": "entries-changed"}),
        )
        .expect_err(&format!("{cmd} не должна быть доступна окну захвата"));
        assert!(
            is_acl_denial(&error),
            "{cmd}: ожидали отказ ACL, получили: {error}"
        );
    }
}

#[test]
fn main_window_cannot_call_capture_commands() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    for command in CAPTURE_COMMANDS {
        let error = call(&main, command, args_for(command))
            .expect_err(&format!("{command} недоступна главному окну"));
        assert!(
            is_acl_denial(&error),
            "{command}: ожидали отказ ACL, получили: {error}"
        );
    }
}

#[test]
fn main_window_cannot_call_unlisted_commands_or_plugin_commands() {
    let fixture = fixture();
    let window = window(&fixture, "main");
    // Из `core` выданы только подписка и отписка от события; остальное (отправка событий, окна, приложение) — нет.
    // Плагины opener, dialog и global-shortcut подключены ради Rust-API, но их JS-команды окну не выданы: открыть адрес,
    // показать диалог или занять сочетание из JS нельзя, только через наши команды, где всё проверяется.
    for cmd in [
        "plugin:event|emit",
        "plugin:event|emit_to",
        "plugin:app|version",
        "plugin:window|close",
        "plugin:window|hide",
        "plugin:fs|read_file",
        "plugin:opener|open_url",
        "plugin:opener|open_path",
        "plugin:opener|reveal_item_in_dir",
        "plugin:dialog|open",
        "plugin:dialog|save",
        "plugin:dialog|message",
        "plugin:dialog|ask",
        "plugin:dialog|confirm",
        "plugin:global-shortcut|register",
        "plugin:global-shortcut|unregister",
        "plugin:global-shortcut|unregister_all",
        "plugin:clipboard-manager|read_text",
        "plugin:shell|open",
        "no_such_command",
        // Отмена отправки убрана в слайсе 1 (решение пользователя); команды больше нет.
        "undo_send",
    ] {
        let error = call(
            &window,
            cmd,
            json!({"url": "https://example.com", "path": "/tmp"}),
        )
        .expect_err(&format!("{cmd} не должна быть доступна главному окну"));
        assert!(
            is_acl_denial(&error),
            "{cmd}: ожидали отказ ACL, получили: {error}"
        );
    }
}

#[test]
fn main_window_may_subscribe_to_events() {
    let fixture = fixture();
    let window = window(&fixture, "main");
    for cmd in ["plugin:event|listen", "plugin:event|unlisten"] {
        let result = call(
            &window,
            cmd,
            json!({"event": ENTRIES_CHANGED, "target": {"kind": "Any"}, "handler": 1, "eventId": 1}),
        );
        if let Err(error) = result {
            assert!(!is_acl_denial(&error), "{cmd}: {error}");
        }
    }
}

#[test]
fn open_external_url_rejects_everything_but_http_https_and_mailto() {
    let fixture = fixture();
    let window = window(&fixture, "main");
    for url in [
        "file:///etc/passwd",
        "javascript:alert(1)",
        "data:text/html,<script>alert(1)</script>",
        "tauri://localhost/",
        "asset://localhost/x",
        "ipc://localhost/app_info",
        "ftp://example.com/",
        "/relative/path",
        "relative",
        "//example.com",
        "",
        "   ",
        " https://example.com",
        "https://example.com\n",
    ] {
        let error = call(&window, "open_external_url", json!({ "url": url }))
            .expect_err(&format!("{url:?} должен быть отвергнут"));
        assert_eq!(error, json!({"code": "link_not_allowed"}), "{url:?}");
    }
}

#[test]
fn send_and_settings_work_end_to_end_over_ipc() {
    let fixture = fixture();
    let window = window(&fixture, "main");
    let inbox = kinige_domain::groups::INBOX_ID;

    let chats = call(&window, "list_chats", json!({})).unwrap();
    assert_eq!(chats[0]["id"], inbox);
    assert_eq!(chats[0]["count"], 0);

    let sent = call(
        &window,
        "send_message",
        json!({"chatId": inbox, "body": "Купить **молоко**"}),
    )
    .unwrap();
    assert_eq!(sent["bodyMd"], "Купить **молоко**");

    let page = call(
        &window,
        "list_messages",
        json!({"chatId": inbox, "limit": 50}),
    )
    .unwrap();
    assert_eq!(page["messages"].as_array().unwrap().len(), 1);
    assert_eq!(page["older"], Value::Null);
    assert_eq!(
        call(&window, "list_chats", json!({})).unwrap()[0]["count"],
        1
    );

    let empty = call(
        &window,
        "send_message",
        json!({"chatId": inbox, "body": "  \n "}),
    )
    .unwrap_err();
    assert_eq!(empty, json!({"code": "empty_message"}));

    call(&window, "set_text_size", json!({"size": "large"})).unwrap();
    let settings = call(&window, "get_settings", json!({})).unwrap();
    assert_eq!(settings["textSize"], "large");
    assert_eq!(settings["mirror"]["isDefault"], true);

    let language = call(&window, "get_language", json!({})).unwrap();
    assert!(language == "en" || language == "ru");
}

#[test]
fn capture_flow_over_ipc_creates_appends_keeps_the_draft_and_notifies_only_main() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    let capture = window(&fixture, "capture");
    let to_main = Arc::new(AtomicUsize::new(0));
    let to_capture = Arc::new(AtomicUsize::new(0));
    {
        let counter = to_main.clone();
        main.listen(ENTRIES_CHANGED, move |_| {
            counter.fetch_add(1, Ordering::SeqCst);
        });
        let counter = to_capture.clone();
        capture.listen(ENTRIES_CHANGED, move |_| {
            counter.fetch_add(1, Ordering::SeqCst);
        });
    }

    // Пустое состояние: ни контекста, ни цели дописывания, ни черновика, ни ошибки.
    let state = call(&capture, "capture_state", json!({})).unwrap();
    assert_eq!(state["context"], json!([]));
    assert_eq!(state["appendTarget"], Value::Null);
    assert_eq!(state["draft"], "");
    assert_eq!(state["error"], Value::Null);
    assert!(state["language"] == "en" || state["language"] == "ru");
    assert_eq!(state["textSize"], "normal");

    // Черновик окна не виден главному и наоборот.
    call(
        &capture,
        "capture_save_draft",
        json!({"body": "недописано"}),
    )
    .unwrap();
    assert_eq!(
        call(&capture, "capture_state", json!({})).unwrap()["draft"],
        "недописано"
    );
    assert_eq!(call(&main, "get_draft", args_for("get_draft")).unwrap(), "");
    call(&main, "save_draft", args_for("save_draft")).unwrap();
    assert_eq!(
        call(&main, "get_draft", args_for("get_draft")).unwrap(),
        "черновик"
    );
    assert_eq!(
        call(&capture, "capture_state", json!({})).unwrap()["draft"],
        "недописано"
    );

    // Enter: новая запись; главное окно её видит и получает событие; черновик окна очищен, черновик главного цел.
    let created = call(
        &capture,
        "capture_send",
        json!({"body": "Идея: попробовать"}),
    )
    .unwrap();
    assert_eq!(created["action"], "created");
    assert_eq!(created["message"]["bodyMd"], "Идея: попробовать");
    assert_eq!(
        created["message"]["chatId"],
        kinige_domain::groups::INBOX_ID
    );
    assert_eq!(to_main.load(Ordering::SeqCst), 1);
    assert_eq!(
        to_capture.load(Ordering::SeqCst),
        0,
        "окно захвата события не получает"
    );
    let page = call(&main, "list_messages", args_for("list_messages")).unwrap();
    assert_eq!(page["messages"].as_array().unwrap().len(), 1);
    assert_eq!(
        call(&main, "get_draft", args_for("get_draft")).unwrap(),
        "черновик"
    );

    // Контекст и цель дописывания.
    let state = call(&capture, "capture_state", json!({})).unwrap();
    assert_eq!(state["draft"], "");
    assert_eq!(state["context"].as_array().unwrap().len(), 1);
    assert_eq!(state["appendTarget"], created["message"]["id"]);

    // ⌘Enter: дописано в ту же запись.
    let appended = call(&capture, "capture_append", json!({"body": "и ещё"})).unwrap();
    assert_eq!(appended["action"], "appended");
    assert_eq!(appended["message"]["id"], created["message"]["id"]);
    assert_eq!(appended["message"]["bodyMd"], "Идея: попробовать\n\nи ещё");
    assert_eq!(to_main.load(Ordering::SeqCst), 2);
    let page = call(&main, "list_messages", args_for("list_messages")).unwrap();
    assert_eq!(page["messages"].as_array().unwrap().len(), 1);
    assert_eq!(page["messages"][0]["bodyMd"], "Идея: попробовать\n\nи ещё");
    assert_eq!(call(&main, "list_chats", json!({})).unwrap()[0]["count"], 1);

    // Ошибки уходят кодом, событий нет.
    let before = to_main.load(Ordering::SeqCst);
    assert_eq!(
        call(&capture, "capture_send", json!({"body": "  "})).unwrap_err(),
        json!({"code": "empty_message"})
    );
    assert_eq!(
        call(&capture, "capture_append", json!({"body": ""})).unwrap_err(),
        json!({"code": "empty_message"})
    );
    assert_eq!(to_main.load(Ordering::SeqCst), before);

    // Прятать окно, которого в mock-рантайме нет, безопасно.
    call(&capture, "capture_hide", json!({})).unwrap();
}

#[test]
fn shortcuts_over_ipc_show_state_validate_and_roll_back() {
    let fixture = fixture_with_taken(&["Alt+Space", "Alt+Cmd+KeyN"]);
    let main = window(&fixture, "main");

    let settings = call(&main, "get_settings", json!({})).unwrap();
    assert_eq!(
        settings["shortcuts"],
        json!([
            {"kind": "capture", "accelerator": "Alt+Space", "isDefault": true, "state": "taken"},
            {"kind": "clipboard", "accelerator": "Alt+Shift+KeyV", "isDefault": true, "state": "assigned"},
        ]),
        "занятое сочетание видно в настройках, остальное работает"
    );

    // Занято — отказ до сохранения, в настройках прежнее.
    assert_eq!(
        call(
            &main,
            "set_shortcut",
            json!({"kind": "capture", "accelerator": "Cmd+Alt+KeyN"})
        )
        .unwrap_err(),
        json!({"code": "shortcut_taken"})
    );
    // Неверное, зарезервированное и совпадающее с другим действием — свои коды.
    for (accelerator, code) in [
        ("Space", "shortcut_invalid"),
        ("Cmd+KeyQ", "shortcut_invalid"),
        ("???", "shortcut_invalid"),
        ("Alt+Shift+KeyV", "shortcut_duplicate"),
    ] {
        assert_eq!(
            call(
                &main,
                "set_shortcut",
                json!({"kind": "capture", "accelerator": accelerator})
            )
            .unwrap_err(),
            json!({"code": code}),
            "{accelerator}"
        );
    }
    let after = call(&main, "get_settings", json!({})).unwrap();
    assert_eq!(after["shortcuts"][0]["accelerator"], "Alt+Space");

    // Свободное сочетание принято и сохранено в каноничном виде; ответ — настройки целиком.
    let changed = call(
        &main,
        "set_shortcut",
        json!({"kind": "capture", "accelerator": "Ctrl+Alt+KeyK"}),
    )
    .unwrap();
    assert_eq!(
        changed["shortcuts"][0],
        json!({"kind": "capture", "accelerator": "Ctrl+Alt+KeyK", "isDefault": false, "state": "assigned"})
    );
    // Сброс: умолчание занято другой программой — отказ, действующее остаётся.
    assert_eq!(
        call(&main, "reset_shortcut", json!({"kind": "capture"})).unwrap_err(),
        json!({"code": "shortcut_taken"})
    );
    fixture.registrar.taken.lock().unwrap().clear();
    let reset = call(&main, "reset_shortcut", json!({"kind": "capture"})).unwrap();
    assert_eq!(
        reset["shortcuts"][0],
        json!({"kind": "capture", "accelerator": "Alt+Space", "isDefault": true, "state": "assigned"})
    );

    // Режим записи включается и выключается без ошибок.
    call(&main, "shortcut_recording", json!({"active": true})).unwrap();
    call(&main, "shortcut_recording", json!({"active": false})).unwrap();
}
