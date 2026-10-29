//! Чтение агента (ADR-0009, слайс 4a): поиск, день и диапазон, «в этот день», запись по id, страницы и предел ответа.

mod common;

use common::agent::*;
use common::*;
use kinige_domain::AgentMode;
use kinige_domain::agent::{ENTRIES_PAGE, LIST_TEXT_CHARS, RESPONSE_BYTES, SEARCH_PAGE};
use kinige_domain::groups::INBOX_ID;
use serde_json::json;

/// 2026-03-14 12:00 по Москве.
const MARCH: i64 = 1_773_468_000_000;
const DAY: i64 = 24 * 60 * MINUTE;

#[test]
fn search_finds_only_visible_entries_and_filters_by_chat_flags_and_dates() {
    let a = agent(AgentMode::Read, true);
    let now = moscow(MARCH);
    let in_work = a
        .store
        .send_message(&a.work, "Raft лидер выбран", &now)
        .unwrap()
        .id;
    let in_inbox = a
        .store
        .send_message(INBOX_ID, "Raft и Paxos", &moscow(MARCH + MINUTE))
        .unwrap()
        .id;
    a.store
        .send_message(&a.secret, "Raft секретно", &now)
        .unwrap();
    a.store
        .send_message(&a.excluded, "Raft дневник", &now)
        .unwrap();
    let trashed = a
        .store
        .send_message(&a.work, "Raft в корзине", &now)
        .unwrap()
        .id;
    a.store.delete_message(&trashed, &now).unwrap();
    a.store
        .set_message_flag(&in_work, kinige_domain::MessageFlag::Favorite, true, &now)
        .unwrap();

    let all = call(&a.store, "search", json!({"query": "Raft"}), MARCH).unwrap();
    assert_eq!(
        entry_ids(&all),
        [in_inbox.clone(), in_work.clone()],
        "новые сверху, только видимые"
    );
    let only_work = call(
        &a.store,
        "search",
        json!({"query": "Raft", "chat_id": a.work}),
        MARCH,
    )
    .unwrap();
    assert_eq!(entry_ids(&only_work), std::slice::from_ref(&in_work));
    let favorites = call(
        &a.store,
        "search",
        json!({"query": "Raft", "favorites": true}),
        MARCH,
    )
    .unwrap();
    assert_eq!(entry_ids(&favorites), std::slice::from_ref(&in_work));
    let dated = call(
        &a.store,
        "search",
        json!({"query": "Raft", "from": "2026-03-15"}),
        MARCH,
    )
    .unwrap();
    assert!(entry_ids(&dated).is_empty());
    // Слова с окончаниями находятся (словоформы ADR-0008), пустой запрос — пустая страница.
    assert!(
        !entry_ids(&call(&a.store, "search", json!({"query": "лидера"}), MARCH).unwrap())
            .is_empty()
    );
    assert!(
        entry_ids(&call(&a.store, "search", json!({"query": "  "}), MARCH).unwrap()).is_empty()
    );
    // Журнал: id записей в ответе — в mcp_log_message, query — в args_json.
    let rows = log_rows(a.dir.path());
    assert_eq!(rows[0].3, "ok");
    assert_eq!(rows[0].4, 2);
    assert_eq!(rows[0].2, r#"{"query":"Raft"}"#);
}

#[test]
fn search_pages_by_keyset_and_a_cursor_points_to_the_next_entry() {
    let a = agent(AgentMode::Read, true);
    for i in 0..(SEARCH_PAGE + 5) {
        a.store
            .send_message(
                &a.work,
                &format!("страница заметка {i}"),
                &moscow(MARCH + i as i64 * MINUTE),
            )
            .unwrap();
    }
    let first = call(&a.store, "search", json!({"query": "заметка"}), MARCH).unwrap();
    assert_eq!(entry_ids(&first).len(), SEARCH_PAGE);
    let cursor = first["next_cursor"].as_str().unwrap().to_string();
    let second = call(
        &a.store,
        "search",
        json!({"query": "заметка", "cursor": cursor}),
        MARCH,
    )
    .unwrap();
    assert_eq!(entry_ids(&second).len(), 5);
    assert!(second.get("next_cursor").is_none());
    let mut all = entry_ids(&first);
    all.extend(entry_ids(&second));
    all.sort();
    all.dedup();
    assert_eq!(all.len(), SEARCH_PAGE + 5, "без повторов и пропусков");
}

#[test]
fn get_entries_reads_a_day_and_a_range_in_chronological_order_with_pages() {
    let a = agent(AgentMode::Read, true);
    let mut ids = Vec::new();
    for i in 0..3 {
        ids.push(
            a.store
                .send_message(&a.work, &format!("день {i}"), &moscow(MARCH + i * DAY))
                .unwrap()
                .id,
        );
    }
    let day = call(
        &a.store,
        "get_entries",
        json!({"date": "2026-03-14"}),
        MARCH,
    )
    .unwrap();
    assert_eq!(entry_ids(&day), [ids[0].clone()]);
    let range = call(
        &a.store,
        "get_entries",
        json!({"from": "2026-03-14", "to": "2026-03-16"}),
        MARCH,
    )
    .unwrap();
    assert_eq!(entry_ids(&range), ids);
    let one_chat = call(
        &a.store,
        "get_entries",
        json!({"date": "2026-03-14", "chat_id": INBOX_ID}),
        MARCH,
    )
    .unwrap();
    assert!(entry_ids(&one_chat).is_empty());
    // Страницы по 200 записей.
    for i in 0..(ENTRIES_PAGE + 3) {
        a.store
            .send_message(
                &a.work,
                &format!("много {i}"),
                &moscow(MARCH + 10 * DAY + i as i64 * 1000),
            )
            .unwrap();
    }
    let page = call(
        &a.store,
        "get_entries",
        json!({"date": "2026-03-24"}),
        MARCH,
    )
    .unwrap();
    assert_eq!(entry_ids(&page).len(), ENTRIES_PAGE);
    let cursor = page["next_cursor"].as_str().unwrap();
    let rest = call(
        &a.store,
        "get_entries",
        json!({"date": "2026-03-24", "cursor": cursor}),
        MARCH,
    )
    .unwrap();
    assert_eq!(entry_ids(&rest).len(), 3);
}

#[test]
fn on_this_day_returns_past_years_and_leap_day_matches_only_leap_day() {
    let a = agent(AgentMode::Read, true);
    let at = |date_ms: i64| moscow(date_ms);
    // 2024-02-29 12:00 MSK, 2025-02-28, 2025-03-14, 2026-03-14 (сегодня), 2023-03-14.
    let leap = a
        .store
        .send_message(&a.work, "високосный", &at(1_709_200_800_000))
        .unwrap()
        .id;
    a.store
        .send_message(&a.work, "двадцать восьмое", &at(1_740_729_600_000))
        .unwrap();
    let last_year = a
        .store
        .send_message(&a.work, "год назад", &at(MARCH - 365 * DAY))
        .unwrap()
        .id;
    a.store
        .send_message(&a.work, "сегодня", &at(MARCH))
        .unwrap();
    let three = a
        .store
        .send_message(&a.work, "три года назад", &at(MARCH - 3 * 365 * DAY - DAY))
        .unwrap()
        .id;
    let reply = call(
        &a.store,
        "on_this_day",
        json!({"date": "2026-03-14"}),
        MARCH,
    )
    .unwrap();
    let ids = entry_ids(&reply);
    assert!(ids.contains(&last_year), "{reply}");
    assert!(
        !ids.iter().any(|id| id == &leap),
        "29 февраля не показывается 14 марта"
    );
    assert!(reply.to_string().contains("год назад") && !reply.to_string().contains("сегодня"));
    let _ = three;
    let feb29 = call(
        &a.store,
        "on_this_day",
        json!({"date": "2028-02-29"}),
        MARCH,
    )
    .unwrap();
    assert_eq!(entry_ids(&feb29), [leap]);
    let feb28 = call(
        &a.store,
        "on_this_day",
        json!({"date": "2028-02-28"}),
        MARCH,
    )
    .unwrap();
    assert_eq!(
        entry_ids(&feb28).len(),
        1,
        "28 февраля: только запись 28 февраля"
    );
    // Без даты — сегодняшний день журнала по времени вызова.
    let today = call(&a.store, "on_this_day", json!({}), MARCH).unwrap();
    assert!(today.to_string().contains("год назад"));
}

#[test]
fn list_texts_are_cut_and_get_entry_returns_the_whole_text() {
    let a = agent(AgentMode::Read, true);
    let long = "я".repeat(LIST_TEXT_CHARS + 500);
    let id = a
        .store
        .send_message(&a.work, &long, &moscow(MARCH))
        .unwrap()
        .id;
    let list = call(
        &a.store,
        "get_entries",
        json!({"date": "2026-03-14"}),
        MARCH,
    )
    .unwrap();
    assert_eq!(list["entries"][0]["truncated"], true);
    assert_eq!(
        list["entries"][0]["text"].as_str().unwrap().chars().count(),
        LIST_TEXT_CHARS
    );
    let whole = call(&a.store, "get_entry", json!({"id": id}), MARCH).unwrap();
    assert_eq!(
        whole["entry"]["text"].as_str().unwrap().chars().count(),
        LIST_TEXT_CHARS + 500
    );
    assert!(whole["entry"].get("truncated").is_none());
    assert!(whole["entry"].get("reply_to_id").is_none());
}

#[test]
fn a_response_over_the_limit_is_cut_by_whole_entries_and_only_returned_ids_are_logged() {
    let a = agent(AgentMode::Read, true);
    // Четыре байта на знак: 200 записей по 2 000 знаков не помещаются в 1 МиБ.
    let big = "😀".repeat(LIST_TEXT_CHARS);
    for i in 0..ENTRIES_PAGE {
        a.store
            .send_message(
                &a.work,
                &format!("{big} {i}"),
                &moscow(MARCH + i as i64 * 1000),
            )
            .unwrap();
    }
    let reply = call(
        &a.store,
        "get_entries",
        json!({"date": "2026-03-14"}),
        MARCH,
    )
    .unwrap();
    let returned = entry_ids(&reply).len();
    assert!(
        returned < ENTRIES_PAGE,
        "ответ усечён по записям: {returned}"
    );
    assert!(reply.to_string().len() <= RESPONSE_BYTES);
    assert!(reply["next_cursor"].is_string());
    let rows = log_rows(a.dir.path());
    assert_eq!(
        rows[0].4, returned as i64,
        "в журнале только возвращённые записи"
    );
    let rest = call(
        &a.store,
        "get_entries",
        json!({"date": "2026-03-14", "cursor": reply["next_cursor"]}),
        MARCH,
    )
    .unwrap();
    assert_eq!(
        returned + entry_ids(&rest).len(),
        ENTRIES_PAGE,
        "курсор указывает на следующую запись"
    );
}

#[test]
fn list_chats_shows_visible_chats_with_counts_and_archive() {
    let a = agent(AgentMode::Read, true);
    a.store
        .send_message(&a.work, "раз", &moscow(MARCH))
        .unwrap();
    let gone = a
        .store
        .send_message(&a.work, "два", &moscow(MARCH))
        .unwrap()
        .id;
    a.store.delete_message(&gone, &moscow(MARCH)).unwrap();
    a.store
        .set_chat_archived(&a.work, true, &moscow(MARCH))
        .unwrap();
    let chats = call(&a.store, "list_chats", json!({}), MARCH).unwrap();
    let work = chats["chats"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == json!(a.work))
        .unwrap()
        .clone();
    assert_eq!(work["entries"], 1);
    assert_eq!(work["archived"], true, "архив виден");
    let inbox = chats["chats"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == json!(INBOX_ID))
        .unwrap()
        .clone();
    assert_eq!(inbox["title"], "Inbox");
    // Корзина не раскрывается: ни размера, ни содержимого.
    assert!(!chats.to_string().contains("trash"));
}

/// Паника внутри запроса чтения агента (внутри `query_map`) — ошибка этого вызова; соединение чтения живо, следующие
/// вызовы агента (в том числе после повторной паники) проходят.
#[test]
fn a_panic_inside_an_agent_read_fails_that_call_only_and_later_agent_calls_work() {
    let a = agent(AgentMode::Read, true);
    a.store
        .send_message(&a.work, "до паники", &moscow(MARCH))
        .unwrap();
    for _ in 0..2 {
        assert!(matches!(
            a.store.panic_in_agent_read_for_test(),
            Err(kinige_store::StoreError::Internal(_))
        ));
        let chats = call(&a.store, "list_chats", json!({}), MARCH).unwrap();
        assert!(chats.to_string().contains("Работа"));
        let found = call(&a.store, "search", json!({"query": "паники"}), MARCH).unwrap();
        assert_eq!(entry_ids(&found).len(), 1);
    }
}
