//! Пункты `- [ ]` внутри записи для агента (ADR-0009, слайс 4a, приёмка): `get_entry` и `get_entries` отдают `tasks` с
//! номерами, `set_task_done` с `item` переключает один пункт тем же `set_task_item`, что щелчок в ленте, с ревизией `edit`.

mod common;

use common::agent::*;
use common::*;
use kinige_domain::AgentMode;
use kinige_domain::groups::INBOX_ID;
use kinige_store::AgentError;
use serde_json::{Value, json};

/// Пункты 0 «первый», 1 «второй», 2 «в цитате», 3 «четвёртый» (отмечен); пункт в блоке кода не считается.
const BODY: &str = "Планы на выходные\n- [ ] первый\n- [ ] второй\n```\n- [ ] в коде\n```\n> - [ ] в цитате\n- [x] четвёртый";

fn body_of(a: &Agent, id: &str) -> String {
    raw(a.dir.path())
        .query_row("SELECT body_md FROM message WHERE id = ?1", [id], |r| {
            r.get(0)
        })
        .unwrap()
}

fn revisions(a: &Agent, id: &str) -> i64 {
    raw(a.dir.path())
        .query_row(
            "SELECT COUNT(*) FROM message_revision WHERE message_id = ?1",
            [id],
            |r| r.get(0),
        )
        .unwrap()
}

fn user_entry(a: &Agent, chat: &str, body: &str) -> String {
    a.store.send_message(chat, body, &moscow(T0)).unwrap().id
}

fn mark(a: &Agent, id: &str, item: Value, done: bool) -> Result<Value, AgentError> {
    call(
        &a.store,
        "set_task_done",
        json!({"id": id, "done": done, "item": item}),
        T0 + MINUTE,
    )
}

#[test]
fn entries_list_their_checklist_items_with_numbers_and_plain_entries_have_none() {
    let a = agent(AgentMode::Read, true);
    let id = user_entry(&a, &a.work, BODY);
    let plain = user_entry(&a, &a.work, "без пунктов");
    let one = call(&a.store, "get_entry", json!({"id": id}), T0).unwrap();
    assert_eq!(
        one["entry"]["tasks"],
        json!([
            {"item": 0, "done": false, "text": "первый"},
            {"item": 1, "done": false, "text": "второй"},
            {"item": 2, "done": false, "text": "в цитате"},
            {"item": 3, "done": true, "text": "четвёртый"},
        ])
    );
    let day = call(
        &a.store,
        "get_entries",
        json!({"date": "2026-10-21", "chat_id": a.work}),
        T0,
    )
    .unwrap();
    let entries = day["entries"].as_array().unwrap();
    let with = entries.iter().find(|e| e["id"] == json!(id)).unwrap();
    assert_eq!(with["tasks"].as_array().unwrap().len(), 4);
    let without = entries.iter().find(|e| e["id"] == json!(plain)).unwrap();
    assert!(without.get("tasks").is_none(), "поля нет, если пунктов нет");
}

#[test]
fn one_item_is_toggled_in_the_text_with_an_edit_revision_and_the_log_keeps_only_its_number() {
    let a = agent(AgentMode::ReadWrite, true);
    let id = user_entry(&a, &a.work, BODY);
    let done = mark(&a, &id, json!(1), true).unwrap();
    assert_eq!(done["action"], "updated");
    assert_eq!(
        done["entry"]["tasks"][1],
        json!({"item": 1, "done": true, "text": "второй"})
    );
    assert_eq!(done["entry"]["task"], false, "сама запись не стала задачей");
    assert_eq!(
        body_of(&a, &id),
        BODY.replacen("- [ ] второй", "- [x] второй", 1)
    );
    // Ревизия `edit` со снимком до правки (как у щелчка в ленте).
    assert_eq!(revisions(&a, &id), 1);
    let snapshot: String = raw(a.dir.path())
        .query_row(
            "SELECT snapshot_json FROM message_revision WHERE message_id = ?1",
            [&id],
            |r| r.get(0),
        )
        .unwrap();
    assert!(snapshot.contains("- [ ] второй"));
    // Пункт в цитате считается по правилам домена; снять отметку тоже можно.
    mark(&a, &id, json!(2), true).unwrap();
    assert!(body_of(&a, &id).contains("> - [x] в цитате"));
    mark(&a, &id, json!(3), false).unwrap();
    assert!(body_of(&a, &id).ends_with("- [ ] четвёртый"));
    assert_eq!(revisions(&a, &id), 3);
    // Журнал: номер пункта и больше ничего; текст пункта и записи в `args_json` нет.
    let rows = log_rows(a.dir.path());
    let last = rows.last().unwrap();
    assert_eq!((last.1.as_str(), last.3.as_str()), ("set_task_done", "ok"));
    assert_eq!(last.2, r#"{"done":false,"item":3}"#);
    assert!(
        rows.iter()
            .all(|r| !r.2.contains("второй") && !r.2.contains(&id))
    );
    assert_eq!(last.4, 1, "в журнал ушла одна запись");
}

#[test]
fn an_item_inside_a_code_block_cannot_be_addressed_and_a_missing_number_is_an_invalid_argument() {
    let a = agent(AgentMode::ReadWrite, true);
    let id = user_entry(&a, &a.work, BODY);
    // Пунктов четыре (0..=3): «в коде» номера не имеет.
    for item in [4, 5, 999] {
        let error = mark(&a, &id, json!(item), true).unwrap_err();
        assert!(matches!(error, AgentError::Invalid(_)), "{error:?}");
        assert_eq!(error.code(), "invalid_argument");
    }
    assert_eq!(body_of(&a, &id), BODY, "ничего не изменилось");
    assert_eq!(revisions(&a, &id), 0);
    assert!(body_of(&a, &id).contains("- [ ] в коде"));
    let rows = log_rows(a.dir.path());
    assert_eq!(rows.last().unwrap().3, "invalid_argument");
    // Запись без пунктов: любой номер — тот же отказ.
    let plain = user_entry(&a, &a.work, "без пунктов");
    assert!(matches!(
        mark(&a, &plain, json!(0), true),
        Err(AgentError::Invalid(_))
    ));
}

#[test]
fn an_item_already_in_the_requested_state_is_a_success_without_a_write() {
    let a = agent(AgentMode::ReadWrite, true);
    let id = user_entry(&a, &a.work, BODY);
    let reply = mark(&a, &id, json!(3), true).unwrap();
    assert_eq!(reply["entry"]["tasks"][3]["done"], true);
    mark(&a, &id, json!(0), false).unwrap();
    assert_eq!(body_of(&a, &id), BODY);
    assert_eq!(revisions(&a, &id), 0, "ревизий нет: текст не менялся");
    let rows = log_rows(a.dir.path());
    assert!(rows.iter().all(|r| r.3 == "ok"));
}

#[test]
fn an_item_in_a_hidden_chat_looks_like_a_missing_entry() {
    let a = agent(AgentMode::ReadWrite, false);
    let in_secret = user_entry(&a, &a.secret, BODY);
    let in_excluded = user_entry(&a, &a.excluded, BODY);
    let in_inbox = user_entry(&a, INBOX_ID, BODY);
    for id in [&in_secret, &in_excluded, &in_inbox] {
        assert_eq!(mark(&a, id, json!(0), true), Err(AgentError::NotFound));
        assert_eq!(body_of(&a, id), BODY);
        assert_eq!(revisions(&a, id), 0);
    }
    // Чтение чужого чата тоже ничего не отдаёт: пунктов не видно.
    assert_eq!(
        call(&a.store, "get_entry", json!({"id": in_secret}), T0),
        Err(AgentError::NotFound)
    );
}

#[test]
fn read_only_mode_refuses_an_item_and_changes_nothing() {
    let a = agent(AgentMode::Read, true);
    let id = user_entry(&a, &a.work, BODY);
    let error = mark(&a, &id, json!(0), true).unwrap_err();
    assert_eq!(error, AgentError::ReadOnly);
    assert_eq!(error.code(), "mcp_read_only");
    assert_eq!(body_of(&a, &id), BODY);
    assert_eq!(revisions(&a, &id), 0);
}

#[test]
fn junk_item_values_are_invalid_arguments_and_nothing_is_written() {
    let a = agent(AgentMode::ReadWrite, true);
    let id = user_entry(&a, &a.work, BODY);
    for junk in [
        json!(-1),
        json!(18_446_744_073_709_551_615u64),
        json!(1_000_001),
        json!(1.5),
        json!("1"),
        json!("первый"),
        json!(true),
        json!([0]),
        json!({"item": 0}),
    ] {
        let error = mark(&a, &id, junk.clone(), true).unwrap_err();
        assert_eq!(error.code(), "invalid_argument", "{junk}");
    }
    assert_eq!(body_of(&a, &id), BODY);
    assert_eq!(revisions(&a, &id), 0);
    // `null` — «поля нет»: отметка записи целиком, а она не задача.
    assert_eq!(mark(&a, &id, Value::Null, true), Err(AgentError::NotATask));
}

#[test]
fn the_markdown_copy_gets_the_toggled_item() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path());
    let work = store.create_chat("Работа", None, &moscow(T0)).unwrap().id;
    store.set_agent_chats(vec![work.clone()]).unwrap();
    store.set_agent_mode(AgentMode::ReadWrite).unwrap();
    store.set_agent_enabled(true).unwrap();
    let id = store.send_message(&work, BODY, &moscow(T0)).unwrap().id;
    call(
        &store,
        "set_task_done",
        json!({"id": id, "done": true, "item": 0}),
        T0 + MINUTE,
    )
    .unwrap();
    let file = mirror_dir(dir.path())
        .join("Работа")
        .join("2026")
        .join("2026-10-21.md");
    wait_until(
        "файл копии с отмеченным пунктом",
        || std::fs::read_to_string(&file).is_ok_and(|text| text.contains("- [x] первый")),
    );
}
