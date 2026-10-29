//! Запись агента (ADR-0009, слайс 4a): байты записи, дописывание с ревизией `edit`, задачи.

mod common;

use common::agent::*;
use common::ops::{groups, hlc_entries};
use common::*;
use kinige_domain::AgentMode;
use kinige_domain::groups::INBOX_ID;
use kinige_store::AgentError;
use serde_json::json;

fn entry_id(reply: &serde_json::Value) -> String {
    reply["entry"]["id"].as_str().unwrap().to_string()
}

#[test]
fn add_entry_writes_an_agent_record_with_exact_group_bytes_and_one_log_row() {
    let a = agent(AgentMode::ReadWrite, true);
    let reply = call(&a.store, "add_entry", json!({"text": "Итоги встречи"}), T0).unwrap();
    assert_eq!(reply["action"], "created");
    assert_eq!(reply["entry"]["origin"], "agent");
    assert_eq!(reply["entry"]["chat"], "Inbox");
    assert_eq!(reply["entry"]["created"], "2026-10-21T09:12:00+03:00");
    let id = entry_id(&reply);
    let g = groups(a.dir.path(), "message", &id);
    assert_eq!(
        g["core"],
        format!(
            r#"{{"created_at":{T0},"created_tz":"Europe/Moscow","origin":"agent","reply_to_id":null}}"#
        )
    );
    assert_eq!(
        g["flags"],
        r#"{"favorite_at":null,"is_task":false,"pinned_at":null,"task_done_at":null}"#
    );
    // Пять групп под одним HLC, sv = 1.
    let hlcs = hlc_entries(a.dir.path(), "message", &id);
    assert_eq!(hlcs.len(), 5);
    assert_eq!(
        hlcs.values()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        1
    );
    let rows = log_rows(a.dir.path());
    assert_eq!(rows.len(), 1);
    assert_eq!(
        (rows[0].1.as_str(), rows[0].3.as_str(), rows[0].4),
        ("add_entry", "ok", 1)
    );
    assert_eq!(rows[0].2, "{}");
    // Лента видит запись с пометкой происхождения.
    let page = a
        .store
        .list_messages(INBOX_ID, None, 50, &moscow(T0))
        .unwrap();
    assert_eq!(page.messages[0].origin, "agent");
}

#[test]
fn add_entry_as_task_creates_the_flags_group_at_once() {
    let a = agent(AgentMode::ReadWrite, true);
    let reply = call(
        &a.store,
        "add_entry",
        json!({"text": "купить билеты", "task": true}),
        T0,
    )
    .unwrap();
    let id = entry_id(&reply);
    assert_eq!(
        groups(a.dir.path(), "message", &id)["flags"],
        r#"{"favorite_at":null,"is_task":true,"pinned_at":null,"task_done_at":null}"#
    );
    assert_eq!(reply["entry"]["task"], true);
    assert_eq!(hlc_entries(a.dir.path(), "message", &id).len(), 5);
    let raw_args: String = raw(a.dir.path())
        .query_row("SELECT args_json FROM mcp_log", [], |r| r.get(0))
        .unwrap();
    assert_eq!(raw_args, r#"{"task":true}"#);
}

#[test]
fn add_entry_goes_to_the_marked_chat_and_the_markdown_copy_gets_it() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path());
    let work = store.create_chat("Работа", None, &moscow(T0)).unwrap().id;
    store.set_agent_chats(vec![work.clone()]).unwrap();
    store.set_agent_mode(AgentMode::ReadWrite).unwrap();
    store.set_agent_enabled(true).unwrap();
    call(
        &store,
        "add_entry",
        json!({"text": "запись агента в работе", "chat_id": work}),
        T0,
    )
    .unwrap();
    let file = mirror_dir(dir.path())
        .join("Работа")
        .join("2026")
        .join("2026-10-21.md");
    wait_until("файл копии с записью агента", || {
        std::fs::read_to_string(&file).is_ok_and(|text| text.contains("запись агента в работе"))
    });
}

#[test]
fn append_goes_to_the_agent_entry_within_ten_minutes_and_saves_an_edit_revision() {
    let a = agent(AgentMode::ReadWrite, true);
    let first = call(&a.store, "add_entry", json!({"text": "Пункт один"}), T0).unwrap();
    let id = entry_id(&first);
    let reply = call(
        &a.store,
        "append_to_last_entry",
        json!({"text": "Пункт два"}),
        T0 + 5 * MINUTE,
    )
    .unwrap();
    assert_eq!(reply["action"], "appended");
    assert_eq!(entry_id(&reply), id);
    assert_eq!(reply["entry"]["text"], "Пункт один\n\nПункт два");
    // Ревизия `edit` со снимком до дописывания (исключение из §A.0: ⌘Enter ревизий не пишет).
    let (snapshot, reason): (String, String) = raw(a.dir.path())
        .query_row(
            "SELECT snapshot_json, reason FROM message_revision WHERE message_id = ?1",
            [&id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(reason, "edit");
    assert!(snapshot.contains("Пункт один") && !snapshot.contains("Пункт два"));
    let rows = log_rows(a.dir.path());
    assert_eq!(rows.len(), 2);
    assert_eq!(
        (rows[1].1.as_str(), rows[1].3.as_str(), rows[1].4),
        ("append_to_last_entry", "ok", 1)
    );
}

#[test]
fn append_belongs_to_any_agent_entry_regardless_of_the_client_name() {
    let a = agent(AgentMode::ReadWrite, true);
    let first = call_as(
        &a.store,
        "Client A",
        "add_entry",
        json!({"text": "от A"}),
        T0,
    )
    .unwrap();
    let reply = call_as(
        &a.store,
        "Client B",
        "append_to_last_entry",
        json!({"text": "от B"}),
        T0 + MINUTE,
    )
    .unwrap();
    assert_eq!(entry_id(&reply), entry_id(&first));
}

#[test]
fn append_refuses_when_the_last_entry_is_not_the_agents_or_is_too_old_and_creates_nothing() {
    let a = agent(AgentMode::ReadWrite, true);
    call(&a.store, "add_entry", json!({"text": "агент в 9:12"}), T0).unwrap();
    // Пользователь записал позже: последняя запись чата не агента.
    a.store
        .send_message(INBOX_ID, "мысль пользователя", &moscow(T0 + 3 * MINUTE))
        .unwrap();
    let before = count(a.dir.path(), "message");
    assert_eq!(
        call(
            &a.store,
            "append_to_last_entry",
            json!({"text": "дописать"}),
            T0 + 5 * MINUTE
        ),
        Err(AgentError::NoAgentEntry)
    );
    assert_eq!(count(a.dir.path(), "message"), before);
    // Запись агента старше десяти минут — тоже нет.
    let b = agent(AgentMode::ReadWrite, true);
    call(&b.store, "add_entry", json!({"text": "старая"}), T0).unwrap();
    assert_eq!(
        call(
            &b.store,
            "append_to_last_entry",
            json!({"text": "поздно"}),
            T0 + 11 * MINUTE
        ),
        Err(AgentError::NoAgentEntry)
    );
    // Пустой чат — нет записи агента.
    let c = agent(AgentMode::ReadWrite, true);
    assert_eq!(
        call(
            &c.store,
            "append_to_last_entry",
            json!({"text": "некуда"}),
            T0
        ),
        Err(AgentError::NoAgentEntry)
    );
}

#[test]
fn append_after_the_user_edited_the_agent_entry_keeps_both_versions() {
    let a = agent(AgentMode::ReadWrite, true);
    let id = entry_id(
        &call(
            &a.store,
            "add_entry",
            json!({"text": "черновик агента"}),
            T0,
        )
        .unwrap(),
    );
    a.store
        .edit_message(&id, "правка пользователя", &moscow(T0 + MINUTE))
        .unwrap();
    let reply = call(
        &a.store,
        "append_to_last_entry",
        json!({"text": "после правки"}),
        T0 + 2 * MINUTE,
    )
    .unwrap();
    assert_eq!(
        reply["entry"]["text"],
        "правка пользователя\n\nпосле правки"
    );
    assert_eq!(count(a.dir.path(), "message_revision"), 2);
}

#[test]
fn the_capture_window_does_not_append_to_an_agent_entry() {
    let a = agent(AgentMode::ReadWrite, true);
    let id = entry_id(&call(&a.store, "add_entry", json!({"text": "агент"}), T0).unwrap());
    let outcome = a
        .store
        .capture_append("пользователь", &moscow(T0 + MINUTE))
        .unwrap();
    assert_ne!(outcome.message.id, id);
    assert_eq!(count(a.dir.path(), "message"), 2);
}

#[test]
fn set_task_done_marks_and_unmarks_tasks_and_refuses_plain_entries() {
    let a = agent(AgentMode::ReadWrite, true);
    let task = entry_id(
        &call(
            &a.store,
            "add_entry",
            json!({"text": "билеты", "task": true}),
            T0,
        )
        .unwrap(),
    );
    let plain = entry_id(
        &call(
            &a.store,
            "add_entry",
            json!({"text": "просто мысль"}),
            T0 + MINUTE,
        )
        .unwrap(),
    );
    let user_task = a
        .store
        .send_message(&a.work, "задача пользователя", &moscow(T0))
        .unwrap()
        .id;
    a.store
        .set_message_flag(
            &user_task,
            kinige_domain::MessageFlag::Task,
            true,
            &moscow(T0),
        )
        .unwrap();
    for id in [&task, &user_task] {
        let done = call(
            &a.store,
            "set_task_done",
            json!({"id": id, "done": true}),
            T0 + 2 * MINUTE,
        )
        .unwrap();
        assert_eq!(done["entry"]["done"], true);
        let undone = call(
            &a.store,
            "set_task_done",
            json!({"id": id, "done": false}),
            T0 + 3 * MINUTE,
        )
        .unwrap();
        assert_eq!(undone["entry"]["done"], false);
    }
    let before = log_rows(a.dir.path()).len();
    assert_eq!(
        call(
            &a.store,
            "set_task_done",
            json!({"id": plain, "done": true}),
            T0 + 4 * MINUTE
        ),
        Err(AgentError::NotATask)
    );
    let rows = log_rows(a.dir.path());
    assert_eq!(rows.len(), before + 1);
    assert_eq!(rows.last().unwrap().3, "not_a_task");
    // Запись, которая не задача, не изменилась.
    let flags = groups(a.dir.path(), "message", &plain)["flags"].clone();
    assert!(flags.contains(r#""is_task":false"#));
}
