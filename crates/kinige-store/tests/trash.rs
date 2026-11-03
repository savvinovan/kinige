//! The trash (slice 3): the list, "N days left", "Delete permanently", auto-delete on day 31 (not on day 29),
//! tombstones with a snapshot of all groups, revisions and tags go away together with the entry.

// The integration test root is test code as a whole: `cfg(test)` gives helpers outside #[test] the same clippy.toml
// relaxations (unwrap/expect/panic/indexing) as the tests in src/.
#![cfg(test)]

mod common;

use common::ops::*;
use common::*;
use kinige_domain::ErrorCode;
use kinige_domain::groups::INBOX_ID;
use kinige_domain::history::DAY_MS;
use kinige_store::{Now, StoreError};
use serde_json::{Value, json};

fn at(ms: i64) -> Now {
    moscow(ms)
}

#[test]
fn the_trash_lists_deleted_entries_with_chat_date_and_days_left() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let books = store.create_chat("Книги", Some("📚"), &at(T0)).unwrap();
    let in_books = store
        .send_message(&books.id, "Дюна", &at(T0 + MINUTE))
        .unwrap();
    let in_inbox = store
        .send_message(INBOX_ID, "входящее", &at(T0 + 2 * MINUTE))
        .unwrap();
    store
        .delete_message(&in_books.id, &at(T0 + 10 * DAY_MS))
        .unwrap();
    store
        .delete_message(&in_inbox.id, &at(T0 + 11 * DAY_MS))
        .unwrap();

    let now = at(T0 + 12 * DAY_MS);
    let items = store.list_trash(&now).unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(
        items[0].message.id, in_inbox.id,
        "the most recently deleted comes first"
    );
    assert!(items[0].chat_is_inbox);
    assert_eq!(items[0].days_left, 29);
    assert_eq!(items[1].message.id, in_books.id);
    assert_eq!(
        (
            items[1].chat_title.as_str(),
            items[1].chat_icon.as_deref(),
            items[1].days_left
        ),
        ("Книги", Some("📚"), 28)
    );
    assert_eq!(items[1].message.body_md, "Дюна");
    assert_eq!(items[1].deleted_at, T0 + 10 * DAY_MS);
    assert_eq!(store.sidebar().unwrap().trash_count, 2);
    // Restoring removes the entry from the trash.
    store.restore_message(&in_books.id, &now).unwrap();
    assert_eq!(store.list_trash(&now).unwrap().len(), 1);
}

#[test]
fn purge_forever_removes_the_entry_its_revisions_and_tags_and_writes_tombstones() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let sent = store
        .send_message(INBOX_ID, "версия 1 #тег", &at(T0))
        .unwrap();
    add_foreign(dir.path(), &sent.id);
    store
        .edit_message(&sent.id, "версия 2 #тег", &at(T0 + MINUTE))
        .unwrap();
    store
        .edit_message(&sent.id, "версия 3", &at(T0 + 2 * MINUTE))
        .unwrap();
    let revisions_before: Vec<(String, String)> = raw(dir.path())
        .prepare("SELECT id, hlc_json FROM message_revision ORDER BY id")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(revisions_before.len(), 2);

    // A live entry cannot be deleted this way.
    let error = store
        .purge_message(&sent.id, &at(T0 + 3 * MINUTE))
        .unwrap_err();
    assert!(matches!(error, StoreError::NotInTrash));
    assert_eq!(error.code(), ErrorCode::NotInTrash);
    assert_eq!(count_rows(dir.path(), "message"), 1);

    store
        .delete_message(&sent.id, &at(T0 + 3 * MINUTE))
        .unwrap();
    // The snapshot of group HLCs is as the device sees them at the moment of purge (with the new `deleted` group).
    let message_hlcs: Value = serde_json::from_str(
        &raw(dir.path())
            .query_row(
                "SELECT hlc_json FROM message WHERE id = ?1",
                [&sent.id],
                |r| r.get::<_, String>(0),
            )
            .unwrap(),
    )
    .unwrap();
    store.purge_message(&sent.id, &at(T0 + 4 * MINUTE)).unwrap();

    assert_eq!(count_rows(dir.path(), "message"), 0);
    assert_eq!(
        count_rows(dir.path(), "message_revision"),
        0,
        "revisions are deleted together with the entry"
    );
    assert_eq!(count_rows(dir.path(), "message_tag"), 0);
    assert_eq!(store.sidebar().unwrap().trash_count, 0);
    assert!(store.list_trash(&at(T0 + 5 * MINUTE)).unwrap().is_empty());
    assert_eq!(
        store
            .purge_message(&sent.id, &at(T0 + 5 * MINUTE))
            .unwrap_err()
            .code(),
        ErrorCode::MessageNotFound
    );

    // The entry's tombstone: a snapshot of the HLCs of all groups, including the unknown `future`; the cascade root has no `cascade_of` and `held`.
    let conn = raw(dir.path());
    let (purged_of, purged_at, cascade_of, held): (String, i64, Option<String>, Option<String>) = conn
        .query_row(
            "SELECT purged_of, purged_at, cascade_of, held FROM tombstone WHERE entity = 'message' AND id = ?1",
            [&sent.id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    let snapshot: Value = serde_json::from_str(&purged_of).unwrap();
    let expected: serde_json::Map<String, Value> = message_hlcs
        .as_object()
        .unwrap()
        .iter()
        .map(|(group, entry)| (group.clone(), entry["h"].clone()))
        .collect();
    assert_eq!(snapshot, Value::Object(expected));
    assert!(
        snapshot.get("future").is_some(),
        "the unknown group is in the snapshot"
    );
    assert_eq!(snapshot.as_object().unwrap().len(), 6);
    assert_eq!(
        purged_of,
        String::from_utf8(kinige_domain::jcs::canonical_bytes(&snapshot).unwrap()).unwrap(),
        "the snapshot is canonical"
    );
    assert_eq!((purged_at, cascade_of, held), (T0 + 4 * MINUTE, None, None));

    // Revision tombstones: `cascade_of` = the entry id, the snapshot is their only group `core`, `held` is empty.
    for (revision_id, hlc_json) in &revisions_before {
        let (purged_of, cascade_of, held): (String, Option<String>, Option<String>) = conn
            .query_row(
                "SELECT purged_of, cascade_of, held FROM tombstone WHERE entity = 'message_revision' AND id = ?1",
                [revision_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        let h = serde_json::from_str::<Value>(hlc_json).unwrap()["core"]["h"].clone();
        assert_eq!(
            serde_json::from_str::<Value>(&purged_of).unwrap(),
            json!({"core": h})
        );
        assert_eq!(
            (cascade_of.as_deref(), held),
            (Some(sent.id.as_str()), None)
        );
    }
    assert_eq!(count_rows(dir.path(), "tombstone"), 3);
}

#[test]
fn entries_are_purged_on_day_31_and_not_on_day_29() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let old = store.send_message(INBOX_ID, "старая", &at(T0)).unwrap();
    let young = store.send_message(INBOX_ID, "молодая", &at(T0)).unwrap();
    let live = store.send_message(INBOX_ID, "живая", &at(T0)).unwrap();
    store
        .edit_message(&old.id, "старая, правка", &at(T0 + 1000))
        .unwrap();
    store.delete_message(&old.id, &at(T0 + 2000)).unwrap();
    store
        .delete_message(&young.id, &at(T0 + 2 * DAY_MS))
        .unwrap();

    // Day 29 for the old one: nothing is deleted.
    let report = store.run_maintenance(&at(T0 + 2000 + 29 * DAY_MS)).unwrap();
    assert_eq!(report.purged, 0);
    assert_eq!(count_rows(dir.path(), "message"), 3);
    assert_eq!(count_rows(dir.path(), "message_revision"), 1);
    assert_eq!(count_rows(dir.path(), "tombstone"), 0);
    assert_eq!(
        store.list_trash(&at(T0 + 2000 + 29 * DAY_MS)).unwrap()[1].days_left,
        1
    );

    // Exactly 30 days: the term is up; the "young" one is not yet.
    let report = store.run_maintenance(&at(T0 + 2000 + 30 * DAY_MS)).unwrap();
    assert_eq!(report.purged, 1);
    assert_eq!(count_rows(dir.path(), "message"), 2);
    assert_eq!(
        count_rows(dir.path(), "message_revision"),
        0,
        "revisions are deleted together with the entry"
    );
    assert_eq!(
        count_rows(dir.path(), "tombstone"),
        2,
        "the entry and its revision"
    );
    let ids: Vec<String> = raw(dir.path())
        .prepare("SELECT id FROM message ORDER BY id")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(!ids.contains(&old.id) && ids.contains(&young.id) && ids.contains(&live.id));

    // Day 31 for the "young" one.
    let report = store
        .run_maintenance(&at(T0 + 2 * DAY_MS + 31 * DAY_MS))
        .unwrap();
    assert_eq!(report.purged, 1);
    assert_eq!(count_rows(dir.path(), "message"), 1);
    assert_eq!(count_rows(dir.path(), "tombstone"), 3);
    // A repeat does nothing.
    assert_eq!(
        store
            .run_maintenance(&at(T0 + 100 * DAY_MS))
            .unwrap()
            .purged,
        0
    );
}

#[test]
fn a_restored_entry_is_not_purged_and_a_corrupt_one_does_not_stop_the_rest() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let restored = store.send_message(INBOX_ID, "вернули", &at(T0)).unwrap();
    let broken = store.send_message(INBOX_ID, "сломана", &at(T0)).unwrap();
    let fine = store.send_message(INBOX_ID, "цела", &at(T0)).unwrap();
    for id in [&restored.id, &broken.id, &fine.id] {
        store.delete_message(id, &at(T0 + 1000)).unwrap();
    }
    store.restore_message(&restored.id, &at(T0 + 2000)).unwrap();
    raw(dir.path())
        .execute(
            "UPDATE message SET hlc_json = '{' WHERE id = ?1",
            [&broken.id],
        )
        .unwrap();

    let result = store.purge_expired(&at(T0 + 40 * DAY_MS));
    assert!(result.is_err(), "a corrupted entry is reported as an error");
    let ids: Vec<String> = raw(dir.path())
        .prepare("SELECT id FROM message ORDER BY id")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(ids.contains(&restored.id), "the restored one is alive");
    assert!(
        ids.contains(&broken.id),
        "the corrupted one stayed in the trash"
    );
    assert!(!ids.contains(&fine.id), "the others are deleted");
}

#[test]
fn trash_is_not_part_of_the_markdown_copy_after_a_purge_either() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path());
    let sent = store.send_message(INBOX_ID, "в корзину", &at(T0)).unwrap();
    store
        .send_message(INBOX_ID, "остаётся", &at(T0 + MINUTE))
        .unwrap();
    let file = mirror_dir(dir.path()).join("Inbox/2026/2026-10-21.md");
    wait_until("file", || file.exists());
    store
        .delete_message(&sent.id, &at(T0 + 2 * MINUTE))
        .unwrap();
    wait_until("the entry left the copy", || {
        std::fs::read_to_string(&file).is_ok_and(|t| !t.contains("в корзину"))
    });
    store.purge_message(&sent.id, &at(T0 + 3 * MINUTE)).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(200));
    assert!(std::fs::read_to_string(&file).unwrap().contains("остаётся"));
    assert!(
        !std::fs::read_to_string(&file)
            .unwrap()
            .contains("в корзину")
    );
}
