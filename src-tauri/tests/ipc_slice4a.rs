//! Команды окна «Доступ ИИ-агентов» (слайс 4a) через IPC на mock-рантайме с настоящими capabilities: только главное окно,
//! окну захвата — отказ на каждую, набор окна захвата не изменился (§12.3).

mod common;

use common::*;
use kinige_lib::{CAPTURE_COMMANDS, MAIN_COMMANDS};
use kinige_store::Store;
use serde_json::json;
use tauri::Manager;

const SLICE_FOUR_A: &[&str] = &[
    "get_agent_access",
    "set_agent_enabled",
    "set_agent_mode",
    "set_agent_chats",
    "list_agent_log",
    "list_agent_log_entries",
];

#[test]
fn agent_commands_are_main_only_and_the_capture_window_still_has_seven() {
    assert_eq!(
        CAPTURE_COMMANDS.len(),
        7,
        "у окна захвата по-прежнему семь команд"
    );
    let fixture = fixture();
    let main = window(&fixture, "main");
    let capture = window(&fixture, "capture");
    let other = window(&fixture, "not-main");
    for command in SLICE_FOUR_A {
        assert!(MAIN_COMMANDS.contains(command), "{command}");
        assert!(!CAPTURE_COMMANDS.contains(command), "{command}");
        for denied in [&capture, &other] {
            let error = call(denied, command, args_for(command))
                .expect_err(&format!("{command}: окну без права нельзя"));
            assert!(is_acl_denial(&error), "{command}: {error}");
        }
        let result = call(&main, command, args_for(command));
        if let Err(error) = &result {
            assert!(!is_acl_denial(error), "{command}: {error}");
        }
    }
}

#[test]
fn a_denied_window_changes_nothing_in_the_settings() {
    let fixture = fixture();
    let capture = window(&fixture, "capture");
    let _ = call(&capture, "set_agent_enabled", json!({"enabled": true}));
    let _ = call(&capture, "set_agent_mode", json!({"mode": "read_write"}));
    let store = fixture.app.state::<Store>();
    let access = store.agent_access().unwrap();
    assert!(
        !access.enabled,
        "доступ выключен по умолчанию, окно захвата его не включит"
    );
    assert_eq!(access.mode, kinige_domain::AgentMode::Read);
}

#[test]
fn the_main_window_drives_the_settings_and_sees_the_socket_state() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    call(&main, "set_agent_enabled", json!({"enabled": true})).unwrap();
    call(&main, "set_agent_mode", json!({"mode": "read_write"})).unwrap();
    call(
        &main,
        "set_agent_chats",
        json!({"chatIds": [kinige_domain::groups::INBOX_ID]}),
    )
    .unwrap();
    let access = call(&main, "get_agent_access", json!({})).unwrap();
    assert_eq!(access["enabled"], true);
    assert_eq!(access["mode"], "read_write");
    let inbox = access["chats"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["isInbox"] == true)
        .unwrap();
    assert_eq!(inbox["visible"], true);
    // В тесте слушателя нет: сокет не слушает, но окно получает ответ, а не ошибку.
    assert!(access["socket"].is_object());
    // Неизвестный чат не принимается, настройка не меняется.
    let error = call(
        &main,
        "set_agent_chats",
        json!({"chatIds": ["0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d"]}),
    )
    .unwrap_err();
    assert_eq!(error, json!({"code": "chat_not_found"}));
    let after = call(&main, "get_agent_access", json!({})).unwrap();
    assert_eq!(after["chats"], access["chats"]);
    // Мусор вместо режима — ошибка разбора, не паника.
    assert!(call(&main, "set_agent_mode", json!({"mode": "root"})).is_err());
    assert!(call(&main, "list_agent_log", json!({"before": "x"})).is_err());
    let page = call(&main, "list_agent_log", json!({})).unwrap();
    assert_eq!(page["rows"], json!([]));
}
