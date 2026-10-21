//! Тест IPC с настоящими capabilities и CSP из `tauri.conf.json` (mock-рантайм): каждая команда доступна окну `main`
//! и недоступна окну с другой меткой; плагины opener и dialog из JS недоступны; ссылки проходят проверку схемы.

use kinige_lib::{COMMANDS, builder};
use kinige_store::{Store, StoreOptions};
use serde_json::{Value, json};
use tauri::test::{INVOKE_KEY, MockRuntime, get_ipc_response, mock_builder};
use tauri::webview::InvokeRequest;
use tauri::{App, Manager, WebviewWindow, WebviewWindowBuilder, ipc::CallbackFn, ipc::InvokeBody};

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

struct Fixture {
    app: App<MockRuntime>,
    _data: tempfile::TempDir,
}

fn fixture() -> Fixture {
    let data = tempfile::tempdir().unwrap();
    let mut options = StoreOptions::new(
        data.path().join("kinige.db"),
        Some(data.path().join("mirror")),
    );
    options.start_mirror = false;
    let store = Store::open(options).unwrap();
    let app = builder(mock_builder())
        .build(tauri::generate_context!())
        .expect("не удалось собрать приложение");
    app.manage(store);
    Fixture { app, _data: data }
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
    match command {
        "list_messages" => json!({"chatId": kinige_domain::groups::INBOX_ID}),
        "send_message" => json!({"chatId": kinige_domain::groups::INBOX_ID, "body": "проверка"}),
        "set_text_size" => json!({"size": "normal"}),
        "open_external_url" => json!({"url": "file:///etc/passwd"}),
        _ => json!({}),
    }
}

/// Эти команды нельзя вызвать в mock-рантайме по-настоящему: диалог ждёт главный поток ОС.
const NEEDS_NATIVE_UI: &[&str] = &["choose_mirror_folder"];

#[test]
fn main_window_can_call_every_command_of_the_slice() {
    let fixture = fixture();
    let window = window(&fixture, "main");
    for command in COMMANDS.iter().filter(|c| !NEEDS_NATIVE_UI.contains(c)) {
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
fn capture_like_second_window_gets_nothing_either() {
    // Заготовка слайса 2: окно захвата получит свою capability и свой набор; пока любое другое окно не может ничего.
    let fixture = fixture();
    let capture = window(&fixture, "capture");
    let error = call(&capture, "send_message", args_for("send_message")).unwrap_err();
    assert!(is_acl_denial(&error));
}

#[test]
fn main_window_cannot_call_unlisted_commands_or_plugin_commands() {
    let fixture = fixture();
    let window = window(&fixture, "main");
    // Команды из `core:default` (event, app) не выданы: capability перечисляет только свои `allow-*`.
    // Плагины opener и dialog подключены ради Rust-API, но их JS-команды окну не выданы: открыть адрес или показать диалог
    // из JS нельзя, только через наши команды, где проверяются схема и путь.
    for cmd in [
        "plugin:event|listen",
        "plugin:app|version",
        "plugin:window|close",
        "plugin:fs|read_file",
        "plugin:opener|open_url",
        "plugin:opener|open_path",
        "plugin:opener|reveal_item_in_dir",
        "plugin:dialog|open",
        "plugin:dialog|save",
        "plugin:dialog|message",
        "plugin:dialog|ask",
        "plugin:dialog|confirm",
        "plugin:shell|open",
        "no_such_command",
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
