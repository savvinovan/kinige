//! The entry a click on a day opens and the view state of the forest in the store (slice 6): the rule for the chat (the open
//! one, the one with the most entries, the Inbox on a tie), garbage input, and the two `app_setting` keys. The year table is in
//! `forest.rs`.

#![cfg(test)]

mod common;

use common::days::{at, ms};
use common::*;
use kinige_domain::groups::INBOX_ID;
use kinige_domain::{ForestMode, ForestView};
use kinige_store::{Now, Store, StoreError};

fn now() -> Now {
    at(2026, 10, 21, 12, 0)
}

fn put(store: &Store, chat: &str, body: &str, when: i64) -> String {
    store.send_message(chat, body, &moscow(when)).unwrap().id
}

fn trash_chat(dir: &std::path::Path, id: &str) {
    raw(dir)
        .execute("UPDATE chat SET deleted_at = 1 WHERE id = ?1", [id])
        .unwrap();
}

#[test]
fn the_open_chat_is_the_target_when_it_has_entries_that_day() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let books = store
        .create_chat("Books", None, &at(2020, 1, 1, 9, 0))
        .unwrap();
    let first_books = put(&store, &books.id, "books", ms(2026, 10, 16, 9, 0));
    put(&store, &books.id, "books again", ms(2026, 10, 16, 9, 5));
    for i in 0..4 {
        put(&store, INBOX_ID, "inbox", ms(2026, 10, 16, 10, i));
    }
    let target = store
        .forest_day_target("2026-10-16", Some(&books.id))
        .unwrap()
        .unwrap();
    assert_eq!(
        (target.chat_id.as_str(), target.message_id.as_str()),
        (books.id.as_str(), first_books.as_str())
    );
}

#[test]
fn otherwise_the_chat_with_most_entries_and_the_inbox_on_a_tie() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let books = store
        .create_chat("Books", None, &at(2020, 1, 1, 9, 0))
        .unwrap();
    let films = store
        .create_chat("Films", None, &at(2020, 1, 1, 9, 1))
        .unwrap();
    // Books 2, Films 3: not the open chat (Inbox has nothing that day) -> Films.
    put(&store, &books.id, "b1", ms(2026, 10, 16, 9, 0));
    put(&store, &books.id, "b2", ms(2026, 10, 16, 9, 1));
    let f1 = put(&store, &films.id, "f1", ms(2026, 10, 16, 10, 0));
    put(&store, &films.id, "f2", ms(2026, 10, 16, 10, 1));
    put(&store, &films.id, "f3", ms(2026, 10, 16, 10, 2));
    let by_count = store
        .forest_day_target("2026-10-16", Some(INBOX_ID))
        .unwrap()
        .unwrap();
    assert_eq!(
        (by_count.chat_id.as_str(), by_count.message_id.as_str()),
        (films.id.as_str(), f1.as_str())
    );
    // Inbox 3 entries ties with Films 3: the Inbox wins even when another chat (one without entries that day) is open.
    let i1 = put(&store, INBOX_ID, "i1", ms(2026, 10, 16, 11, 0));
    put(&store, INBOX_ID, "i2", ms(2026, 10, 16, 11, 1));
    put(&store, INBOX_ID, "i3", ms(2026, 10, 16, 11, 2));
    let quiet = store
        .create_chat("Quiet", None, &at(2020, 1, 1, 9, 3))
        .unwrap();
    let tie = store
        .forest_day_target("2026-10-16", Some(&quiet.id))
        .unwrap()
        .unwrap();
    assert_eq!(
        (tie.chat_id.as_str(), tie.message_id.as_str()),
        (INBOX_ID, i1.as_str())
    );
    // Trash and trashed chats are not candidates.
    trash_chat(dir.path(), &films.id);
    let without_films = store
        .forest_day_target("2026-10-16", None)
        .unwrap()
        .unwrap();
    assert_eq!(without_films.chat_id, INBOX_ID);
}

#[test]
fn an_empty_day_and_a_day_with_only_trash_have_no_target() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    assert_eq!(
        store
            .forest_day_target("2026-10-16", Some(INBOX_ID))
            .unwrap(),
        None
    );
    let id = put(&store, INBOX_ID, "soon gone", ms(2026, 10, 16, 9, 0));
    assert!(
        store
            .forest_day_target("2026-10-16", None)
            .unwrap()
            .is_some()
    );
    store.delete_message(&id, &now()).unwrap();
    assert_eq!(store.forest_day_target("2026-10-16", None).unwrap(), None);
}

#[test]
fn garbage_dates_and_chat_ids_are_errors_or_ignored_never_a_panic() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    put(&store, INBOX_ID, "one", ms(2026, 10, 16, 9, 0));
    let huge = "я".repeat(1 << 20);
    for bad in [
        "",
        "2026-13-01",
        "2026-02-30",
        "16.10.2026",
        "0001-01-01",
        "2026-10-16\u{0}",
        huge.as_str(),
    ] {
        assert!(
            matches!(
                store.forest_day_target(bad, None),
                Err(StoreError::InvalidDate)
            ),
            "{:?}",
            bad.chars().take(12).collect::<String>()
        );
    }
    for chat in ["", "нет-такого", huge.as_str(), "'; DROP TABLE message; --"] {
        let target = store
            .forest_day_target("2026-10-16", Some(chat))
            .unwrap()
            .unwrap();
        assert_eq!(
            target.chat_id, INBOX_ID,
            "an unknown open chat falls back to the day's own chat"
        );
    }
}

#[test]
fn the_view_state_defaults_to_the_strip_and_survives_reopening() {
    let dir = tempfile::tempdir().unwrap();
    {
        let store = open_without_mirror(dir.path());
        let settings = store.get_settings().unwrap();
        assert_eq!(
            (settings.forest_view, settings.forest_mode),
            (ForestView::Strip, ForestMode::Forest)
        );
        store
            .set_forest_view(ForestView::Hidden, ForestMode::Calendar)
            .unwrap();
    }
    let store = open_without_mirror(dir.path());
    let settings = store.get_settings().unwrap();
    assert_eq!(
        (settings.forest_view, settings.forest_mode),
        (ForestView::Hidden, ForestMode::Calendar)
    );
    store
        .set_forest_view(ForestView::Expanded, ForestMode::Forest)
        .unwrap();
    let settings = store.get_settings().unwrap();
    assert_eq!(
        (settings.forest_view, settings.forest_mode),
        (ForestView::Expanded, ForestMode::Forest)
    );
}

#[test]
fn damaged_stored_view_state_reads_as_the_defaults() {
    let dir = tempfile::tempdir().unwrap();
    {
        let store = open_without_mirror(dir.path());
        store
            .set_forest_view(ForestView::Hidden, ForestMode::Calendar)
            .unwrap();
    }
    raw(dir.path())
        .execute_batch(
            "UPDATE app_setting SET value = 'sideways' WHERE key = 'forest_view'; \
             UPDATE app_setting SET value = '' WHERE key = 'forest_mode';",
        )
        .unwrap();
    let store = open_without_mirror(dir.path());
    let settings = store.get_settings().unwrap();
    assert_eq!(
        (settings.forest_view, settings.forest_mode),
        (ForestView::Strip, ForestMode::Forest)
    );
}

#[test]
fn the_view_state_is_exactly_two_app_setting_keys() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    store
        .set_forest_view(ForestView::Expanded, ForestMode::Calendar)
        .unwrap();
    let keys: Vec<String> = raw(dir.path())
        .prepare("SELECT key FROM app_setting WHERE key LIKE 'forest%' ORDER BY key")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(
        keys,
        ["forest_mode", "forest_view"],
        "two app_setting keys and nothing else"
    );
}
