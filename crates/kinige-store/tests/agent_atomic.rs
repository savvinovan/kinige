//! Запись агента и строка журнала — одна транзакция (ADR-0009, слайс 4a): откат при сбое журнала, `abort` после фиксации.

mod common;

use common::agent::*;
use common::*;
use kinige_domain::AgentMode;
use kinige_domain::groups::INBOX_ID;
use kinige_store::AgentError;
use serde_json::json;
use std::process::Command;

const CHILD_ENV: &str = "KINIGE_AGENT_ABORT_CHILD_DIR";

fn entry_id(reply: &serde_json::Value) -> String {
    reply["entry"]["id"].as_str().unwrap().to_string()
}

/// Ловушка, которая роняет любую вставку в `mcp_log`: проверка, что запись и журнал — одна транзакция.
fn break_the_log(dir: &std::path::Path) {
    raw(dir)
        .execute_batch(
            "CREATE TRIGGER mcp_log_broken BEFORE INSERT ON mcp_log BEGIN SELECT RAISE(ABORT, 'журнал сломан'); END;",
        )
        .unwrap();
}

#[test]
fn a_failing_log_row_rolls_the_write_back() {
    let a = agent(AgentMode::ReadWrite, true);
    let first = entry_id(&call(&a.store, "add_entry", json!({"text": "первая"}), T0).unwrap());
    break_the_log(a.dir.path());
    let revisions = count(a.dir.path(), "message_revision");
    assert!(
        call(
            &a.store,
            "add_entry",
            json!({"text": "вторая"}),
            T0 + MINUTE
        )
        .is_err()
    );
    assert!(
        call(
            &a.store,
            "append_to_last_entry",
            json!({"text": "ещё"}),
            T0 + MINUTE
        )
        .is_err()
    );
    let task = entry_id(&{
        raw(a.dir.path())
            .execute("DROP TRIGGER mcp_log_broken", [])
            .unwrap();
        let t = call(
            &a.store,
            "add_entry",
            json!({"text": "задача", "task": true}),
            T0 + 2 * MINUTE,
        )
        .unwrap();
        break_the_log(a.dir.path());
        t
    });
    assert!(
        call(
            &a.store,
            "set_task_done",
            json!({"id": task, "done": true}),
            T0 + 3 * MINUTE
        )
        .is_err()
    );
    assert_eq!(count(a.dir.path(), "message"), 2, "новых записей нет");
    assert_eq!(
        count(a.dir.path(), "message_revision"),
        revisions,
        "ревизий нет"
    );
    let body: String = raw(a.dir.path())
        .query_row("SELECT body_md FROM message WHERE id = ?1", [&first], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(body, "первая");
    let done: Option<i64> = raw(a.dir.path())
        .query_row(
            "SELECT task_done_at FROM message WHERE id = ?1",
            [&task],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(done, None);
}

#[test]
fn a_failing_log_commit_on_a_read_gives_the_agent_an_error_and_no_data() {
    let a = agent(AgentMode::ReadWrite, true);
    a.store
        .send_message(INBOX_ID, "видимая запись", &moscow(T0))
        .unwrap();
    break_the_log(a.dir.path());
    for (tool, arguments) in [
        ("search", json!({"query": "видимая"})),
        ("get_entries", json!({"date": "2026-10-21"})),
        ("list_chats", json!({})),
    ] {
        let result = call(&a.store, tool, arguments, T0);
        assert!(
            matches!(result, Err(AgentError::Internal | AgentError::Unavailable)),
            "{tool}"
        );
    }
}

#[test]
fn abort_right_after_a_write_keeps_the_entry_and_its_log_row() {
    if let Ok(dir) = std::env::var(CHILD_ENV) {
        let store = open_without_mirror(dir.as_ref());
        store.set_agent_chats(vec![INBOX_ID.to_string()]).unwrap();
        store.set_agent_mode(AgentMode::ReadWrite).unwrap();
        store.set_agent_enabled(true).unwrap();
        call(&store, "add_entry", json!({"text": "переживёт abort"}), T0).unwrap();
        std::process::abort();
    }
    let dir = tempfile::tempdir().unwrap();
    let status = Command::new(std::env::current_exe().unwrap())
        .args([
            "abort_right_after_a_write_keeps_the_entry_and_its_log_row",
            "--exact",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD_ENV, dir.path())
        .status()
        .unwrap();
    assert!(!status.success());
    let store = open(dir.path());
    let page = store
        .list_messages(INBOX_ID, None, 50, &moscow(T0 + MINUTE))
        .unwrap();
    assert_eq!(page.messages.len(), 1);
    assert_eq!(page.messages[0].origin, "agent");
    let rows = log_rows(dir.path());
    assert_eq!(rows.len(), 1);
    assert_eq!(
        (rows[0].1.as_str(), rows[0].3.as_str(), rows[0].4),
        ("add_entry", "ok", 1)
    );
    // Markdown-копия дописана при следующем старте.
    let file = mirror_dir(dir.path())
        .join("Inbox")
        .join("2026")
        .join("2026-10-21.md");
    wait_until("файл копии", || {
        std::fs::read_to_string(&file).is_ok_and(|text| text.contains("переживёт abort"))
    });
}
