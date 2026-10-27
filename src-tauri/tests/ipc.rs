//! Тест IPC с настоящими capabilities и CSP из `tauri.conf.json` (mock-рантайм): у каждого окна свой набор команд, чужие и
//! команды плагинов недоступны; плагины opener и dialog из JS недоступны; ссылки проходят проверку схемы. Сценарии через
//! IPC (чаты, запись, корзина, захват, сочетания, восстановление) — в `ipc_flows.rs`.

mod common;

use common::*;
use kinige_lib::capture::{ENTRIES_CHANGED, OPEN_CHAT};
use kinige_lib::recovery::DATABASE_CORRUPT;
use kinige_lib::{CAPTURE_COMMANDS, COMMANDS, MAIN_COMMANDS};
use serde_json::json;

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
fn capture_resize_takes_only_a_number_and_never_fails_on_extreme_values() {
    let fixture = fixture();
    let window = window(&fixture, "capture");
    for height in [json!(0), json!(-1.0), json!(1.0e12), json!(168.5)] {
        call(&window, "capture_resize", json!({ "height": height }))
            .unwrap_or_else(|error| panic!("{height}: {error}"));
    }
    for args in [
        json!({}),
        json!({"height": "высокое"}),
        json!({"height": null}),
    ] {
        let error = call(&window, "capture_resize", args).expect_err("не число — ошибка");
        assert!(!is_acl_denial(&error), "{error}");
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

/// Команды слайса 3: чаты, действия с записью, корзина, настройки сайдбара, восстановление. Окну захвата — отказ на
/// каждую; его набор остаётся шестью командами (§12.3).
const SLICE_THREE: &[&str] = &[
    "create_chat",
    "rename_chat",
    "set_chat_icon",
    "set_chat_pinned",
    "set_chat_archived",
    "delete_message",
    "restore_message",
    "edit_message",
    "toggle_task_item",
    "set_message_flag",
    "set_message_date",
    "list_trash",
    "purge_message",
    "set_sidebar_collapsed",
    "add_recent_emoji",
    "startup_state",
    "restore_backup",
    "restart_for_recovery",
];

#[test]
fn every_slice_three_command_is_main_only_and_the_capture_set_is_unchanged() {
    assert_eq!(
        CAPTURE_COMMANDS,
        [
            "capture_state",
            "capture_send",
            "capture_append",
            "capture_save_draft",
            "capture_hide",
            "capture_resize"
        ],
        "набор окна захвата не меняется"
    );
    let fixture = fixture();
    let main = window(&fixture, "main");
    let capture = window(&fixture, "capture");
    for command in SLICE_THREE {
        assert!(
            MAIN_COMMANDS.contains(command),
            "{command} выдана главному окну"
        );
        assert!(!CAPTURE_COMMANDS.contains(command), "{command}");
        let error = call(&capture, command, args_for(command))
            .expect_err(&format!("{command} недоступна окну захвата"));
        assert!(
            is_acl_denial(&error),
            "{command}: ожидали отказ ACL, получили: {error}"
        );
        if !NEEDS_NATIVE_UI.contains(command)
            && let Err(error) = call(&main, command, args_for(command))
        {
            assert!(!is_acl_denial(&error), "{command}: {error}");
        }
    }
}

#[test]
fn capture_window_cannot_call_anything_but_its_six_commands() {
    let fixture = fixture();
    let capture = window(&fixture, "capture");
    // Чужие команды: лента, чаты, отправка по chat_id, настройки, диалог, ссылки, черновик главного окна, сочетания,
    // действия слайса 3.
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
        // Размер и положение окна страница меняет только своей командой `capture_resize` (высота в узких пределах).
        "plugin:window|set_size",
        "plugin:window|set_position",
        "plugin:window|set_min_size",
        "plugin:window|set_max_size",
        "plugin:window|set_resizable",
        "plugin:window|set_ignore_cursor_events",
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
            json!({"url": "https://example.com", "path": "/tmp", "event": ENTRIES_CHANGED}),
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
        // Удаления чата нет (ТЗ §2.1: архив); «очистить корзину» тоже (решение пользователя 4).
        "delete_chat",
        "purge_all",
        "empty_trash",
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
fn main_window_may_subscribe_to_its_events() {
    let fixture = fixture();
    let window = window(&fixture, "main");
    // Подписка на события приложения: записи изменились, открыть чат, база повреждена.
    for event in [ENTRIES_CHANGED, OPEN_CHAT, DATABASE_CORRUPT] {
        for cmd in ["plugin:event|listen", "plugin:event|unlisten"] {
            let result = call(
                &window,
                cmd,
                json!({"event": event, "target": {"kind": "Any"}, "handler": 1, "eventId": 1}),
            );
            if let Err(error) = result {
                assert!(!is_acl_denial(&error), "{cmd} {event}: {error}");
            }
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
