//! Окно настроек (слайс 4b, ADR-0010) через IPC на mock-рантайме с настоящими capabilities: матрица «окно → команды»,
//! отрицательные тесты, события `settings-changed` и `open-message`, «Файл настроек».

mod common;

use common::*;
use kinige_lib::settings_window::{OPEN_MESSAGE, SETTINGS_CHANGED};
use kinige_lib::{CAPTURE_COMMANDS, COMMANDS, MAIN_COMMANDS, SETTINGS_COMMANDS};
use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tauri::{Listener, Manager};

const INBOX: &str = kinige_domain::groups::INBOX_ID;

#[test]
fn the_settings_window_can_call_every_command_of_its_set() {
    let fixture = fixture();
    let settings = window(&fixture, "settings");
    for command in SETTINGS_COMMANDS
        .iter()
        .filter(|c| !NEEDS_NATIVE_UI.contains(c))
    {
        if let Err(error) = call(&settings, command, args_for(command)) {
            assert!(
                !is_acl_denial(&error),
                "{command}: окну settings разрешена, получили: {error}"
            );
        }
    }
}

#[test]
fn the_settings_window_is_denied_the_feed_search_writing_import_restore_and_capture_commands() {
    let fixture = fixture();
    let settings = window(&fixture, "settings");
    let mut denied = 0;
    for command in COMMANDS.iter().filter(|c| !SETTINGS_COMMANDS.contains(c)) {
        let error = call(&settings, command, args_for(command))
            .expect_err(&format!("{command} окну настроек недоступна"));
        assert!(is_acl_denial(&error), "{command}: {error}");
        denied += 1;
    }
    assert!(denied > 30, "проверены все чужие команды: {denied}");
    // Названные в спецификации обязательно среди них.
    for command in [
        "list_messages",
        "search_messages",
        "send_message",
        "purge_message",
        "restore_backup",
        "import_telegram_open",
        "import_telegram_run",
        "capture_state",
        "capture_send",
        "list_chats",
        "open_settings",
        "app_info",
    ] {
        assert!(!SETTINGS_COMMANDS.contains(&command), "{command}");
        assert!(is_acl_denial(
            &call(&settings, command, args_for(command)).unwrap_err()
        ));
    }
    // Плагины и события: слушать можно, остальное нельзя.
    for cmd in [
        "plugin:event|emit",
        "plugin:event|emit_to",
        "plugin:window|close",
        "plugin:window|set_size",
        "plugin:window|create",
        "plugin:webview|create_webview_window",
        "plugin:app|version",
        "plugin:fs|read_file",
        "plugin:opener|open_url",
        "plugin:opener|open_path",
        "plugin:dialog|open",
        "plugin:clipboard-manager|read_text",
        "plugin:global-shortcut|register",
        "plugin:global-shortcut|unregister_all",
        "plugin:shell|open",
        "plugin:log|log",
        "no_such_command",
    ] {
        let error = call(
            &settings,
            cmd,
            json!({"url": "https://example.com", "path": "/tmp"}),
        )
        .expect_err(&format!("{cmd} окну настроек недоступна"));
        assert!(is_acl_denial(&error), "{cmd}: {error}");
    }
}

#[test]
fn the_main_window_lost_the_moved_commands_and_keeps_get_settings_and_the_new_open_command() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    for command in [
        "set_text_size",
        "choose_mirror_folder",
        "set_shortcut",
        "reset_shortcut",
        "shortcut_recording",
        "get_agent_access",
        "set_agent_enabled",
        "set_agent_mode",
        "set_agent_chats",
        "list_agent_log",
        "list_agent_log_entries",
        "get_settings_file",
        "settings_open_entry",
    ] {
        assert!(!MAIN_COMMANDS.contains(&command), "{command}");
        let error = call(&main, command, args_for(command)).unwrap_err();
        assert!(is_acl_denial(&error), "{command}: {error}");
    }
    for command in ["get_settings", "get_language", "open_settings"] {
        assert!(MAIN_COMMANDS.contains(&command), "{command}");
    }
    assert!(call(&main, "get_settings", json!({})).is_ok());
}

#[test]
fn the_capture_window_still_has_exactly_seven_commands_and_none_of_the_settings_ones() {
    assert_eq!(CAPTURE_COMMANDS.len(), 7);
    let fixture = fixture();
    let capture = window(&fixture, "capture");
    for command in SETTINGS_COMMANDS.iter().chain(&["open_settings"]) {
        let error = call(&capture, command, args_for(command)).unwrap_err();
        assert!(is_acl_denial(&error), "{command}: {error}");
    }
}

#[test]
fn the_settings_and_main_windows_may_subscribe_to_the_new_events() {
    let fixture = fixture();
    for label in ["settings", "main"] {
        let window = window(&fixture, label);
        for event in [SETTINGS_CHANGED, OPEN_MESSAGE] {
            for cmd in ["plugin:event|listen", "plugin:event|unlisten"] {
                let result = call(
                    &window,
                    cmd,
                    json!({"event": event, "target": {"kind": "Any"}, "handler": 1, "eventId": 1}),
                );
                if let Err(error) = result {
                    assert!(!is_acl_denial(&error), "{label} {cmd} {event}: {error}");
                }
            }
        }
    }
    let capture = window(&fixture, "capture");
    let error = call(
        &capture,
        "plugin:event|listen",
        json!({"event": SETTINGS_CHANGED, "target": {"kind": "Any"}, "handler": 1, "eventId": 1}),
    )
    .unwrap_err();
    assert!(is_acl_denial(&error));
}

#[test]
fn an_edit_from_the_settings_window_notifies_both_windows_but_not_the_capture_window() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    let settings = window(&fixture, "settings");
    let capture = window(&fixture, "capture");
    let counters: Vec<Arc<AtomicUsize>> = [&main, &settings, &capture]
        .iter()
        .map(|w| {
            let counter = Arc::new(AtomicUsize::new(0));
            let c = counter.clone();
            w.listen(SETTINGS_CHANGED, move |_| {
                c.fetch_add(1, Ordering::SeqCst);
            });
            counter
        })
        .collect();
    call(&settings, "set_text_size", json!({"size": "large"})).unwrap();
    assert_eq!(counters[0].load(Ordering::SeqCst), 1, "главное окно узнало");
    assert_eq!(counters[1].load(Ordering::SeqCst), 1, "окно настроек тоже");
    assert_eq!(
        counters[2].load(Ordering::SeqCst),
        0,
        "окно захвата событий не получает"
    );
    assert_eq!(
        call(&main, "get_settings", json!({})).unwrap()["textSize"],
        "large"
    );
    // Размер текста попал и в файл.
    let text = std::fs::read_to_string(fixture.data.path().join("settings.json")).unwrap();
    assert!(text.contains("\"text_size\": \"large\""), "{text}");
}

#[test]
fn the_settings_file_state_reports_the_path_and_a_broken_file() {
    let fixture = fixture();
    let settings = window(&fixture, "settings");
    let state = call(&settings, "get_settings_file", json!({})).unwrap();
    assert_eq!(
        state["path"],
        fixture
            .data
            .path()
            .join("settings.json")
            .to_string_lossy()
            .as_ref()
    );
    assert_eq!(state["problem"], Value::Null);
    assert_eq!(state["issues"], json!([]));
}

#[test]
fn opening_an_entry_checks_it_and_tells_only_the_main_window() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    let settings = window(&fixture, "settings");
    let to_main = Arc::new(AtomicUsize::new(0));
    let payload: Arc<std::sync::Mutex<String>> = Arc::default();
    {
        let (c, p) = (to_main.clone(), payload.clone());
        main.listen(OPEN_MESSAGE, move |event| {
            c.fetch_add(1, Ordering::SeqCst);
            *p.lock().unwrap() = event.payload().to_string();
        });
    }
    let sent = call(
        &main,
        "send_message",
        json!({"chatId": INBOX, "body": "из журнала"}),
    )
    .unwrap();
    let id = sent["id"].as_str().unwrap();
    call(&settings, "settings_open_entry", json!({"id": id})).unwrap();
    assert_eq!(to_main.load(Ordering::SeqCst), 1);
    let parsed: Value = serde_json::from_str(&payload.lock().unwrap()).unwrap();
    assert_eq!(parsed, json!({"chatId": INBOX, "messageId": id}));
    // Нет записи, мусор, запись в корзине: «Запись удалена», событие не уходит.
    call(&main, "delete_message", json!({"id": id})).unwrap();
    for bad in [id, "нет-такой", "", "'; DROP TABLE message; --"] {
        let error = call(&settings, "settings_open_entry", json!({"id": bad})).unwrap_err();
        assert_eq!(error, json!({"code": "message_not_found"}), "{bad:?}");
    }
    assert_eq!(to_main.load(Ordering::SeqCst), 1);
    let _ = fixture.app.state::<kinige_store::Store>();
}

#[test]
fn closing_the_settings_window_ends_the_shortcut_recording() {
    // Закрытие окна во время записи сочетания снимает режим записи: иначе глобальные сочетания остались бы выключенными.
    let fixture = fixture();
    let settings = window(&fixture, "settings");
    call(&settings, "shortcut_recording", json!({"active": true})).unwrap();
    assert!(
        fixture
            .app
            .state::<kinige_lib::hotkeys::Hotkeys>()
            .is_recording()
    );
    kinige_lib::settings_window::on_destroyed(fixture.app.handle());
    assert!(
        !fixture
            .app
            .state::<kinige_lib::hotkeys::Hotkeys>()
            .is_recording()
    );
}

#[test]
fn opening_the_settings_window_twice_gives_one_window() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    call(&main, "open_settings", json!({})).unwrap();
    call(&main, "open_settings", json!({})).unwrap();
    let count = fixture
        .app
        .webview_windows()
        .keys()
        .filter(|label| label.as_str() == "settings")
        .count();
    assert_eq!(count, 1);
}

#[test]
fn no_settings_window_is_created_while_the_journal_is_not_open() {
    // Восстановление и «база новее»: журнал не открыт, пункт «Настройки…» недоступен, а команда ничего не создаёт.
    let dir = tempfile::tempdir().unwrap();
    let fixture = fixture_without_store(&dir.path().join("kinige.db"));
    let main = window(&fixture, "main");
    let _ = call(&main, "open_settings", json!({}));
    assert!(fixture.app.get_webview_window("settings").is_none());
}
