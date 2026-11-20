//! Log privacy: command errors are written to `log`, but neither the address, nor the id, nor the entry text gets into the log.

// The whole root of the integration tests is test code: `cfg(test)` gives helpers outside #[test] the same relaxations
// of clippy.toml (unwrap/expect/panic/indexing) as the tests in src/.
#![cfg(test)]

mod common;

use common::*;
use log::{LevelFilter, Log, Metadata, Record};
use serde_json::json;
use std::sync::Mutex;

/// A unique mark present in all the "user" arguments of the test.
const MARK: &str = "ZEBRAUNIQ";

static CAPTURED: Mutex<Vec<String>> = Mutex::new(Vec::new());

struct Capture;

impl Log for Capture {
    fn enabled(&self, _: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        CAPTURED
            .lock()
            .unwrap()
            .push(format!("[{}] {}", record.level(), record.args()));
    }

    fn flush(&self) {}
}

/// There is one logger for the whole test binary; the tests run in one process and install it through `Once`.
fn install_logger() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        log::set_logger(&Capture).expect("the logger is installed once");
        log::set_max_level(LevelFilter::Trace);
    });
}

/// The MCP server (slice 4a): entry texts, search queries, ids and chat names from agent calls do not get into the process
/// log, neither on success, nor on refusal, nor on garbage on the wire.
#[cfg(unix)]
#[test]
fn the_mcp_server_logs_no_texts_queries_or_arguments() {
    use common::mcp::mcp;
    use kinige_domain::AgentMode;
    use kinige_domain::groups::INBOX_ID;
    install_logger();
    let mcp = mcp(AgentMode::ReadWrite, &[INBOX_ID]);
    let mut client = mcp.client();
    client.handshake(&format!("{MARK}-client"));
    client.call("add_entry", json!({"text": format!("{MARK} текст записи")}));
    client.call("search", json!({"query": format!("{MARK} запрос")}));
    client.call("get_entry", json!({"id": format!("{MARK}-id")}));
    client.call("add_entry", json!({"text": format!("{MARK} в чужой чат"), "chat_id": "0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d"}));
    client.send_raw(format!("{MARK} \u{0}мусор").as_bytes());
    drop(client);
    // The server handles the garbage and the close on its own task; its last step is the `disconnect` row in the access
    // log. Wait for that row, otherwise the check below may run before the server has logged anything.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !mcp
        .store()
        .agent_log_page(None)
        .unwrap()
        .rows
        .iter()
        .any(|row| row.tool == "disconnect")
    {
        assert!(
            std::time::Instant::now() < deadline,
            "the server did not finish the connection"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let lines = CAPTURED.lock().unwrap().clone();
    // The capture is live: the listener announced itself through `log` when it started.
    assert!(
        lines.iter().any(|line| line.contains("mcp: socket is up")),
        "the logger sees the server's lines: {lines:?}"
    );
    for line in &lines {
        assert!(
            !line.to_uppercase().contains(MARK),
            "user input in the process log: {line}"
        );
    }
}

#[test]
fn failed_commands_log_only_categories_never_user_input() {
    install_logger();
    let fixture = fixture();
    let window = window(&fixture, "main");

    for url in [
        format!("javascript:{MARK}"),
        format!(" https://{MARK}.example/"),
        format!("{MARK}/relative"),
    ] {
        call(&window, "open_external_url", json!({ "url": url })).expect_err("link rejected");
    }
    call(
        &window,
        "send_message",
        json!({"chatId": format!("{MARK}-chat"), "body": format!("{MARK} текст записи")}),
    )
    .expect_err("no such chat");
    call(
        &window,
        "edit_message",
        json!({"id": format!("{MARK}-id"), "body": format!("{MARK} правка")}),
    )
    .expect_err("no such entry");

    let lines = CAPTURED.lock().unwrap().clone();
    assert!(
        lines.iter().any(|line| line.contains("link rejected")),
        "the link refusal must get into the log: {lines:?}"
    );
    assert!(
        lines.iter().any(|line| line.contains("store error")),
        "the store error must get into the log: {lines:?}"
    );
    for line in &lines {
        assert!(
            !line.to_uppercase().contains(MARK),
            "user input in the log: {line}"
        );
    }
}

/// The forest (slice 6): dates, excerpts and chat ids of the year table and of the day target, with success and with
/// refusal, stay out of the process log.
#[test]
fn the_forest_commands_log_no_dates_excerpts_or_chat_ids() {
    install_logger();
    let fixture = fixture();
    let window = window(&fixture, "main");
    let sent = call(
        &window,
        "send_message",
        json!({"chatId": kinige_domain::groups::INBOX_ID, "body": format!("{MARK} text of the first entry")}),
    )
    .unwrap();
    let today = sent["journalDate"].as_str().unwrap();
    let year: i64 = today.split('-').next().unwrap().parse().unwrap();
    let table = call(
        &window,
        "get_forest_year",
        json!({"year": year, "firstWeekday": "monday", "weekdaySource": "fallback"}),
    )
    .unwrap();
    assert!(
        table.to_string().contains(MARK),
        "the excerpt reaches the window"
    );
    call(
        &window,
        "get_forest_day_target",
        json!({"date": today, "currentChatId": format!("{MARK}-chat")}),
    )
    .unwrap();
    call(
        &window,
        "get_forest_day_target",
        json!({"date": format!("{MARK}-date"), "currentChatId": null}),
    )
    .expect_err("not a date");
    call(
        &window,
        "get_forest_year",
        json!({"year": 0, "firstWeekday": "monday"}),
    )
    .expect_err("not a year");
    call(
        &window,
        "get_forest_year",
        json!({"year": year, "firstWeekday": format!("{MARK}-day")}),
    )
    .expect_err("not a weekday");
    call(
        &window,
        "set_forest_view",
        json!({"view": format!("{MARK}-view"), "mode": "forest"}),
    )
    .expect_err("not a view");

    let lines = CAPTURED.lock().unwrap().clone();
    // The debug line about the first weekday is among the captured lines, so that the checks below look at it too.
    assert!(
        lines
            .iter()
            .any(|line| line.contains("forest: first weekday")),
        "the weekday line was not captured: {lines:?}"
    );
    for line in &lines {
        assert!(
            !line.to_uppercase().contains(MARK),
            "user input in the log: {line}"
        );
        assert!(!line.contains(today), "a date in the log: {line}");
        assert!(
            !line.contains(kinige_domain::groups::INBOX_ID),
            "a chat id in the log: {line}"
        );
    }
}

/// Attachment details (slice 7b): a name, a description and arguments that do not read, on success and on every refusal, stay out of the
/// process log. serde's error text quotes an unexpected string, so no deserialization error may reach the log with its content.
#[test]
fn the_details_commands_log_no_names_descriptions_or_rejected_arguments() {
    use kinige_store::{Now, Store};
    use tauri::Manager;
    install_logger();
    let fixture = fixture();
    let window = window(&fixture, "main");
    let tile = fixture
        .app
        .state::<Store>()
        .attach_bytes(
            kinige_domain::groups::INBOX_ID,
            Some("scan.pdf"),
            b"%PDF-1.4",
            &Now::system(),
        )
        .unwrap()
        .id;
    for command in ["set_draft_attachment_details", "set_attachment_details"] {
        for changes in [
            json!({"name": format!("{MARK}-name"), "description": format!("{MARK} описание")}),
            json!({"name": format!("{MARK}/slash")}),
            json!({"description": format!("{MARK}\u{0}nul")}),
            json!({"name": format!("{MARK}\u{202e}bidi")}),
            json!({"description": MARK.repeat(1000)}),
            json!({"spoiler": format!("{MARK}-string")}),
            json!({"spoiler": null, MARK: format!("{MARK}-value")}),
            json!({"name": [MARK], "unknownkey": MARK}),
        ] {
            let _ = call(
                &window,
                command,
                json!({"attachmentId": tile, "changes": changes}),
            );
        }
        let _ = call(
            &window,
            command,
            json!({"attachmentId": format!("{MARK}-id"), "changes": {"name": format!("{MARK}-x")}}),
        );
    }
    let lines = CAPTURED.lock().unwrap().clone();
    for line in &lines {
        assert!(
            !line.to_uppercase().contains(MARK),
            "attachment details in the log: {line}"
        );
    }
}

/// The settings tools (slice 5d, ADR-0022): a mark in the key, the value, a copy folder path and a leftover file name does not get
/// into the process log, neither on a refusal, nor on a change, nor in the start-up clean-up (which logs a count only).
#[test]
fn the_settings_tools_log_only_categories_never_keys_values_or_paths() {
    use kinige_domain::AgentMode;
    use kinige_store::{AgentRequest, Now, Store};
    use tauri::Manager;
    install_logger();
    let fixture = fixture();
    let store = fixture.app.state::<Store>();
    store.set_agent_mode(AgentMode::ReadWrite).unwrap();
    store.set_agent_enabled(true).unwrap();
    let folder = fixture.data.path().join(format!("{MARK}-copy"));
    std::fs::create_dir(&folder).unwrap();
    store.set_mirror_dir(&folder).unwrap();
    std::fs::write(
        fixture
            .data
            .path()
            .join(".settings.json.0123456789abcdef0123456789abcdef.tmp"),
        b"x",
    )
    .unwrap();
    assert_eq!(store.remove_settings_file_leftovers(), 1);
    let call = |tool: &str, arguments: serde_json::Value| {
        let arguments = arguments.as_object().cloned();
        let _ = store.agent_call(
            &AgentRequest {
                client: "Probe",
                tool,
                arguments: arguments.as_ref(),
                inbox_title: "Inbox",
            },
            &Now::system(),
        );
    };
    call("get_settings", json!({}));
    call(
        "set_setting",
        json!({"key": format!("{MARK}-key"), "value": format!("{MARK}-value")}),
    );
    call(
        "set_setting",
        json!({"key": "text_size", "value": format!("{MARK}-huge")}),
    );
    call(
        "set_setting",
        json!({"key": "mirror.folder", "value": format!("/{MARK}/folder")}),
    );
    call(
        "set_setting",
        json!({"key": "shortcuts.capture", "value": format!("{MARK}-accelerator")}),
    );
    call("set_setting", json!({"key": "text_size", "value": "large"}));
    let lines = CAPTURED.lock().unwrap().clone();
    for line in &lines {
        assert!(
            !line.to_uppercase().contains(MARK),
            "user input in the log: {line}"
        );
        assert!(
            !line.contains("settings.json.0123"),
            "a file name in the log: {line}"
        );
    }
    assert!(
        lines
            .iter()
            .any(|line| line.contains("removed 1 leftover temporary files")),
        "the clean-up logs its count: {lines:?}"
    );
}

/// Voice notes (slice 7a): the raw-body command refuses garbage in every header, and none of the user's values (chat id, reply target,
/// duration text) reaches the log; the refusal itself does.
#[test]
fn send_voice_logs_only_categories_never_headers() {
    install_logger();
    let fixture = fixture();
    let window = window(&fixture, "main");
    let inbox = kinige_domain::groups::INBOX_ID;

    for headers in [
        vec![
            ("x-chat-id", format!("{MARK}-chat")),
            ("x-duration-ms", "40000".to_string()),
        ],
        vec![
            ("x-chat-id", inbox.to_string()),
            ("x-duration-ms", format!("{MARK}ms")),
        ],
        vec![
            ("x-chat-id", inbox.to_string()),
            ("x-duration-ms", "40000".to_string()),
            ("x-reply-to", format!("{MARK}-reply")),
        ],
    ] {
        let headers: Vec<(&str, &str)> = headers.iter().map(|(k, v)| (*k, v.as_str())).collect();
        let body = common::voice::m4a(256);
        common::voice::raw_call(&window, "send_voice", &headers, body)
            .expect_err("the note is refused");
    }

    let lines = CAPTURED.lock().unwrap().clone();
    assert!(
        lines.iter().any(|line| line.contains("store error")),
        "the refusal must get into the log: {lines:?}"
    );
    for line in &lines {
        assert!(
            !line.to_uppercase().contains(MARK),
            "user input in the log: {line}"
        );
    }
}
