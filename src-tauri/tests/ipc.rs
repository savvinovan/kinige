//! An IPC test with the real capabilities and the CSP from `tauri.conf.json` (a mock runtime): each window has its own set of commands, foreign
//! commands and plugin commands are unavailable; the opener and dialog plugins are unavailable from JS; links pass the scheme check. The scenarios through
//! IPC (chats, entries, trash, capture, shortcuts, restore) are in `ipc_flows.rs`.

// The whole root of the integration tests is test code: `cfg(test)` gives helpers outside #[test] the same relaxations
// of clippy.toml (unwrap/expect/panic/indexing) as the tests in src/.
#![cfg(test)]

mod common;

use common::*;
use kinige_lib::capture::{ENTRIES_CHANGED, OPEN_CHAT};
use kinige_lib::recovery::DATABASE_CORRUPT;
use kinige_lib::settings_window::{OPEN_MESSAGE, SETTINGS_CHANGED};
use kinige_lib::{CAPTURE_COMMANDS, COMMANDS, MAIN_COMMANDS, SETTINGS_COMMANDS};
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
                "{command}: the command must be allowed for the main window, got: {error}"
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
                "{command}: the command must be allowed for the capture window, got: {error}"
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
        let error = call(&window, "capture_resize", args).expect_err("not a number is an error");
        assert!(!is_acl_denial(&error), "{error}");
    }
}

#[test]
fn every_command_is_denied_to_a_window_with_another_label() {
    let fixture = fixture();
    let other = window(&fixture, "not-main");
    for command in COMMANDS {
        let error = call(&other, command, args_for(command)).expect_err(&format!(
            "{command}: not allowed for a window without a capability"
        ));
        assert!(
            is_acl_denial(&error),
            "{command}: expected an ACL refusal, got: {error}"
        );
    }
}

/// The slice 3 commands: chats, entry actions, trash, sidebar settings, restore. The capture window is refused on
/// each; its set is the six commands of slice 2 and `capture_add_recent_emoji` (§12.3).
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
    "move_message",
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
            "capture_resize",
            "capture_add_recent_emoji"
        ],
        "the capture window set: the six commands of slice 2 and one from the slice 3 acceptance decisions (recent emoji)"
    );
    let fixture = fixture();
    let main = window(&fixture, "main");
    let capture = window(&fixture, "capture");
    for command in SLICE_THREE {
        assert!(
            MAIN_COMMANDS.contains(command),
            "{command} is granted to the main window"
        );
        assert!(!CAPTURE_COMMANDS.contains(command), "{command}");
        let error = call(&capture, command, args_for(command))
            .expect_err(&format!("{command} is unavailable to the capture window"));
        assert!(
            is_acl_denial(&error),
            "{command}: expected an ACL refusal, got: {error}"
        );
        if !NEEDS_NATIVE_UI.contains(command)
            && let Err(error) = call(&main, command, args_for(command))
        {
            assert!(!is_acl_denial(&error), "{command}: {error}");
        }
    }
}

#[test]
fn capture_window_cannot_call_anything_but_its_own_commands() {
    let fixture = fixture();
    let capture = window(&fixture, "capture");
    // Foreign commands: the feed, chats, sending by chat_id, settings, the dialog, links, the main window draft, shortcuts,
    // the slice 3 actions.
    for command in MAIN_COMMANDS.iter().chain(SETTINGS_COMMANDS) {
        let error = call(&capture, command, args_for(command))
            .expect_err(&format!("{command} is unavailable to the capture window"));
        assert!(
            is_acl_denial(&error),
            "{command}: expected an ACL refusal, got: {error}"
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
        assert!(
            MAIN_COMMANDS.contains(&command) || SETTINGS_COMMANDS.contains(&command),
            "{command}"
        );
    }
    // Events and plugins: no subscriptions, no sending events, no windows, no links, no dialogs, no shortcuts.
    for cmd in [
        "plugin:event|listen",
        "plugin:event|unlisten",
        "plugin:event|emit",
        "plugin:event|emit_to",
        "plugin:app|version",
        "plugin:window|close",
        "plugin:window|hide",
        "plugin:window|show",
        // The page changes the window size and position only with its own `capture_resize` command (the height within narrow limits).
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
        // Only Rust writes the log: the page cannot clutter the log file.
        "plugin:log|log",
        "no_such_command",
    ] {
        let error = call(
            &capture,
            cmd,
            json!({"url": "https://example.com", "path": "/tmp", "event": ENTRIES_CHANGED}),
        )
        .expect_err(&format!(
            "{cmd} must not be available to the capture window"
        ));
        assert!(
            is_acl_denial(&error),
            "{cmd}: expected an ACL refusal, got: {error}"
        );
    }
}

#[test]
fn main_window_cannot_call_capture_commands() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    for command in CAPTURE_COMMANDS {
        let error = call(&main, command, args_for(command))
            .expect_err(&format!("{command} is unavailable to the main window"));
        assert!(
            is_acl_denial(&error),
            "{command}: expected an ACL refusal, got: {error}"
        );
    }
}

#[test]
fn main_window_cannot_call_unlisted_commands_or_plugin_commands() {
    let fixture = fixture();
    let window = window(&fixture, "main");
    // From `core` only subscribing and unsubscribing from an event are granted; the rest (sending events, windows, the app) is not.
    // The opener, dialog and global-shortcut plugins are attached for the Rust API, but their JS commands are not granted to the window: opening an address,
    // showing a dialog or taking a shortcut from JS is impossible, only through our commands, where everything is checked.
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
        // Cancelling a send was removed in slice 1 (a user decision); the command no longer exists.
        "undo_send",
        // There is no chat deletion (PRD §2.1: archive); "empty the trash" is absent too (user decision 4).
        "delete_chat",
        "purge_all",
        "empty_trash",
    ] {
        let error = call(
            &window,
            cmd,
            json!({"url": "https://example.com", "path": "/tmp"}),
        )
        .expect_err(&format!("{cmd} must not be available to the main window"));
        assert!(
            is_acl_denial(&error),
            "{cmd}: expected an ACL refusal, got: {error}"
        );
    }
}

#[test]
fn main_window_may_subscribe_to_its_events() {
    let fixture = fixture();
    let window = window(&fixture, "main");
    // Subscription to app events: entries changed, open a chat, database corrupt.
    for event in [
        ENTRIES_CHANGED,
        OPEN_CHAT,
        DATABASE_CORRUPT,
        SETTINGS_CHANGED,
        OPEN_MESSAGE,
    ] {
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
            .expect_err(&format!("{url:?} must be rejected"));
        assert_eq!(error, json!({"code": "link_not_allowed"}), "{url:?}");
    }
}
