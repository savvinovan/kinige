//! Сервер MCP в процессе приложения (ADR-0009, слайс 4a) по настоящим сокетам: рукопожатие, список инструментов по режиму,
//! вызовы, два клиента, выключенный доступ, единый `not_found`, предел кадра, мусор на проводе, «окна не всплывают».

mod common;

use common::mcp::*;
use kinige_domain::AgentMode;
use kinige_domain::groups::INBOX_ID;
use kinige_lib::capture::ENTRIES_CHANGED;
use kinige_lib::recovery::Recovery;
use kinige_store::Now;
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use tauri::{Listener, Manager};

const READ_TOOLS: [&str; 5] = [
    "list_chats",
    "search",
    "get_entries",
    "on_this_day",
    "get_entry",
];

fn log_tools(mcp: &Mcp) -> Vec<(String, String, String)> {
    let page = mcp.store().agent_log_page(None).unwrap();
    page.rows
        .into_iter()
        .rev()
        .map(|row| (row.client, row.tool, row.outcome))
        .collect()
}

fn wait_for(what: &str, mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if condition() {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("не дождались: {what}");
}

#[test]
fn handshake_lists_tools_by_mode_and_two_clients_work_at_once() {
    let mcp = mcp(AgentMode::Read, &[INBOX_ID]);
    let mut a = mcp.client();
    let mut b = mcp.client();
    let hello = a.handshake("Client A");
    assert_eq!(hello["result"]["serverInfo"]["name"], "kinige");
    assert!(
        hello["result"]["instructions"]
            .as_str()
            .unwrap()
            .contains("model provider")
    );
    b.handshake("Client B");
    assert_eq!(a.tool_names(), READ_TOOLS);
    let tools = a.request("tools/list", json!({}));
    assert!(
        tools["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .all(|t| t["annotations"]["readOnlyHint"] == true)
    );
    // Одновременные вызовы двух клиентов.
    let (ra, rb) = std::thread::scope(|scope| {
        let ha = scope.spawn(|| a.call("list_chats", json!({})));
        let hb = scope.spawn(|| b.call("list_chats", json!({})));
        (ha.join().unwrap(), hb.join().unwrap())
    });
    assert!(!ra.0 && !rb.0);
    // Режим «чтение и запись» — все восемь инструментов (после нового подключения).
    mcp.store().set_agent_mode(AgentMode::ReadWrite).unwrap();
    let mut c = mcp.client();
    c.handshake("Client C");
    assert_eq!(c.tool_names().len(), 8);
    // Журнал: по одной строке подключения на клиента.
    let connects = log_tools(&mcp)
        .into_iter()
        .filter(|(_, tool, _)| tool == "connect")
        .count();
    assert_eq!(connects, 3);
    drop((a, b, c));
    wait_for("три строки отключения", || {
        log_tools(&mcp)
            .iter()
            .filter(|(_, tool, _)| tool == "disconnect")
            .count()
            == 3
    });
}

#[test]
fn disabled_access_shows_only_the_tools_and_instructions_and_refuses_every_call() {
    let mcp = mcp(AgentMode::ReadWrite, &[INBOX_ID]);
    mcp.store()
        .send_message(INBOX_ID, "секретная запись", &Now::system())
        .unwrap();
    mcp.store().set_agent_enabled(false).unwrap();
    let mut client = mcp.client();
    let hello = client.handshake("Claude");
    assert!(hello["result"]["instructions"].is_string());
    assert_eq!(
        client.tool_names().len(),
        8,
        "список виден, данных в нём нет"
    );
    for (tool, arguments) in [
        ("list_chats", json!({})),
        ("search", json!({"query": "секретная"})),
        ("add_entry", json!({"text": "x"})),
    ] {
        let (is_error, text) = client.call(tool, arguments);
        assert!(
            is_error && text.starts_with("mcp_disabled"),
            "{tool}: {text}"
        );
        assert!(!text.contains("секретная"));
    }
    let count: i64 = rusqlite::Connection::open(mcp.fixture.data.path().join("kinige.db"))
        .unwrap()
        .query_row("SELECT COUNT(*) FROM message", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn a_write_works_end_to_end_notifies_the_window_and_never_shows_or_focuses_it() {
    let mcp = mcp(AgentMode::ReadWrite, &[INBOX_ID]);
    let window = common::window(&mcp.fixture, "main");
    let visible_before = window.is_visible().unwrap();
    let focused_before = window.is_focused().unwrap();
    let events = Arc::new(AtomicUsize::new(0));
    let counter = events.clone();
    window.listen(ENTRIES_CHANGED, move |_| {
        counter.fetch_add(1, Ordering::SeqCst);
    });
    let mut client = mcp.client();
    client.handshake("Claude Desktop");
    let (is_error, text) = client.call("add_entry", json!({"text": "Итоги встречи", "task": true}));
    assert!(!is_error, "{text}");
    let reply: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(reply["entry"]["origin"], "agent");
    let id = reply["entry"]["id"].as_str().unwrap();
    wait_for(
        "событие «записи изменились»",
        || events.load(Ordering::SeqCst) >= 1,
    );
    let (_, done) = client.call("set_task_done", json!({"id": id, "done": true}));
    assert!(done.contains("\"done\":true"), "{done}");
    let (error, refused) = client.call(
        "append_to_last_entry",
        json!({"text": "ещё", "chat_id": "0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d"}),
    );
    assert!(error && refused.starts_with("not_found"));
    // Окно не вышло вперёд и не получило фокус.
    assert_eq!(window.is_visible().unwrap(), visible_before);
    assert_eq!(window.is_focused().unwrap(), focused_before);
    let rows = log_tools(&mcp);
    assert!(
        rows.iter()
            .any(|(client, tool, outcome)| client == "Claude Desktop"
                && tool == "add_entry"
                && outcome == "ok")
    );
}

#[test]
fn not_found_is_the_same_text_for_closed_excluded_deleted_and_missing() {
    let mcp = mcp(AgentMode::ReadWrite, &[INBOX_ID]);
    let store = mcp.store();
    let now = Now::system();
    let secret = store.create_chat("Секретный", None, &now).unwrap().id;
    let in_secret = store.send_message(&secret, "закрыто", &now).unwrap().id;
    let trashed = store.send_message(INBOX_ID, "в корзине", &now).unwrap().id;
    store.delete_message(&trashed, &now).unwrap();
    let mut client = mcp.client();
    client.handshake("Claude");
    let missing = "0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d";
    let mut texts = Vec::new();
    for (tool, arguments) in [
        ("get_entry", json!({"id": in_secret})),
        ("get_entry", json!({"id": trashed})),
        ("get_entry", json!({"id": missing})),
        ("search", json!({"query": "закрыто", "chat_id": secret})),
        (
            "get_entries",
            json!({"date": "2026-10-21", "chat_id": missing}),
        ),
        ("add_entry", json!({"text": "x", "chat_id": secret})),
        (
            "append_to_last_entry",
            json!({"text": "x", "chat_id": missing}),
        ),
        ("set_task_done", json!({"id": in_secret, "done": true})),
    ] {
        let (is_error, text) = client.call(tool, arguments);
        assert!(is_error, "{tool}");
        texts.push(text);
    }
    assert!(
        texts
            .iter()
            .all(|text| text == &texts[0] && text.starts_with("not_found")),
        "{texts:?}"
    );
}

#[test]
fn a_frame_over_4_mib_closes_the_connection_and_the_server_lives_on() {
    let mcp = mcp(AgentMode::Read, &[INBOX_ID]);
    let mut client = mcp.client();
    client.handshake("Big");
    let huge = format!(
        r#"{{"jsonrpc":"2.0","id":99,"method":"tools/call","params":{{"name":"{}"}}}}"#,
        "x".repeat(4 * 1024 * 1024 + 100)
    );
    // Сервер закрывает соединение, не дочитав кадр: запись может оборваться, чтение — закончиться RST.
    client.send_raw_lossy(huge.as_bytes());
    assert!(client.closed_by_server());
    let mut again = mcp.client();
    again.handshake("Next");
    assert!(!again.call("list_chats", json!({})).0);
}

#[test]
fn garbage_on_the_wire_never_breaks_the_server_or_writes_anything() {
    let mcp = mcp(AgentMode::ReadWrite, &[INBOX_ID]);
    let mut client = mcp.client();
    client.send_raw(b"\xff\xfe\x00 not json at all");
    client.send_raw(br#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":"#);
    client.send_raw(br#"{"jsonrpc":"2.0","id":2,"method":"initialize","params":{"clientInfo":5}}"#);
    client.send_raw(b"[1,2,3]");
    client.send_raw(b"{}");
    // После мусора рукопожатие и вызов работают на том же соединении или на новом.
    let mut fresh = mcp.client();
    fresh.handshake("Fine");
    for (tool, arguments) in [
        ("add_entry", json!({"text": 5})),
        ("add_entry", json!({})),
        ("search", json!({"query": ["a"]})),
        ("get_entry", json!({"id": "../../etc/passwd"})),
        ("set_task_done", json!({"id": "x", "done": "yes"})),
        ("no_such_tool", json!({})),
    ] {
        let (is_error, text) = fresh.call(tool, arguments);
        assert!(
            is_error && (text.starts_with("invalid_argument") || text.contains("tool")),
            "{tool}: {text}"
        );
    }
    assert_eq!(
        mcp.store()
            .list_messages(INBOX_ID, None, 10, &Now::system())
            .unwrap()
            .messages
            .len(),
        0
    );
}

#[test]
fn journal_unavailable_when_corruption_was_found() {
    let mcp = mcp(AgentMode::Read, &[INBOX_ID]);
    let mut client = mcp.client();
    client.handshake("Claude");
    assert!(!client.call("list_chats", json!({})).0);
    mcp.fixture
        .app
        .state::<Recovery>()
        .corruption_flag()
        .store(true, Ordering::SeqCst);
    let (is_error, text) = client.call("list_chats", json!({}));
    assert!(
        is_error && text.starts_with("journal_unavailable"),
        "{text}"
    );
    // Список инструментов остаётся (данных в нём нет).
    assert_eq!(client.tool_names().len(), 5);
}

#[test]
fn a_connected_client_holds_no_store_clone() {
    let mcp = mcp(AgentMode::ReadWrite, &[INBOX_ID]);
    let baseline = mcp.store().handle_count();
    let mut client = mcp.client();
    client.handshake("Claude");
    client.call("add_entry", json!({"text": "после вызова"}));
    client.call("list_chats", json!({}));
    assert_eq!(
        mcp.store().handle_count(),
        baseline,
        "ни слушатель, ни соединение, ни задачи вызовов не держат клон Store"
    );
}

#[test]
fn the_reconnect_marker_from_the_bridge_is_its_own_log_row_not_a_second_connect() {
    let mcp = mcp(AgentMode::Read, &[INBOX_ID]);
    let mut plain = mcp.client();
    plain.handshake("Claude");
    let mut replayed = mcp.client();
    replayed.send_raw(br#"{"jsonrpc":"2.0","method":"kinige/reconnect"}"#);
    replayed.handshake("Claude");
    assert!(!replayed.call("list_chats", json!({})).0);
    let count = |tool: &str| log_tools(&mcp).iter().filter(|(_, t, _)| t == tool).count();
    assert_eq!(
        count("connect"),
        1,
        "повторный initialize от моста не «подключение»"
    );
    assert_eq!(
        count("reconnect"),
        1,
        "немых путей нет: переподключение — строка журнала"
    );
    drop((plain, replayed));
    wait_for("отключения обоих соединений", || count("disconnect") >= 2);
}

/// Маркер приходит частями (мост пишет его одним `write`, но сокет не обещает этого): всё равно «переподключение».
#[test]
fn the_reconnect_marker_arriving_in_pieces_is_still_recognised() {
    let mcp = mcp(AgentMode::Read, &[INBOX_ID]);
    let mut client = mcp.client();
    let marker = br#"{"jsonrpc":"2.0","method":"kinige/reconnect"}"#;
    client.send_partial(&marker[..11]);
    std::thread::sleep(Duration::from_millis(150));
    client.send_partial(&marker[11..30]);
    std::thread::sleep(Duration::from_millis(150));
    client.send_raw(&marker[30..]);
    client.handshake("Piecewise");
    assert!(!client.call("list_chats", json!({})).0);
    assert!(
        log_tools(&mcp)
            .iter()
            .any(|(_, tool, _)| tool == "reconnect")
    );
    assert!(!log_tools(&mcp).iter().any(|(_, tool, _)| tool == "connect"));
}

/// Обычное первое сообщение, которое только начинается как маркер, маркером не считается и не теряется.
#[test]
fn a_first_message_that_only_starts_like_the_marker_is_served_normally() {
    let mcp = mcp(AgentMode::Read, &[INBOX_ID]);
    let mut client = mcp.client();
    let hello = client.handshake("Normal");
    assert_eq!(hello["result"]["serverInfo"]["name"], "kinige");
    assert!(log_tools(&mcp).iter().any(|(_, tool, _)| tool == "connect"));
}

/// Встроенный жизненный цикл `rmcp` (`_meta.protocolVersion` в самом запросе) пропускает `tools/call` и `tools/list` мимо
/// `initialize`, причём не только для версии 2026-07-28, но и для 2025-11-25. Такой сеанс шёл бы без «подключения» и без
/// имени клиента, а `add_entry` записал бы данные. Сервер сам требует `initialize`: вызов не выполняется, в базе и журнале
/// ничего нет, сеанс не начат.
#[test]
fn a_request_that_skips_initialize_by_inline_version_is_refused_and_leaves_no_trace() {
    let mcp = mcp(AgentMode::ReadWrite, &[INBOX_ID]);
    for version in ["2026-07-28", "2025-11-25"] {
        let mut client = mcp.client();
        let before = log_tools(&mcp).len();
        let meta = json!({
            "io.modelcontextprotocol/protocolVersion": version,
            "io.modelcontextprotocol/clientCapabilities": {},
            "io.modelcontextprotocol/clientInfo": {"name": "Sneaky", "version": "1"}
        });
        let call = client.request(
            "tools/call",
            json!({"name": "add_entry", "arguments": {"text": "в обход рукопожатия"}, "_meta": meta}),
        );
        assert!(call["error"].is_object(), "{version}: {call}");
        assert!(call["result"].is_null(), "{version}: {call}");
        let list = client.request("tools/list", json!({"_meta": meta}));
        assert!(list["error"].is_object(), "{version}: {list}");
        assert_eq!(
            log_tools(&mcp).len(),
            before,
            "{version}: ни подключения, ни вызова в журнале"
        );
        assert_eq!(
            mcp.store()
                .list_messages(INBOX_ID, None, 10, &Now::system())
                .unwrap()
                .messages
                .len(),
            0,
            "{version}: запись не создана"
        );
        // Обычное рукопожатие после этого работает.
        client.handshake("Honest");
        assert!(!client.call("list_chats", json!({})).0);
        // Закрытое соединение пишет «отключение» позже: дожидаемся, чтобы оно не попало в счёт следующего круга.
        drop(client);
        wait_for("строка отключения", || {
            log_tools(&mcp)
                .last()
                .is_some_and(|row| row.1 == "disconnect")
        });
    }
}

/// Если строку подключения записать не удалось (база в восстановлении), она пишется перед первым вызовом.
#[test]
fn a_connection_row_that_failed_at_initialize_is_written_before_the_first_call() {
    let mcp = mcp(AgentMode::Read, &[INBOX_ID]);
    let flag = mcp.fixture.app.state::<Recovery>().corruption_flag();
    flag.store(true, Ordering::SeqCst);
    let mut client = mcp.client();
    client.handshake("Late");
    flag.store(false, Ordering::SeqCst);
    assert!(
        log_tools(&mcp).is_empty(),
        "пока база недоступна, строки нет"
    );
    assert!(!client.call("list_chats", json!({})).0);
    let rows = log_tools(&mcp);
    let tools: Vec<&str> = rows.iter().map(|(_, tool, _)| tool.as_str()).collect();
    assert_eq!(tools, ["connect", "list_chats"]);
    assert_eq!(rows[0].0, "Late");
}
