//! The two settings tools over a real MCP session (slice 5d, ADR-0022): the reply objects, every refusal, the event that reaches the
//! main and settings windows only, and rule 11 (no window is shown or focused).

// The whole root of the integration tests is test code: `cfg(test)` gives helpers outside #[test] the same relaxations
// of clippy.toml (unwrap/expect/panic/indexing) as the tests in src/.
#![cfg(test)]
// The MCP server is a Unix socket; there is none on Windows (slice 10a).
#![cfg(unix)]

mod common;

use common::mcp::*;
use kinige_domain::AgentMode;
use kinige_domain::groups::INBOX_ID;
use kinige_lib::settings_window::SETTINGS_CHANGED;
use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use tauri::Listener;

fn wait_for(what: &str, mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if condition() {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("did not wait for: {what}");
}

#[test]
fn a_text_size_change_tells_main_and_settings_only_and_shows_and_focuses_nothing() {
    let mcp = mcp(AgentMode::ReadWrite, &[INBOX_ID]);
    let main = common::window(&mcp.fixture, "main");
    let settings = common::window(&mcp.fixture, "settings");
    let capture = common::window(&mcp.fixture, "capture");
    let counters: Vec<Arc<AtomicUsize>> = [&main, &settings, &capture]
        .iter()
        .map(|window| {
            let counter = Arc::new(AtomicUsize::new(0));
            let seen = counter.clone();
            window.listen(SETTINGS_CHANGED, move |_| {
                seen.fetch_add(1, Ordering::SeqCst);
            });
            counter
        })
        .collect();
    let state = [&main, &settings, &capture]
        .map(|window| (window.is_visible().unwrap(), window.is_focused().unwrap()));
    let mut client = mcp.client();
    client.handshake("Claude Desktop");

    let (is_error, text) =
        client.call("set_setting", json!({"key": "text_size", "value": "large"}));
    assert!(!is_error, "{text}");
    let reply: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(reply["text_size"], "large");
    wait_for("the event in main and settings", || {
        counters[0].load(Ordering::SeqCst) >= 1 && counters[1].load(Ordering::SeqCst) >= 1
    });
    // The settings window's own reads see the change at once.
    assert_eq!(
        common::call(&main, "get_settings", json!({})).unwrap()["textSize"],
        "large"
    );
    // A refusal tells nobody and changes nothing.
    let told = counters[0].load(Ordering::SeqCst);
    let (is_error, refused) = client.call(
        "set_setting",
        json!({"key": "shortcuts.capture", "value": "Ctrl+Alt+KeyJ"}),
    );
    assert!(
        is_error && refused.starts_with("setting_not_writable"),
        "{refused}"
    );
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(counters[0].load(Ordering::SeqCst), told);
    assert_eq!(
        counters[2].load(Ordering::SeqCst),
        0,
        "the capture window is not told"
    );
    // Rule 11: nothing was shown or focused.
    let after = [&main, &settings, &capture]
        .map(|window| (window.is_visible().unwrap(), window.is_focused().unwrap()));
    assert_eq!(after, state);
}

#[test]
fn the_session_reads_settings_and_meets_every_refusal_with_its_code() {
    let mcp = mcp(AgentMode::ReadWrite, &[INBOX_ID]);
    let mut client = mcp.client();
    client.handshake("Claude Desktop");
    let (is_error, text) = client.call("get_settings", json!({}));
    assert!(!is_error, "{text}");
    let settings: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(settings["shortcuts"]["capture"]["value"], "Ctrl+Alt+KeyN");
    assert_eq!(
        settings["agent_access"]["visible_chat_ids"],
        json!([INBOX_ID])
    );
    for key in [
        "shortcuts.capture",
        "shortcuts.clipboard",
        "agent_access.enabled",
        "agent_access.mode",
        "agent_access.chats",
        "mirror.folder",
    ] {
        let (is_error, text) = client.call("set_setting", json!({"key": key, "value": "x"}));
        assert!(
            is_error && text.starts_with("setting_not_writable"),
            "{key}: {text}"
        );
    }
    for key in [
        "mcp_enabled",
        "mcp_mode",
        "mcp_chats",
        "mirror_dir",
        "shortcut_capture",
        "Text_Size",
        " text_size",
    ] {
        let (is_error, text) = client.call("set_setting", json!({"key": key, "value": "large"}));
        assert!(
            is_error && text.starts_with("invalid_argument"),
            "{key}: {text}"
        );
    }
    let (is_error, text) = client.call("set_setting", json!({"key": "text_size", "value": "huge"}));
    assert!(is_error && text.starts_with("invalid_argument"), "{text}");
    // Nothing the agent tried reached the settings.
    let (_, again) = client.call("get_settings", json!({}));
    assert_eq!(serde_json::from_str::<Value>(&again).unwrap(), settings);
    // "Read only": reads work, a change is refused.
    mcp.store().set_agent_mode(AgentMode::Read).unwrap();
    let (is_error, text) =
        client.call("set_setting", json!({"key": "text_size", "value": "large"}));
    assert!(is_error && text.starts_with("mcp_read_only"), "{text}");
    assert!(!client.call("get_settings", json!({})).0);
}
