//! Feed pages around an entry and newer ones (slice 4, task 4.10): walking up and down yields every entry once, the boundaries,
//! a tag, the trash and a foreign chat.

// The root of the integration tests is entirely test code: `cfg(test)` gives helpers outside #[test] the same
// clippy.toml relaxations (unwrap/expect/panic/indexing) as the tests in src/.
#![cfg(test)]

mod common;

use common::*;
use kinige_domain::groups::INBOX_ID;
use kinige_domain::{FeedCursor, MessagePage};
use kinige_store::{PageAt, Store, StoreError};

const DAY: i64 = 24 * 60 * MINUTE;

/// `n` Inbox entries, by ascending time: each in its own day and its own minute; the even ones carry the tag `#чет`. The result is the ids in
/// ascending feed order.
fn fill(store: &Store, n: usize) -> Vec<String> {
    (0..n)
        .map(|i| {
            let tag = if i % 2 == 0 { " #чет" } else { "" };
            store
                .send_message(
                    INBOX_ID,
                    &format!("запись {i}{tag}"),
                    &moscow(T0 + (i as i64 / 3) * DAY + (i as i64) * MINUTE),
                )
                .unwrap()
                .id
        })
        .collect()
}

fn ids(page: &MessagePage) -> Vec<String> {
    page.messages.iter().map(|m| m.id.clone()).collect()
}

fn page(store: &Store, at: PageAt<'_>, limit: usize, tag: Option<&str>) -> MessagePage {
    store
        .list_messages_page(INBOX_ID, at, limit, tag, &moscow(T0 + 40 * DAY))
        .unwrap()
}

#[test]
fn latest_and_before_pages_are_what_list_messages_returns() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let all = fill(&store, 12);
    let now = moscow(T0 + 40 * DAY);
    let latest = store.list_messages(INBOX_ID, None, 5, &now).unwrap();
    assert_eq!(ids(&latest), all[7..]);
    assert_eq!(
        latest.newer, None,
        "there is nothing newer than the newest page"
    );
    let older = latest.older.clone().expect("there is an older page");
    assert_eq!(older.id, all[7]);
    let before = store
        .list_messages(INBOX_ID, Some(&older), 5, &now)
        .unwrap();
    assert_eq!(ids(&before), all[2..7]);
    assert_eq!(
        before.newer.as_ref().map(|c| c.id.as_str()),
        Some(all[6].as_str()),
        "a page from the middle knows there is something newer"
    );
    let last = store
        .list_messages(INBOX_ID, Some(&before.older.clone().unwrap()), 5, &now)
        .unwrap();
    assert_eq!(ids(&last), all[..2]);
    assert_eq!(last.older, None);
}

fn walk(store: &Store, from: &MessagePage, limit: usize, tag: Option<&str>) -> Vec<String> {
    // Up from the page, then down from it; in total every entry exactly once.
    let mut up: Vec<Vec<String>> = Vec::new();
    let mut cursor = from.older.clone();
    while let Some(c) = cursor {
        let p = page(store, PageAt::Before(&c), limit, tag);
        up.push(ids(&p));
        cursor = p.older;
    }
    let mut down: Vec<String> = Vec::new();
    let mut cursor = from.newer.clone();
    while let Some(c) = cursor {
        let p = page(store, PageAt::After(&c), limit, tag);
        down.extend(ids(&p));
        cursor = p.newer;
    }
    let mut all: Vec<String> = up.into_iter().rev().flatten().collect();
    all.extend(ids(from));
    all.extend(down);
    all
}

#[test]
fn walking_up_and_down_from_the_middle_visits_every_record_once() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let all = fill(&store, 31);
    for limit in [1, 2, 4, 7, 10, 31, 40] {
        let around = page(&store, PageAt::Around(&all[15]), limit, None);
        assert!(ids(&around).contains(&all[15]), "limit {limit}");
        assert!(around.messages.len() <= limit);
        assert_eq!(walk(&store, &around, limit, None), all, "limit {limit}");
    }
}

#[test]
fn around_gives_half_before_and_half_after_the_record() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let all = fill(&store, 30);
    let around = page(&store, PageAt::Around(&all[15]), 11, None);
    assert_eq!(
        ids(&around),
        all[10..21],
        "five before, the entry, five after"
    );
    assert_eq!(
        around.older.as_ref().map(|c| c.id.clone()),
        Some(all[10].clone())
    );
    assert_eq!(
        around.newer.as_ref().map(|c| c.id.clone()),
        Some(all[20].clone())
    );
    // An even limit: one fewer before the entry than after it.
    let around = page(&store, PageAt::Around(&all[15]), 10, None);
    assert_eq!(ids(&around), all[11..21]);
}

#[test]
fn page_borders_have_no_cursor_outward() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let all = fill(&store, 9);
    let first = page(&store, PageAt::Around(&all[0]), 5, None);
    assert_eq!(ids(&first), all[..3]);
    assert_eq!(first.older, None);
    assert!(first.newer.is_some());
    let last = page(&store, PageAt::Around(&all[8]), 5, None);
    assert_eq!(ids(&last), all[6..]);
    assert_eq!(last.newer, None);
    assert!(last.older.is_some());
    // One entry in the chat: a page of that one entry, no cursors.
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let only = fill(&store, 1);
    let single = page(&store, PageAt::Around(&only[0]), 5, None);
    assert_eq!(ids(&single), only);
    assert_eq!((single.older, single.newer), (None, None));
}

#[test]
fn after_the_last_record_the_page_is_empty() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let all = fill(&store, 5);
    let latest = page(&store, PageAt::Latest, 10, None);
    let last = latest.messages.last().unwrap();
    let cursor = FeedCursor {
        journal_date: last.journal_date.clone(),
        created_at: last.created_at,
        id: last.id.clone(),
    };
    let empty = page(&store, PageAt::After(&cursor), 10, None);
    assert!(empty.messages.is_empty());
    assert_eq!((empty.older, empty.newer), (None, None));
    assert_eq!(all.len(), 5);
}

#[test]
fn tag_filter_applies_on_both_sides_of_around() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let all = fill(&store, 20);
    let even: Vec<String> = all.iter().step_by(2).cloned().collect();
    let around = page(&store, PageAt::Around(&all[10]), 5, Some("чет"));
    assert_eq!(
        ids(&around),
        even[3..8],
        "the page has only entries with the tag"
    );
    assert_eq!(walk(&store, &around, 3, Some("чет")), even);
    // An entry without the tag is not found under the filter of that tag.
    let error = store
        .list_messages_page(
            INBOX_ID,
            PageAt::Around(&all[11]),
            5,
            Some("чет"),
            &moscow(T0),
        )
        .unwrap_err();
    assert!(matches!(error, StoreError::MessageNotFound));
}

#[test]
fn around_a_record_of_another_chat_or_in_trash_is_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let all = fill(&store, 6);
    let other = store.create_chat("Другой", None, &moscow(T0)).unwrap();
    let moved = store
        .move_message(&all[2], &other.id, &moscow(T0 + MINUTE))
        .unwrap();
    store
        .delete_message(&all[4], &moscow(T0 + 2 * MINUTE))
        .unwrap();
    let now = moscow(T0 + 40 * DAY);
    for id in [all[2].as_str(), all[4].as_str(), "нет-такой"] {
        let error = store
            .list_messages_page(INBOX_ID, PageAt::Around(id), 5, None, &now)
            .unwrap_err();
        assert!(matches!(error, StoreError::MessageNotFound), "{id}");
    }
    // Found in its own chat, and the page contains neither a moved nor a deleted entry.
    let around = store
        .list_messages_page(INBOX_ID, PageAt::Around(&all[3]), 10, None, &now)
        .unwrap();
    assert_eq!(
        ids(&around),
        [
            all[0].clone(),
            all[1].clone(),
            all[3].clone(),
            all[5].clone()
        ]
    );
    let own = store
        .list_messages_page(&other.id, PageAt::Around(&moved.id), 5, None, &now)
        .unwrap();
    assert_eq!(ids(&own), [moved.id]);
}
