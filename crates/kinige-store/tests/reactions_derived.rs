//! Slice 5a, the derived rows: rebuilt rows equal the incremental ones and a rules version that differs rebuilds on open, migration 10, and the groups of
//! a peer that are not well-formed.

#![cfg(test)]

mod common;

use common::ops::wait_closed;
use common::reactions::*;
use common::*;
use kinige_domain::groups::INBOX_ID;

const DEVICE: &str = "5f0c3e2a-9b1d-4c7e-8a64-0d2f1b3c4e5f";

fn stamp(l: u64) -> String {
    format!("{l:013}-00000-{DEVICE}")
}

#[test]
fn rebuilt_rows_equal_incremental_rows_and_a_rules_version_mismatch_rebuilds_on_open() {
    let dir = tempfile::tempdir().unwrap();
    {
        let store = open_without_mirror(dir.path());
        let first = send(&store, "one");
        let second = send(&store, "two");
        for (i, e) in ["😂", "🔥", HEART].iter().enumerate() {
            set(&store, &first.id, e, true, &later(i as u32 + 1));
        }
        set(&store, &first.id, "🔥", false, &later(5));
        set(&store, &second.id, "😢", true, &later(6));
    }
    wait_closed(dir.path());
    let incremental = all_rows(dir.path());
    assert_eq!((incremental.0.len(), incremental.1.len()), (3, 2));
    // The rows are lost (a restore of an older file, a bug): a version that differs rebuilds them all from the groups.
    raw(dir.path()).execute_batch("DELETE FROM message_reaction; DELETE FROM message_reaction_keys; UPDATE app_setting SET value = '0' WHERE key = 'reaction_rules_version';").unwrap();
    {
        let _store = open_without_mirror(dir.path());
    }
    wait_closed(dir.path());
    assert_eq!(all_rows(dir.path()), incremental);
    let version: String = raw(dir.path())
        .query_row(
            "SELECT value FROM app_setting WHERE key = 'reaction_rules_version'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(version, kinige_domain::reactions::RULES_VERSION.to_string());
    // With the version in place nothing is rebuilt on open: the rows that are there stay (the version is the only signal).
    raw(dir.path())
        .execute("DELETE FROM message_reaction", [])
        .unwrap();
    {
        let _store = open_without_mirror(dir.path());
    }
    wait_closed(dir.path());
    assert!(all_rows(dir.path()).0.is_empty());
}

#[test]
fn migration_ten_makes_the_rows_of_a_version_nine_database_and_a_fresh_one_has_the_tables() {
    let dir = tempfile::tempdir().unwrap();
    let id = {
        let store = open_without_mirror(dir.path());
        let sent = send(&store, "from before");
        set(&store, &sent.id, "🔥", true, &later(1));
        sent.id
    };
    let group_before = group(dir.path(), &id).unwrap();
    common::old_db::downgrade_to(dir.path(), 9);
    let has_table = |name: &str| {
        raw(dir.path())
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE name = ?1",
                [name],
                |r| r.get::<_, i64>(0),
            )
            .unwrap()
            == 1
    };
    assert!(!has_table("message_reaction") && !has_table("message_reaction_keys"));
    let store = open_without_mirror(dir.path());
    assert!(has_table("message_reaction") && has_table("message_reaction_keys"));
    assert_eq!(
        group(dir.path(), &id).unwrap(),
        group_before,
        "the group is not touched"
    );
    assert_eq!(rows(dir.path(), &id).len(), 1);
    let page = store.list_messages(INBOX_ID, None, 10, &later(2)).unwrap();
    assert_eq!(page.messages[0].reactions[0].emoji, "🔥");
    let fresh = tempfile::tempdir().unwrap();
    let _ = open_without_mirror(fresh.path());
    assert!(
        raw(fresh.path())
            .query_row("SELECT COUNT(*) FROM message_reaction", [], |r| r
                .get::<_, i64>(0))
            .is_ok()
    );
}

#[test]
fn an_older_peers_groups_that_are_not_well_formed_show_nothing_and_raise_no_error() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let page_of =
        |store: &kinige_store::Store| store.list_messages(INBOX_ID, None, 10, &later(1)).unwrap();
    let mut ids = Vec::new();
    let bad_groups = [
        (r#"{"items":"no"}"#.to_string(), 5, 1),
        (
            r#"{"items":[{"at":-1,"e":"🔥","h":"x","tz":"UTC"}]}"#.to_string(),
            5,
            1,
        ),
        (r#"{ "items": [] }"#.to_string(), 5, 1),
        (
            format!(
                r#"{{"items":[{{"at":1,"e":"🔥","h":"{}","tz":"UTC"}}]}}"#,
                stamp(9_999_999)
            ),
            5,
            1,
        ),
        (
            format!(
                r#"{{"items":[{{"at":1,"e":"🔥","h":"{}","tz":"UTC"}}]}}"#,
                stamp(5)
            ),
            5,
            2,
        ),
    ];
    for (i, (bytes, h, sv)) in bad_groups.iter().enumerate() {
        let sent = send(&store, &format!("entry {i}"));
        put_group(dir.path(), &sent.id, bytes, &stamp(*h), *sv);
        ids.push(sent.id);
    }
    drop(store);
    wait_closed(dir.path());
    raw(dir.path())
        .execute(
            "DELETE FROM app_setting WHERE key = 'reaction_rules_version'",
            [],
        )
        .unwrap();
    let store = open_without_mirror(dir.path());
    let page = page_of(&store);
    assert!(
        page.messages
            .iter()
            .all(|m| m.reactions.is_empty() && m.reaction_keys_had.is_empty())
    );
    assert_eq!(all_rows(dir.path()), (Vec::new(), Vec::new()));
}
