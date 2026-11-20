//! Slice 5a, the rest of the write side: the caps (10 live, 32 different ever), a group over them that came from elsewhere, the recent list of the
//! device, a write that dies before its commit, and a reopen.

#![cfg(test)]

mod common;

use common::ops::{clock, groups as message_groups};
use common::reactions::*;
use common::*;
use kinige_domain::ErrorCode;
use kinige_domain::groups::INBOX_ID;
use kinige_store::{AttachFault, StoreError};
use serde_json::{Value, json};
use std::sync::Arc;

fn count(dir: &std::path::Path, sql: &str) -> i64 {
    raw(dir).query_row(sql, [], |r| r.get(0)).unwrap()
}

#[test]
fn the_eleventh_live_reaction_and_the_thirty_third_key_are_refused_and_nothing_is_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let sent = send(&store, "text");
    let many = emoji(0x1F600, 11);
    for e in &many[..10] {
        set(&store, &sent.id, e, true, &later(1));
    }
    let before = group(dir.path(), &sent.id);
    let error = store
        .set_message_reaction(&sent.id, &many[10], true, &later(2))
        .unwrap_err();
    assert_eq!(error.code(), ErrorCode::ReactionLimit);
    assert_eq!(group(dir.path(), &sent.id), before);
    assert_eq!(rows(dir.path(), &sent.id).len(), 10);
    // A live one still toggles; with room again the eleventh goes in.
    set(&store, &sent.id, &many[0], false, &later(3));
    set(&store, &sent.id, &many[10], true, &later(4));

    // The key cap: 32 different emoji ever, most of them removed.
    let other = send(&store, "second");
    let keys = emoji(0x1F300, 33);
    for e in &keys[..32] {
        set(&store, &other.id, e, true, &later(5));
        set(&store, &other.id, e, false, &later(6));
    }
    let view = store.list_messages(INBOX_ID, None, 10, &later(7)).unwrap();
    let shown = view.messages.iter().find(|m| m.id == other.id).unwrap();
    assert_eq!(
        (shown.reactions.len(), shown.reaction_keys_had.len()),
        (0, 32)
    );
    let before = group(dir.path(), &other.id);
    let error = store
        .set_message_reaction(&other.id, &keys[32], true, &later(8))
        .unwrap_err();
    assert_eq!(error.code(), ErrorCode::ReactionLimit);
    assert_eq!(group(dir.path(), &other.id), before);
    let back = set(&store, &other.id, &keys[3], true, &later(9));
    assert_eq!(
        back.reactions.len(),
        1,
        "a key the entry has had can come back"
    );
}

#[test]
fn a_group_over_the_caps_from_elsewhere_is_shown_in_full_refuses_a_put_and_allows_a_removal() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let sent = send(&store, "text");
    let items: Vec<Value> = emoji(0x1F600, 12)
        .iter()
        .map(|e| json!({"at": later(1).ms, "e": e, "h": "0000000000001-00000-5f0c3e2a-9b1d-4c7e-8a64-0d2f1b3c4e5f", "tz": "UTC"}))
        .collect();
    let bytes = kinige_domain::jcs::canonical_string(&json!({ "items": items })).unwrap();
    // The HLC of the group is above the items' (they are from "another device" with an old clock).
    set(&store, &sent.id, HEART, true, &later(1));
    let h = hlc_of(dir.path(), &sent.id, "reactions");
    put_group(dir.path(), &sent.id, &bytes, &h, 1);
    drop(store);
    let store = open_without_mirror(dir.path());
    raw(dir.path())
        .execute(
            "DELETE FROM app_setting WHERE key = 'reaction_rules_version'",
            [],
        )
        .unwrap();
    drop(store);
    let store = open_without_mirror(dir.path());
    let view = store.list_messages(INBOX_ID, None, 10, &later(2)).unwrap();
    assert_eq!(view.messages[0].reactions.len(), 12, "shown in full");
    let error = store
        .set_message_reaction(&sent.id, "🎉", true, &later(3))
        .unwrap_err();
    assert_eq!(error.code(), ErrorCode::ReactionLimit);
    let view = set(&store, &sent.id, &emoji(0x1F600, 1)[0], false, &later(4));
    assert_eq!(view.reactions.len(), 11);
    assert_eq!(view.reaction_keys_had.len(), 12, "nothing was trimmed");
}

#[test]
fn the_recent_list_goes_by_use_is_capped_at_eight_and_a_removal_does_not_touch_it() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let sent = send(&store, "text");
    assert_eq!(
        store.get_settings().unwrap().quick_reactions,
        [HEART, "😂", "😢"]
    );
    set(&store, &sent.id, HEART, true, &later(1));
    set(&store, &sent.id, "🔥", true, &later(2));
    let settings = store.get_settings().unwrap();
    assert_eq!(
        settings.quick_reactions,
        ["🔥", HEART, "😂"],
        "the latest first, then the fixed ones without repeats"
    );
    assert_eq!(settings.recent_reactions, ["🔥", HEART]);
    set(&store, &sent.id, "🔥", false, &later(3));
    assert_eq!(
        store.get_settings().unwrap().recent_reactions,
        ["🔥", HEART],
        "a removal does not touch the list"
    );
    set(&store, &sent.id, "🔥", true, &later(4));
    // Putting a live reaction again writes nothing, so the order of the list is the order of the writes.
    set(&store, &sent.id, HEART, true, &later(5));
    assert_eq!(
        store.get_settings().unwrap().recent_reactions,
        ["🔥", HEART]
    );

    let other = send(&store, "second");
    for e in emoji(0x1F600, 10) {
        set(&store, &other.id, &e, true, &later(6));
        set(&store, &other.id, &e, false, &later(7));
    }
    let recent = store.get_settings().unwrap().recent_reactions;
    assert_eq!(recent.len(), 8);
    assert_eq!(recent[0], emoji(0x1F600, 10)[9]);

    // A broken stored value is an empty list: the defaults come back.
    raw(dir.path())
        .execute(
            "UPDATE app_setting SET value = '{broken' WHERE key = 'recent_reactions'",
            [],
        )
        .unwrap();
    let settings = store.get_settings().unwrap();
    assert!(settings.recent_reactions.is_empty());
    assert_eq!(settings.quick_reactions, [HEART, "😂", "😢"]);
    // The list is local: nothing of it is in the group or in `settings.json`.
    assert!(!group(dir.path(), &other.id).unwrap().contains("recent"));
}

#[test]
fn a_write_that_dies_before_its_commit_leaves_no_half_state() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let sent = send(&store, "text");
    set(&store, &sent.id, HEART, true, &later(1));
    let snapshot = |dir: &std::path::Path| {
        (
            message_groups(dir, "message", &sent.id),
            clock(dir),
            all_rows(dir),
            count(dir, "SELECT COUNT(*) FROM mirror_dirty"),
            raw(dir)
                .query_row(
                    "SELECT value FROM app_setting WHERE key = 'recent_reactions'",
                    [],
                    |r| r.get::<_, String>(0),
                )
                .unwrap(),
        )
    };
    let before = snapshot(dir.path());
    store.inject_attach_fault_for_test(AttachFault::ReactionBeforeCommit(Arc::new(|| {
        panic!("injected: the process dies before the commit")
    })));
    let error = store
        .set_message_reaction(&sent.id, "🔥", true, &later(2))
        .unwrap_err();
    assert!(matches!(error, StoreError::Internal(_)), "{error:?}");
    store.inject_attach_fault_for_test(AttachFault::Clear);
    assert_eq!(
        snapshot(dir.path()),
        before,
        "the transaction rolled back: group, rows, clock, copy mark and recent list"
    );
    // The store still works, and the same write now goes through.
    let view = set(&store, &sent.id, "🔥", true, &later(3));
    assert_eq!(view.reactions.len(), 2);
}

#[test]
fn the_reactions_and_their_times_survive_a_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let id = {
        let store = open_without_mirror(dir.path());
        let sent = send(&store, "text");
        set(&store, &sent.id, HEART, true, &later(1));
        set(&store, &sent.id, "🔥", true, &later(2));
        sent.id
    };
    common::ops::wait_closed(dir.path());
    let store = open_without_mirror(dir.path());
    let page = store.list_messages(INBOX_ID, None, 10, &later(3)).unwrap();
    let got = &page.messages.iter().find(|m| m.id == id).unwrap().reactions;
    let pairs: Vec<_> = got.iter().map(|r| (r.emoji.as_str(), r.put_at)).collect();
    assert_eq!(pairs, [(HEART, later(1).ms), ("🔥", later(2).ms)]);
}
