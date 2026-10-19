//! Тест IPC с настоящими capabilities и CSP из `tauri.conf.json` (mock-рантайм).

use kinige_lib::builder;
use tauri::test::{INVOKE_KEY, get_ipc_response, mock_builder};
use tauri::webview::InvokeRequest;
use tauri::{App, WebviewWindowBuilder, ipc::CallbackFn, ipc::InvokeBody};

fn request(cmd: &str) -> InvokeRequest {
    InvokeRequest {
        cmd: cmd.into(),
        callback: CallbackFn(0),
        error: CallbackFn(1),
        url: "tauri://localhost".parse().unwrap(),
        body: InvokeBody::default(),
        headers: Default::default(),
        invoke_key: INVOKE_KEY.to_string(),
    }
}

fn app() -> App<tauri::test::MockRuntime> {
    builder(mock_builder())
        .build(tauri::generate_context!())
        .expect("не удалось собрать приложение")
}

#[test]
fn main_window_can_call_app_info() {
    let app = app();
    let window = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();

    let response = get_ipc_response(&window, request("app_info")).expect("команда разрешена");
    let value: serde_json::Value = response.deserialize().unwrap();
    assert_eq!(
        value["title"],
        format!("Kinige {}", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn other_window_cannot_call_app_info() {
    let app = app();
    let window = WebviewWindowBuilder::new(&app, "not-main", Default::default())
        .build()
        .unwrap();

    let result = get_ipc_response(&window, request("app_info"));
    let error = result.expect_err("окно без capability не должно вызывать команду");
    assert!(
        error.as_str().is_some_and(|e| e.contains("not allowed")),
        "ожидали отказ ACL, получили: {error}"
    );
}

#[test]
fn main_window_cannot_call_unlisted_commands() {
    let app = app();
    let window = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();

    // Ни команды ядра Tauri, ни несуществующие команды из capability не выданы.
    for cmd in [
        "plugin:window|close",
        "plugin:fs|read_file",
        "no_such_command",
    ] {
        assert!(
            get_ipc_response(&window, request(cmd)).is_err(),
            "{cmd} не должна быть доступна главному окну"
        );
    }
}
