//! Single-entry actions (slice 3): delete and restore, edit with a revision, a task item, flags, the entry's day.
//! Properties common to all rewrites (verbatim foreign data, HLC, corruption, write failure, read-only) are in `ops.rs`.

// The integration test root is test code as a whole: `cfg(test)` gives helpers outside #[test] the same clippy.toml
// relaxations (unwrap/expect/panic/indexing) as the tests in src/.
#![cfg(test)]

mod common;

use common::ops::*;
use common::*;
use kinige_domain::groups::INBOX_ID;
use kinige_domain::{ErrorCode, MessageFlag};
use kinige_store::{Store, StoreError};
use serde_json::json;
use std::path::Path;

fn at(minutes: i64) -> kinige_store::Now {
    moscow(T0 + minutes * MINUTE)
}

const TEXT: &str = "Идея #заметка\n- [ ] дело";

fn setup(dir: &Path) -> (Store, String) {
    let store = open_without_mirror(dir);
    let sent = store.send_message(INBOX_ID, TEXT, &at(0)).unwrap();
    (store, sent.id)
}

#[test]
fn delete_and_restore_move_the_counter_and_the_feed_and_are_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let (store, id) = setup(dir.path());
    let other = store.send_message(INBOX_ID, "вторая", &at(1)).unwrap();
    assert_eq!(store.list_chats().unwrap()[0].count, 2);

    store.delete_message(&id, &at(2)).unwrap();
    assert_eq!(
        store.list_chats().unwrap()[0].count,
        1,
        "the counter dropped by 1"
    );
    assert_eq!(store.sidebar().unwrap().trash_count, 1);
    let page = store.list_messages(INBOX_ID, None, 10, &at(3)).unwrap();
    assert_eq!(
        page.messages
            .iter()
            .map(|m| m.id.as_str())
            .collect::<Vec<_>>(),
        [other.id.as_str()]
    );
    let clock_after_delete = clock(dir.path());
    store.delete_message(&id, &at(9)).unwrap();
    assert_eq!(
        clock(dir.path()),
        clock_after_delete,
        "a repeated delete writes nothing"
    );
    assert_eq!(
        columns(dir.path(), &id)["deleted_at"],
        json!(T0 + 2 * MINUTE)
    );

    store.restore_message(&id, &at(4)).unwrap();
    assert_eq!(store.list_chats().unwrap()[0].count, 2);
    assert_eq!(store.sidebar().unwrap().trash_count, 0);
    let page = store.list_messages(INBOX_ID, None, 10, &at(5)).unwrap();
    assert_eq!(
        page.messages
            .iter()
            .map(|m| m.id.clone())
            .collect::<Vec<_>>(),
        [id.clone(), other.id],
        "the entry returned to its place"
    );
    store.restore_message(&id, &at(6)).unwrap();
    assert_eq!(
        store.restore_message("нет", &at(6)).unwrap_err().code(),
        ErrorCode::MessageNotFound
    );
}

#[test]
fn edit_keeps_the_entry_in_place_and_saves_the_old_version_as_a_revision() {
    let dir = tempfile::tempdir().unwrap();
    let (store, id) = setup(dir.path());
    add_foreign(dir.path(), &id);
    let first = store.send_message(INBOX_ID, "до", &at(-5)).unwrap();
    let before = groups(dir.path(), "message", &id);
    let hlcs_before = hlc_entries(dir.path(), "message", &id);

    let edited = store
        .edit_message(&id, "\n\nНовый текст #новый \r\n", &at(3))
        .unwrap();
    assert_eq!(edited.body_md, "Новый текст #новый");
    assert_eq!(edited.tags, ["новый"]);
    assert_eq!(
        (edited.created_at, edited.journal_date.as_str()),
        (T0, "2026-10-21")
    );

    // The entry is in the same place and has the same time.
    let page = store.list_messages(INBOX_ID, None, 10, &at(4)).unwrap();
    assert_eq!(
        page.messages
            .iter()
            .map(|m| m.id.clone())
            .collect::<Vec<_>>(),
        [first.id, id.clone()]
    );
    assert_eq!(
        columns(dir.path(), &id)["edited_at"],
        json!(T0 + 3 * MINUTE)
    );

    // Exactly one `edit` revision with a snapshot of the entry before the edit: the verbatim bytes of all groups with HLC and sv.
    assert_eq!(revisions(dir.path(), &id), 1);
    let (snapshot, reason, saved_at, revision_hlc): (String, String, i64, String) = raw(dir.path())
        .query_row(
            "SELECT snapshot_json, reason, saved_at, hlc_json FROM message_revision WHERE message_id = ?1",
            [&id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!(reason, "edit");
    assert_eq!(saved_at, T0 + 3 * MINUTE);
    let snapshot: serde_json::Value = serde_json::from_str(&snapshot).unwrap();
    let entries = snapshot["groups"].as_array().unwrap();
    assert_eq!(entries.len(), before.len());
    for entry in entries {
        let name = entry["g"].as_str().unwrap();
        assert_eq!(
            entry["raw"].as_str().unwrap(),
            before[name],
            "{name}: the bytes before the edit are verbatim"
        );
        let hlc_entry: serde_json::Value = serde_json::from_str(&hlcs_before[name]).unwrap();
        assert_eq!(entry["h"], hlc_entry["h"], "{name}");
        assert_eq!(entry["sv"], hlc_entry["sv"], "{name}");
    }
    // The revision itself is an entry with only the `core` group; its HLC is the stamp of the edit event.
    let revision_groups = groups(
        dir.path(),
        "message_revision",
        &revision_id(dir.path(), &id),
    );
    assert_eq!(revision_groups.keys().collect::<Vec<_>>(), ["core"]);
    let body_hlc = group_hlc(dir.path(), "message", &id, "body");
    assert_eq!(
        revision_hlc,
        format!(r#"{{"core":{{"h":"{body_hlc}","sv":1}}}}"#)
    );
    assert_columns_match_groups(dir.path(), &id);
}

fn revision_id(dir: &Path, message_id: &str) -> String {
    raw(dir)
        .query_row(
            "SELECT id FROM message_revision WHERE message_id = ?1",
            [message_id],
            |r| r.get(0),
        )
        .unwrap()
}

#[test]
fn edit_with_the_same_empty_or_oversized_text_writes_no_revision() {
    let dir = tempfile::tempdir().unwrap();
    let (store, id) = setup(dir.path());
    let clock_before = clock(dir.path());
    store.edit_message(&id, TEXT, &at(1)).unwrap();
    assert_eq!(revisions(dir.path(), &id), 0, "the same text, no revision");
    assert_eq!(clock(dir.path()), clock_before);
    for text in ["", "  \n\t "] {
        assert!(matches!(
            store.edit_message(&id, text, &at(1)),
            Err(StoreError::Empty)
        ));
    }
    let huge = "я".repeat(300 * 1024);
    assert!(matches!(
        store.edit_message(&id, &huge, &at(1)),
        Err(StoreError::TooLarge)
    ));
    assert_eq!(revisions(dir.path(), &id), 0);
    assert_eq!(columns(dir.path(), &id)["body_md"], json!(TEXT));
}

#[test]
fn edit_branches_of_base_and_supersedes_follow_the_confirmation() {
    let dir = tempfile::tempdir().unwrap();
    let (store, id) = setup(dir.path());
    let h0 = group_hlc(dir.path(), "message", &id, "body");
    // Its own unconfirmed version: base stays null, the previous HLC goes into supersedes.
    store.edit_message(&id, "раз", &at(1)).unwrap();
    let c = columns(dir.path(), &id);
    assert_eq!(
        (c["body_base"].clone(), c["body_supersedes"].clone()),
        (json!(null), json!(format!(r#"["{h0}"]"#)))
    );
    // The server confirmed the version: base = its HLC, supersedes is empty.
    let h1 = group_hlc(dir.path(), "message", &id, "body");
    raw(dir.path())
        .execute("UPDATE message SET server_known = 1 WHERE id = ?1", [&id])
        .unwrap();
    store.edit_message(&id, "два", &at(2)).unwrap();
    let c = columns(dir.path(), &id);
    assert_eq!(
        (c["body_base"].clone(), c["body_supersedes"].clone()),
        (json!(h1), json!("[]"))
    );
    assert_columns_match_groups(dir.path(), &id);
}

#[test]
fn task_item_toggle_is_a_text_edit_with_a_revision_and_stale_requests_fail() {
    let dir = tempfile::tempdir().unwrap();
    let (store, id) = setup(dir.path());
    let done = store.toggle_task_item(&id, 0, true, &at(1)).unwrap();
    assert_eq!(done.body_md, "Идея #заметка\n- [x] дело");
    assert_eq!(revisions(dir.path(), &id), 1, "the old version is kept");
    let undone = store.toggle_task_item(&id, 0, false, &at(2)).unwrap();
    assert_eq!(undone.body_md, TEXT);
    assert_eq!(revisions(dir.path(), &id), 2);

    let clock_before = clock(dir.path());
    for (index, checked) in [(0, false), (1, true), (5, false)] {
        let error = store
            .toggle_task_item(&id, index, checked, &at(3))
            .unwrap_err();
        assert!(
            matches!(error, StoreError::TaskItemStale),
            "{index} {checked}"
        );
        assert_eq!(error.code(), ErrorCode::TaskItemStale);
    }
    assert_eq!(clock(dir.path()), clock_before);
    assert_eq!(revisions(dir.path(), &id), 2);
}

#[test]
fn flags_task_done_favorite_and_no_ops() {
    let dir = tempfile::tempdir().unwrap();
    let (store, id) = setup(dir.path());
    // "Done" on an entry that is not a task is an error, while clearing it is a quiet no-op.
    assert!(
        store
            .set_message_flag(&id, MessageFlag::TaskDone, true, &at(1))
            .is_err()
    );
    store
        .set_message_flag(&id, MessageFlag::TaskDone, false, &at(1))
        .unwrap();

    let view = store
        .set_message_flag(&id, MessageFlag::Task, true, &at(2))
        .unwrap();
    assert!(view.is_task && view.task_done_at.is_none());
    let view = store
        .set_message_flag(&id, MessageFlag::TaskDone, true, &at(3))
        .unwrap();
    assert_eq!(view.task_done_at, Some(T0 + 3 * MINUTE));
    let view = store
        .set_message_flag(&id, MessageFlag::Favorite, true, &at(4))
        .unwrap();
    assert_eq!(view.favorite_at, Some(T0 + 4 * MINUTE));
    let clock_before = clock(dir.path());
    store
        .set_message_flag(&id, MessageFlag::Favorite, true, &at(9))
        .unwrap();
    assert_eq!(
        clock(dir.path()),
        clock_before,
        "the same state, nothing is written"
    );
    // Removing the task resets "done", the favorite stays.
    let view = store
        .set_message_flag(&id, MessageFlag::Task, false, &at(5))
        .unwrap();
    assert!(!view.is_task && view.task_done_at.is_none());
    assert_eq!(view.favorite_at, Some(T0 + 4 * MINUTE));
    assert_eq!(
        groups(dir.path(), "message", &id)["flags"],
        format!(
            r#"{{"favorite_at":{},"is_task":false,"pinned_at":null,"task_done_at":null}}"#,
            T0 + 4 * MINUTE
        )
    );
    assert_eq!(
        revisions(dir.path(), &id),
        0,
        "flags do not write revisions"
    );
}

#[test]
fn changing_the_date_moves_the_entry_between_days_in_both_branches() {
    let dir = tempfile::tempdir().unwrap();
    let (store, id) = setup(dir.path());
    let h0 = group_hlc(dir.path(), "message", &id, "placement");

    let moved = store.set_message_date(&id, "2026-10-20", &at(1)).unwrap();
    assert_eq!(moved.journal_date, "2026-10-20");
    assert_eq!(moved.created_at, T0, "the creation time does not change");
    let c = columns(dir.path(), &id);
    assert_eq!(c["placement_base"], json!(null));
    assert_eq!(c["placement_supersedes"], json!(format!(r#"["{h0}"]"#)));
    assert_eq!(
        groups(dir.path(), "message", &id)["placement"],
        format!(
            r#"{{"base":null,"chat_id":"{INBOX_ID}","journal_date":"2026-10-20","supersedes":["{h0}"]}}"#
        )
    );

    let h1 = group_hlc(dir.path(), "message", &id, "placement");
    raw(dir.path())
        .execute("UPDATE message SET server_known = 1 WHERE id = ?1", [&id])
        .unwrap();
    store.set_message_date(&id, "2099-01-31", &at(2)).unwrap();
    let c = columns(dir.path(), &id);
    assert_eq!(
        (
            c["placement_base"].clone(),
            c["placement_supersedes"].clone()
        ),
        (json!(h1), json!("[]"))
    );
    assert_eq!(
        c["journal_date"],
        json!("2099-01-31"),
        "a future date is allowed"
    );

    for bad in ["2026-02-30", "вчера", "", "2026-13-01"] {
        assert!(
            matches!(
                store.set_message_date(&id, bad, &at(3)),
                Err(StoreError::InvalidDate)
            ),
            "{bad}"
        );
    }
    // The same date, nothing is written.
    let clock_before = clock(dir.path());
    store.set_message_date(&id, "2099-01-31", &at(4)).unwrap();
    assert_eq!(clock(dir.path()), clock_before);
    assert_columns_match_groups(dir.path(), &id);
    assert_eq!(
        revisions(dir.path(), &id),
        0,
        "a move does not write revisions"
    );
}
