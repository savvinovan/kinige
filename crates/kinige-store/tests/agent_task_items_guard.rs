//! Страховки отметки пункта для агента (ADR-0009, слайс 4a, замечания guardian): `item_text` против сдвинувшихся номеров,
//! предел ответа записи при десятках тысяч пунктов, `item` не трогает флаг «выполнено» самой записи-задачи.

mod common;

use common::agent::*;
use common::*;
use kinige_domain::AgentMode;
use kinige_domain::agent::RESPONSE_BYTES;
use kinige_store::AgentError;
use serde_json::{Value, json};

const BODY: &str = "Планы\n- [ ] первый\n- [ ] Почитать книгу\n- [ ] третий";

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

fn mark(a: &Agent, id: &str, arguments: Value) -> Result<Value, AgentError> {
    let mut all = json!({"id": id, "done": true});
    all.as_object_mut()
        .unwrap()
        .extend(arguments.as_object().unwrap().clone());
    call(&a.store, "set_task_done", all, T0 + 2 * MINUTE)
}

#[test]
fn an_item_added_above_after_the_agent_read_the_list_is_a_mismatch_and_nothing_is_written() {
    let a = agent(AgentMode::ReadWrite, true);
    let id = a.store.send_message(&a.work, BODY, &moscow(T0)).unwrap().id;
    // Агент прочитал: «Почитать книгу» — пункт 1.
    let read = call(&a.store, "get_entry", json!({"id": id}), T0).unwrap();
    assert_eq!(read["entry"]["tasks"][1]["text"], "Почитать книгу");
    // Пользователь добавил пункт наверх: теперь под номером 1 стоит «первый».
    let edited = "Планы\n- [ ] новый\n- [ ] первый\n- [ ] Почитать книгу\n- [ ] третий";
    a.store
        .edit_message(&id, edited, &moscow(T0 + MINUTE))
        .unwrap();
    let revisions_before = revisions(&a, &id);
    let error = mark(&a, &id, json!({"item": 1, "item_text": "Почитать книгу"})).unwrap_err();
    assert_eq!(error.code(), "invalid_argument");
    assert!(
        matches!(&error, AgentError::Invalid(why) if why == "checklist item changed; re-read the entry")
    );
    assert_eq!(body_of(&a, &id), edited, "ничего не записано");
    assert_eq!(revisions(&a, &id), revisions_before);
    let rows = log_rows(a.dir.path());
    let last = rows.last().unwrap();
    assert_eq!(last.3, "invalid_argument");
    // Текст пункта в журнал не попадает ни при отказе, ни при успехе.
    assert!(rows.iter().all(|r| !r.2.contains("Почитать")));
    // После повторного чтения с новым номером — отмечается тот пункт, который имелся в виду.
    mark(&a, &id, json!({"item": 2, "item_text": "Почитать книгу"})).unwrap();
    assert_eq!(
        body_of(&a, &id),
        edited.replacen("- [ ] Почитать книгу", "- [x] Почитать книгу", 1)
    );
    assert!(
        log_rows(a.dir.path())
            .iter()
            .all(|r| !r.2.contains("Почитать"))
    );
}

#[test]
fn a_matching_item_text_marks_the_item_and_surrounding_spaces_do_not_matter() {
    let a = agent(AgentMode::ReadWrite, true);
    let id = a.store.send_message(&a.work, BODY, &moscow(T0)).unwrap().id;
    let reply = mark(
        &a,
        &id,
        json!({"item": 1, "item_text": "  Почитать книгу \n"}),
    )
    .unwrap();
    assert_eq!(reply["entry"]["tasks"][1]["done"], true);
    assert!(body_of(&a, &id).contains("- [x] Почитать книгу"));
    // Совпавший текст у пункта в нужном состоянии — успех без записи.
    let revisions_now = revisions(&a, &id);
    mark(&a, &id, json!({"item": 1, "item_text": "Почитать книгу"})).unwrap();
    assert_eq!(revisions(&a, &id), revisions_now);
    // Текст другого регистра — другой пункт.
    let error = mark(&a, &id, json!({"item": 0, "item_text": "Первый "})).unwrap_err();
    assert_eq!(error.code(), "invalid_argument");
    // `item_text` без `item` — не принимается, пустой — тоже.
    for junk in [
        json!({"item_text": "первый"}),
        json!({"item": 0, "item_text": ""}),
        json!({"item": 0, "item_text": 5}),
    ] {
        assert_eq!(mark(&a, &id, junk).unwrap_err().code(), "invalid_argument");
    }
}

/// Без `item_text` номер принимается как есть, даже если пункт под ним уже не тот: так работал бы клиент, который не
/// передаёт страховку. Описание инструмента просит её передавать; тест фиксирует, что без неё поведение прежнее.
#[test]
fn without_item_text_the_number_is_taken_as_it_is_even_when_the_items_shifted() {
    let a = agent(AgentMode::ReadWrite, true);
    let id = a.store.send_message(&a.work, BODY, &moscow(T0)).unwrap().id;
    let edited = "Планы\n- [ ] новый\n- [ ] первый\n- [ ] Почитать книгу\n- [ ] третий";
    a.store
        .edit_message(&id, edited, &moscow(T0 + MINUTE))
        .unwrap();
    mark(&a, &id, json!({"item": 1})).unwrap();
    assert_eq!(
        body_of(&a, &id),
        edited.replacen("- [ ] первый", "- [x] первый", 1),
        "отмечен пункт, который теперь стоит под номером 1"
    );
}

#[test]
fn an_item_never_changes_the_done_flag_of_a_task_entry() {
    let a = agent(AgentMode::ReadWrite, true);
    let created = call(
        &a.store,
        "add_entry",
        json!({"text": "Покупки\n- [ ] хлеб\n- [ ] молоко", "task": true, "chat_id": a.work}),
        T0,
    )
    .unwrap();
    let id = created["entry"]["id"].as_str().unwrap().to_string();
    let flags = |a: &Agent| -> String {
        raw(a.dir.path())
            .query_row(
                "SELECT task_done_at FROM message WHERE id = ?1",
                [&id],
                |r| r.get::<_, Option<i64>>(0),
            )
            .unwrap()
            .map_or("null".into(), |at| at.to_string())
    };
    assert_eq!(flags(&a), "null");
    for (item, text) in ["хлеб", "молоко"].into_iter().enumerate() {
        let reply = mark(&a, &id, json!({"item": item, "item_text": text})).unwrap();
        assert_eq!(reply["entry"]["task"], true);
        assert_eq!(
            reply["entry"]["done"], false,
            "запись-задача не стала выполненной"
        );
    }
    assert!(body_of(&a, &id).contains("- [x] хлеб") && body_of(&a, &id).contains("- [x] молоко"));
    assert_eq!(
        flags(&a),
        "null",
        "task_done_at не тронут, хотя все пункты отмечены"
    );
    // Отметка записи целиком по-прежнему отдельное действие.
    let whole = call(
        &a.store,
        "set_task_done",
        json!({"id": id, "done": true}),
        T0 + 3 * MINUTE,
    )
    .unwrap();
    assert_eq!(whole["entry"]["done"], true);
}

/// Ответы инструментов записи, как и `get_entry`, не больше `RESPONSE_BYTES`, даже когда `tasks` вдвое больше текста.
#[test]
fn write_replies_stay_within_the_response_limit_for_entries_with_tens_of_thousands_of_items() {
    let a = agent(AgentMode::ReadWrite, true);
    let big: String = std::iter::once("Большой список".to_string())
        .chain((0..50_000).map(|_| "- [ ] a".to_string()))
        .collect::<Vec<_>>()
        .join("\n");
    let size = |reply: &Value| reply.to_string().len();
    let created = call(
        &a.store,
        "add_entry",
        json!({"text": big, "chat_id": a.work}),
        T0,
    )
    .unwrap();
    assert!(
        size(&created) <= RESPONSE_BYTES,
        "add_entry: {}",
        size(&created)
    );
    assert_eq!(created["entry"]["truncated"], true);
    let id = created["entry"]["id"].as_str().unwrap().to_string();
    // Начало списка цело, номера идут с нуля.
    let tasks = created["entry"]["tasks"].as_array().unwrap();
    assert!(!tasks.is_empty());
    for (position, task) in tasks.iter().enumerate() {
        assert_eq!(task["item"], position);
    }
    let appended = call(
        &a.store,
        "append_to_last_entry",
        json!({"text": "Конец", "chat_id": a.work}),
        T0 + MINUTE,
    )
    .unwrap();
    assert!(
        size(&appended) <= RESPONSE_BYTES,
        "append: {}",
        size(&appended)
    );
    let marked = mark(&a, &id, json!({"item": 100, "item_text": "a"})).unwrap();
    assert!(
        size(&marked) <= RESPONSE_BYTES,
        "set_task_done: {}",
        size(&marked)
    );
    // В базе запись целая: пункт 100 отмечен, текст не обрезан.
    let body = body_of(&a, &id);
    assert!(body.ends_with("Конец"));
    assert_eq!(body.matches("- [x] a").count(), 1);
    assert_eq!(body.matches("- [ ] a").count(), 49_999);
    // `get_entry` по-прежнему в пределе.
    let read = call(&a.store, "get_entry", json!({"id": id}), T0).unwrap();
    assert!(size(&read) <= RESPONSE_BYTES);
}
