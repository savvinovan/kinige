//! Команды окна «Доступ ИИ-агентов» (слайс 4a) через IPC на mock-рантайме с настоящими capabilities. С слайса 4b они
//! живут в окне настроек (`settings`): главному окну и окну захвата — отказ на каждую, набор окна захвата не изменился (§12.3).

// Корень интеграционных тестов целиком — тестовый код: `cfg(test)` даёт хелперам вне #[test] те же послабления
// clippy.toml (unwrap/expect/panic/индексация), что и тестам в src/.
#![cfg(test)]

mod common;

use common::*;
use kinige_lib::{CAPTURE_COMMANDS, MAIN_COMMANDS, SETTINGS_COMMANDS};
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
fn agent_commands_are_settings_only_and_the_capture_window_still_has_seven() {
    assert_eq!(
        CAPTURE_COMMANDS.len(),
        7,
        "у окна захвата по-прежнему семь команд"
    );
    let fixture = fixture();
    let settings = window(&fixture, "settings");
    let main = window(&fixture, "main");
    let capture = window(&fixture, "capture");
    let other = window(&fixture, "not-main");
    for command in SLICE_FOUR_A {
        assert!(SETTINGS_COMMANDS.contains(command), "{command}");
        assert!(!MAIN_COMMANDS.contains(command), "{command}");
        assert!(!CAPTURE_COMMANDS.contains(command), "{command}");
        for denied in [&main, &capture, &other] {
            let error = call(denied, command, args_for(command))
                .expect_err(&format!("{command}: окну без права нельзя"));
            assert!(is_acl_denial(&error), "{command}: {error}");
        }
        let result = call(&settings, command, args_for(command));
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

#[cfg(unix)]
#[test]
fn the_settings_window_drives_the_settings_and_sees_the_socket_state() {
    let fixture = fixture();
    let main = window(&fixture, "settings");
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

/// Windows (slice 10a): there is no MCP server, the commands that change something answer `unsupported`, the setting does
/// not change, and `get_agent_access` gives the window the code `unsupported` instead of a socket state.
#[cfg(not(unix))]
#[test]
fn without_an_mcp_server_the_settings_window_gets_unsupported_and_nothing_changes() {
    let fixture = fixture();
    let settings = window(&fixture, "settings");
    let unsupported = json!({"code": "unsupported"});
    assert_eq!(
        call(&settings, "set_agent_enabled", json!({"enabled": true})).unwrap_err(),
        unsupported
    );
    assert_eq!(
        call(&settings, "set_agent_mode", json!({"mode": "read_write"})).unwrap_err(),
        unsupported
    );
    assert_eq!(
        call(
            &settings,
            "set_agent_chats",
            json!({"chatIds": [kinige_domain::groups::INBOX_ID]})
        )
        .unwrap_err(),
        unsupported
    );
    // Garbage instead of arguments is still a parse error, not a panic.
    assert!(call(&settings, "set_agent_mode", json!({"mode": "root"})).is_err());
    let access = call(&settings, "get_agent_access", json!({})).unwrap();
    assert_eq!(access["enabled"], false);
    assert_eq!(access["mode"], "read");
    assert_eq!(access["socket"]["listening"], false);
    assert_eq!(access["socket"]["error"], "unsupported");
    assert!(access["socket"]["socketPath"].is_null());
    assert!(access["socket"]["codeCommand"].is_null());
    // The access log is available (empty): the log page does not break.
    let page = call(&settings, "list_agent_log", json!({})).unwrap();
    assert_eq!(page["rows"], json!([]));
}
