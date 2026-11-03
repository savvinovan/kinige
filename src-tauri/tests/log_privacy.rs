//! Приватность журнала: ошибки команд пишутся в `log`, но ни адрес, ни id, ни текст записи в журнал не попадают.

// Корень интеграционных тестов целиком — тестовый код: `cfg(test)` даёт хелперам вне #[test] те же послабления
// clippy.toml (unwrap/expect/panic/индексация), что и тестам в src/.
#![cfg(test)]

mod common;

use common::*;
use log::{LevelFilter, Log, Metadata, Record};
use serde_json::json;
use std::sync::Mutex;

/// Уникальная метка, которая есть во всех «пользовательских» аргументах теста.
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

/// Логгер один на весь тестовый бинарник; оба теста идут в одном процессе и ставят его через `Once`.
fn install_logger() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        log::set_logger(&Capture).expect("логгер ставится один раз");
        log::set_max_level(LevelFilter::Trace);
    });
}

/// Сервер MCP (слайс 4a): тексты записей, поисковые запросы, id и имена чатов из вызовов агента в журнал процесса не
/// попадают, ни при успехе, ни при отказе, ни при мусоре на проводе.
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
        lines.iter().any(|line| line.contains("mcp: сокет поднят")),
        "the logger sees the server's lines: {lines:?}"
    );
    for line in &lines {
        assert!(
            !line.to_uppercase().contains(MARK),
            "в журнале процесса пользовательский ввод: {line}"
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
        call(&window, "open_external_url", json!({ "url": url })).expect_err("ссылка отвергнута");
    }
    call(
        &window,
        "send_message",
        json!({"chatId": format!("{MARK}-chat"), "body": format!("{MARK} текст записи")}),
    )
    .expect_err("нет такого чата");
    call(
        &window,
        "edit_message",
        json!({"id": format!("{MARK}-id"), "body": format!("{MARK} правка")}),
    )
    .expect_err("нет такой записи");

    let lines = CAPTURED.lock().unwrap().clone();
    assert!(
        lines.iter().any(|line| line.contains("ссылка отвергнута")),
        "отказ по ссылке должен попасть в журнал: {lines:?}"
    );
    assert!(
        lines.iter().any(|line| line.contains("ошибка хранилища")),
        "ошибка хранилища должна попасть в журнал: {lines:?}"
    );
    for line in &lines {
        assert!(
            !line.to_uppercase().contains(MARK),
            "в журнале пользовательский ввод: {line}"
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

/// Файл настроек (слайс 4b, ADR-0010): метка в неизвестном ключе, названии чата, недопустимом значении, пути папки и в
/// битом файле не попадает в журнал процесса ни при применении, ни при ошибке разбора, ни при небезопасных правах.
#[test]
fn the_settings_file_logs_only_categories_never_keys_titles_values_or_paths() {
    use kinige_store::SettingsFile;
    use tauri::Manager;
    install_logger();
    let fixture = fixture();
    let file = fixture.app.state::<SettingsFile>();
    let path = fixture.data.path().join("settings.json");
    let write = |text: String, mode: u32| {
        std::fs::write(&path, text).unwrap();
        // The file mode applies on Unix only; Windows ignores it (ADR-0010, Windows note).
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
        }
        #[cfg(not(unix))]
        let _ = mode;
        file.sync_now();
    };
    write(
        json!({
            format!("{MARK}-unknown-key"): 1,
            "shortcuts": {format!("{MARK}-nested"): 2, "capture": format!("{MARK}-bad-shortcut")},
            "text_size": format!("{MARK}-huge"),
            "mirror": {"folder": format!("/{MARK}/no/such/folder")},
            "agent_access": {
                "enabled": true,
                "mode": format!("{MARK}-mode"),
                "chats": [{"id": format!("{MARK}-id"), "title": format!("{MARK} название")}, {"title": format!("{MARK} другое")}],
            },
        })
        .to_string(),
        0o600,
    );
    // Небезопасные права, битый файл, не-UTF-8.
    write(
        json!({"agent_access": {"enabled": true}, format!("{MARK}-x"): 1}).to_string(),
        0o666,
    );
    write(format!("{{\"{MARK}\": "), 0o600);
    std::fs::write(&path, [0xFF, 0xFE, b'{']).unwrap();
    file.sync_now();
    let lines = CAPTURED.lock().unwrap().clone();
    assert!(
        lines.iter().any(|line| line.contains("файл настроек")),
        "события файла попадают в журнал категориями: {lines:?}"
    );
    for line in &lines {
        assert!(
            !line.to_uppercase().contains(MARK),
            "в журнале пользовательский ввод из файла настроек: {line}"
        );
        assert!(
            !line.contains("settings.json") || !line.contains('/'),
            "путь в журнале: {line}"
        );
    }
}
