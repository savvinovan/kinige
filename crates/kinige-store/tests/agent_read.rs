//! Agent reads (ADR-0009, slice 4a): search, a day and a range, "on this day", an entry by id, pages and the reply limit.

// The root of the integration tests is entirely test code: `cfg(test)` gives helpers outside #[test] the same
// clippy.toml relaxations (unwrap/expect/panic/indexing) as the tests in src/.
#![cfg(test)]

mod common;

use common::agent::*;
use common::*;
use kinige_domain::AgentMode;
use kinige_domain::agent::{ENTRIES_PAGE, LIST_TEXT_CHARS, RESPONSE_BYTES, SEARCH_PAGE};
use kinige_domain::groups::INBOX_ID;
use serde_json::json;

/// 2026-03-14 12:00 Moscow time.
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
        "newest first, only the visible ones"
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
    // Words with endings are found (word forms, ADR-0008), an empty query gives an empty page.
    assert!(
        !entry_ids(&call(&a.store, "search", json!({"query": "лидера"}), MARCH).unwrap())
            .is_empty()
    );
    assert!(
        entry_ids(&call(&a.store, "search", json!({"query": "  "}), MARCH).unwrap()).is_empty()
    );
    // Log: the ids of the entries in the reply go to mcp_log_message, the query goes to args_json.
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
    assert_eq!(all.len(), SEARCH_PAGE + 5, "no repeats and no gaps");
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
    // Pages of 200 entries.
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
    // 2024-02-29 12:00 MSK, 2025-02-28, 2025-03-14, 2026-03-14 (today), 2023-03-14.
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
        "February 29 is not shown on March 14"
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
        "February 28: only the February 28 entry"
    );
    // Without a date, today's journal day by the call time.
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
    // Four bytes per character: 200 entries of 2,000 characters do not fit into 1 MiB.
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
        "the reply is truncated by entries: {returned}"
    );
    assert!(reply.to_string().len() <= RESPONSE_BYTES);
    assert!(reply["next_cursor"].is_string());
    let rows = log_rows(a.dir.path());
    assert_eq!(
        rows[0].4, returned as i64,
        "the log has only the returned entries"
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
        "the cursor points to the next entry"
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
    assert_eq!(work["archived"], true, "the archive is visible");
    let inbox = chats["chats"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == json!(INBOX_ID))
        .unwrap()
        .clone();
    assert_eq!(inbox["title"], "Inbox");
    // The trash is not revealed: neither its size nor its content.
    assert!(!chats.to_string().contains("trash"));
}

/// A panic inside an agent read query (inside `query_map`) is an error of that call; the read connection stays alive, the next
/// agent calls (including after a repeated panic) go through.
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
        // The window's own reads are on another connection and are not affected.
        assert!(a.store.list_chats().unwrap().len() >= 2);
    }
}

/// KINIG-47.2: the agent has its own read connection. While one agent read holds it, the window's reads (feed page, chat list,
/// the tray menu's recent chats) answer at once; the other way round too.
#[test]
fn an_agent_read_in_progress_does_not_hold_back_the_windows_reads() {
    let a = agent(AgentMode::Read, true);
    a.store
        .send_message(&a.work, "запись в работе", &moscow(MARCH))
        .unwrap();
    let store = a.store.clone();
    let (page, chats, recent) = a.store.with_agent_read_held_for_test(move || {
        // The window's reads run on another thread so that a shared lock would hang the test instead of passing it silently.
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .spawn(move || {
                let page = store
                    .list_messages(INBOX_ID, None, 10, &moscow(MARCH))
                    .map(|p| p.messages.len());
                let chats = store.list_chats().map(|c| c.len());
                let recent = store.recent_chats(5).map(|c| c.len());
                let _ = sender.send((page, chats, recent));
            })
            .unwrap();
        receiver
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("the window's reads must not wait for the agent's connection")
    });
    assert_eq!(page.unwrap(), 0);
    assert!(chats.unwrap() >= 2);
    assert_eq!(recent.unwrap(), 1);
}

/// A panic in an agent read leaves both connections usable: the window's reads work, and so do later agent calls.
#[test]
fn after_an_agent_panic_the_windows_reads_still_work() {
    let a = agent(AgentMode::Read, true);
    a.store
        .send_message(&a.work, "до паники", &moscow(MARCH))
        .unwrap();
    assert!(matches!(
        a.store.panic_in_agent_read_for_test(),
        Err(kinige_store::StoreError::Internal(_))
    ));
    assert!(a.store.list_chats().unwrap().len() >= 2);
    assert_eq!(a.store.recent_chats(5).unwrap().len(), 1);
    let found = call(&a.store, "search", json!({"query": "паники"}), MARCH).unwrap();
    assert_eq!(entry_ids(&found).len(), 1);
}

/// The agent's own read connection is `query_only`: a write through it is refused by SQLite and changes nothing.
#[test]
fn the_agent_connection_refuses_a_write() {
    let a = agent(AgentMode::Read, true);
    let before = a.store.list_chats().unwrap();
    let error = a.store.write_through_agent_read_for_test();
    assert!(error.contains("readonly"), "got: {error:?}");
    assert_eq!(a.store.list_chats().unwrap(), before);
}
