//! "Recent chats" of the tray menu (KINIG-47.3): ordered by the last live entry in feed order, one backward step over
//! `message_feed` per chat.

// The root of the integration tests is entirely test code: `cfg(test)` gives helpers outside #[test] the same
// clippy.toml relaxations (unwrap/expect/panic/indexing) as the tests in src/.
#![cfg(test)]

mod common;

use common::*;
use kinige_domain::groups::INBOX_ID;
use kinige_store::{Now, Store};

fn at(minutes: i64) -> Now {
    moscow(T0 + minutes * MINUTE)
}

fn titles(store: &Store, limit: usize) -> Vec<String> {
    store
        .recent_chats(limit)
        .unwrap()
        .into_iter()
        .map(|chat| chat.title)
        .collect()
}

#[test]
fn recent_chats_follow_the_last_entry_and_skip_inbox_archive_and_trash() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let mut ids = Vec::new();
    for (index, title) in ["Один", "Два", "Три", "Четыре"].into_iter().enumerate() {
        let chat = store.create_chat(title, None, &at(index as i64)).unwrap();
        store
            .send_message(&chat.id, "запись", &at(10 + index as i64))
            .unwrap();
        ids.push(chat.id);
    }
    store.create_chat("Пустой", None, &at(30)).unwrap();
    store
        .send_message(INBOX_ID, "во входящих", &at(40))
        .unwrap();
    store.set_chat_archived(&ids[3], true, &at(41)).unwrap();
    assert_eq!(titles(&store, 5), ["Три", "Два", "Один"]);
    assert_eq!(titles(&store, 2), ["Три", "Два"]);
    assert!(titles(&store, 0).is_empty());

    // A new entry raises its chat; an entry in the trash is not the last one.
    store.send_message(&ids[0], "снова", &at(50)).unwrap();
    assert_eq!(titles(&store, 1), ["Один"]);
    let last = store.send_message(&ids[1], "последняя", &at(60)).unwrap();
    store.delete_message(&last.id, &at(61)).unwrap();
    // "Два" falls back to its earlier entry, which is older than the one in "Три".
    assert_eq!(titles(&store, 5), ["Один", "Три", "Два"]);
    // A chat whose only entry went to the trash has no live entry and drops out.
    let only = store.create_chat("Одна", None, &at(70)).unwrap();
    let lone = store.send_message(&only.id, "одна", &at(71)).unwrap();
    assert_eq!(titles(&store, 1), ["Одна"]);
    store.delete_message(&lone.id, &at(72)).unwrap();
    assert!(!titles(&store, 9).contains(&"Одна".to_string()));
}

#[test]
fn an_entry_moved_to_an_old_day_does_not_raise_its_chat() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let old = store.create_chat("Старый", None, &at(1)).unwrap();
    let new = store.create_chat("Новый", None, &at(2)).unwrap();
    store.send_message(&old.id, "раньше", &at(10)).unwrap();
    store.send_message(&new.id, "позже", &at(20)).unwrap();
    assert_eq!(titles(&store, 2), ["Новый", "Старый"]);

    // A fresh note placed on an old journal day sits early in feed order: the chat does not jump up.
    let fresh = store
        .send_message(&old.id, "задним числом", &at(30))
        .unwrap();
    assert_eq!(titles(&store, 2), ["Старый", "Новый"]);
    store
        .set_message_date(&fresh.id, "2020-01-05", &at(31))
        .unwrap();
    assert_eq!(titles(&store, 2), ["Новый", "Старый"]);
}

#[test]
fn the_plan_walks_message_feed_and_never_scans_message() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let plan = store.recent_chats_plan_for_test().unwrap();
    let text = plan.join(" | ");
    assert!(text.contains("message_feed"), "plan: {text}");
    assert!(!text.contains("SCAN message"), "plan: {text}");
    assert!(
        !text.contains("TEMP B-TREE"),
        "the order comes from the index: {text}"
    );
}
