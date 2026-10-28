//! Журнал обращений агентов (ADR-0009, слайс 4a): без содержимого, имя клиента, подключение, `purge`, ротация, миграция 5.

mod common;

use common::agent::*;
use common::*;
use kinige_domain::AgentMode;
use kinige_domain::agent::AgentTool;
use kinige_domain::groups::INBOX_ID;
use kinige_store::{LATEST_VERSION, LOG_MAX_ROWS, Store};
use serde_json::json;

const SECRET: &str = "СЕКРЕТНЫЙТЕКСТ7731";

#[test]
fn the_log_never_holds_entry_text_whatever_the_tool_and_arguments() {
    let a = agent(AgentMode::ReadWrite, true);
    let text = format!("{SECRET} заметка");
    call(
        &a.store,
        "add_entry",
        json!({"text": text, "task": true}),
        T0,
    )
    .unwrap();
    call(
        &a.store,
        "append_to_last_entry",
        json!({"text": format!("{SECRET} ещё")}),
        T0 + MINUTE,
    )
    .unwrap();
    for tool in AgentTool::ALL {
        let _ = call(
            &a.store,
            tool.name(),
            json!({"text": SECRET, "query": "заметка", "id": SECRET, "extra": SECRET}),
            T0,
        );
    }
    let conn = raw(a.dir.path());
    let mut statement = conn
        .prepare("SELECT client, tool, args_json, outcome FROM mcp_log")
        .unwrap();
    let dump: Vec<String> = statement
        .query_map([], |r| {
            Ok(format!(
                "{}|{}|{}|{}",
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?
            ))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert!(!dump.is_empty());
    assert!(dump.iter().all(|line| !line.contains(SECRET)), "{dump:?}");
}

#[test]
fn client_name_is_cut_and_cleaned_in_the_log() {
    let a = agent(AgentMode::Read, true);
    let name = format!("Evil\u{0}Client\u{1b}[31m\n{}", "x".repeat(200));
    call_as(&a.store, &name, "list_chats", json!({}), T0).unwrap();
    let rows = log_rows(a.dir.path());
    assert!(rows[0].0.chars().count() <= 64, "{}", rows[0].0);
    assert!(!rows[0].0.chars().any(char::is_control));
    assert!(rows[0].0.starts_with("EvilClient"));
}

#[test]
fn connection_and_disconnection_are_one_row_each() {
    let a = agent(AgentMode::Read, false);
    a.store
        .agent_connected("Claude Desktop", &moscow(T0))
        .unwrap();
    call_as(&a.store, "Claude Desktop", "list_chats", json!({}), T0).unwrap();
    a.store
        .agent_disconnected("Claude Desktop", &moscow(T0 + MINUTE))
        .unwrap();
    let rows = log_rows(a.dir.path());
    let tools: Vec<&str> = rows.iter().map(|r| r.1.as_str()).collect();
    assert_eq!(tools, ["connect", "list_chats", "disconnect"]);
    assert!(rows.iter().all(|r| r.0 == "Claude Desktop" && r.3 == "ok"));
}

#[test]
fn purging_an_entry_removes_its_log_traces_but_not_the_call_itself() {
    let a = agent(AgentMode::ReadWrite, true);
    let id =
        call(&a.store, "add_entry", json!({"text": "для удаления"}), T0).unwrap()["entry"]["id"]
            .as_str()
            .unwrap()
            .to_string();
    let other = call(
        &a.store,
        "add_entry",
        json!({"text": "остаётся"}),
        T0 + MINUTE,
    )
    .unwrap()["entry"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(count(a.dir.path(), "mcp_log_message"), 2);
    a.store
        .delete_message(&id, &moscow(T0 + 2 * MINUTE))
        .unwrap();
    a.store
        .purge_message(&id, &moscow(T0 + 3 * MINUTE))
        .unwrap();
    let rows: Vec<String> = raw(a.dir.path())
        .prepare("SELECT message_id FROM mcp_log_message")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(rows, [other]);
    assert_eq!(count(a.dir.path(), "mcp_log"), 2, "обращение остаётся");
    // Окно: удалённая навсегда запись — «удалена» (message = None), живая — запись.
    let page = a.store.agent_log_page(None).unwrap();
    let first_call = page
        .rows
        .iter()
        .find(|r| r.tool == "add_entry" && r.count == 0)
        .unwrap();
    assert!(a.store.agent_log_entries(first_call.id).unwrap().is_empty());
    let with_entry = page.rows.iter().find(|r| r.count == 1).unwrap();
    let entries = a.store.agent_log_entries(with_entry.id).unwrap();
    assert_eq!(entries[0].message.as_ref().unwrap().body_md, "остаётся");
    assert_eq!(entries[0].message.as_ref().unwrap().origin, "agent");
    assert!(entries[0].chat_is_inbox);
}

#[test]
fn log_page_is_newest_first_with_a_keyset_cursor_and_the_query_text() {
    let a = agent(AgentMode::Read, true);
    a.store.send_message(INBOX_ID, "Raft", &moscow(T0)).unwrap();
    for i in 0..35 {
        call(&a.store, "search", json!({"query": "Raft"}), T0 + i * 1000).unwrap();
    }
    let first = a.store.agent_log_page(None).unwrap();
    assert_eq!(first.rows.len(), kinige_store::LOG_PAGE);
    assert!(first.rows[0].id > first.rows[1].id);
    assert_eq!(first.rows[0].query.as_deref(), Some("Raft"));
    assert_eq!(first.rows[0].count, 1);
    let second = a.store.agent_log_page(first.older).unwrap();
    assert_eq!(second.rows.len(), 5);
    assert_eq!(second.older, None);
}

#[test]
fn rotation_drops_rows_older_than_90_days_and_beyond_20000_and_leaves_no_orphans() {
    let a = agent(AgentMode::Read, true);
    let conn = raw(a.dir.path());
    let now = T0;
    let day = 24 * 60 * MINUTE;
    // 20 010 свежих строк (новее по id), 5 строк старше 90 дней, у каждой по записи в mcp_log_message.
    let tx = conn.unchecked_transaction().unwrap();
    for i in 0..5 {
        tx.execute("INSERT INTO mcp_log (at, client, tool, args_json, outcome) VALUES (?1, 'c', 'search', '{}', 'ok')", [now - 91 * day - i]).unwrap();
        tx.execute(
            "INSERT INTO mcp_log_message (log_id, message_id) VALUES (last_insert_rowid(), ?1)",
            [format!("old-{i}")],
        )
        .unwrap();
    }
    for i in 0..(LOG_MAX_ROWS + 10) {
        tx.execute("INSERT INTO mcp_log (at, client, tool, args_json, outcome) VALUES (?1, 'c', 'search', '{}', 'ok')", [now - i]).unwrap();
        tx.execute(
            "INSERT INTO mcp_log_message (log_id, message_id) VALUES (last_insert_rowid(), ?1)",
            [format!("new-{i}")],
        )
        .unwrap();
    }
    tx.commit().unwrap();
    drop(conn);
    let removed = a.store.rotate_agent_log(&moscow(now)).unwrap();
    assert_eq!(removed, 5 + 10);
    assert_eq!(count(a.dir.path(), "mcp_log"), LOG_MAX_ROWS);
    let orphans: i64 = raw(a.dir.path())
        .query_row(
            "SELECT COUNT(*) FROM mcp_log_message WHERE log_id NOT IN (SELECT id FROM mcp_log)",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(orphans, 0);
    assert_eq!(count(a.dir.path(), "mcp_log_message"), LOG_MAX_ROWS);
}

#[test]
fn maintenance_rotates_the_log() {
    let a = agent(AgentMode::Read, true);
    raw(a.dir.path())
        .execute("INSERT INTO mcp_log (at, client, tool, args_json, outcome) VALUES (?1, 'c', 'search', '{}', 'ok')", [T0 - 100 * 24 * 60 * MINUTE])
        .unwrap();
    a.store.run_maintenance(&moscow(T0)).unwrap();
    assert_eq!(count(a.dir.path(), "mcp_log"), 0);
}

#[test]
fn migration_four_to_five_takes_a_copy_keeps_the_data_and_creates_the_log_tables() {
    let dir = tempfile::tempdir().unwrap();
    let id = {
        let store = open_without_mirror(dir.path());
        store
            .send_message(INBOX_ID, "до миграции 5", &moscow(T0))
            .unwrap()
            .id
    };
    {
        let conn = raw(dir.path());
        conn.execute_batch("DROP TABLE mcp_log; DROP TABLE mcp_log_message;")
            .unwrap();
        conn.pragma_update(None, "user_version", 4).unwrap();
    }
    let later = moscow(T0 + 10 * MINUTE);
    let mut options = options(dir.path());
    options.start_mirror = false;
    let store = Store::open_at(options, &later).unwrap();
    let version: i64 = raw(dir.path())
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, LATEST_VERSION);
    assert_eq!(LATEST_VERSION, 5);
    let copies: Vec<String> =
        std::fs::read_dir(db_path(dir.path()).parent().unwrap().join("backups"))
            .unwrap()
            .flatten()
            .map(|e| e.file_name().into_string().unwrap())
            .collect();
    assert_eq!(
        copies,
        [format!(
            "pre-migration-v4-{}.db",
            kinige_store::utc_stamp(later.ms)
        )]
    );
    assert_eq!(count(dir.path(), "mcp_log"), 0);
    let page = store.list_messages(INBOX_ID, None, 10, &later).unwrap();
    assert_eq!(page.messages[0].id, id);
    assert_eq!(page.messages[0].origin, "user");
}
