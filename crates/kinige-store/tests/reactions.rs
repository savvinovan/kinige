//! Slice 5a: putting and removing a reaction in the store. The group, its HLC, the derived rows, what is not touched (the other groups,
//! the revisions, `edited_at`, the favorite), the input rules and the caps, the recent list, and a write that dies before its commit.

#![cfg(test)]

mod common;

use common::days::at;
use common::ops::{clock, groups as message_groups};
use common::reactions::*;
use common::*;
use kinige_domain::ErrorCode;
use kinige_domain::MessageFlag;
use kinige_domain::groups::INBOX_ID;
use kinige_store::{AttachFault, StoreError};
use serde_json::{Value, json};
use std::sync::Arc;

fn count(dir: &std::path::Path, sql: &str) -> i64 {
    raw(dir).query_row(sql, [], |r| r.get(0)).unwrap()
}

#[test]
fn the_first_put_adds_the_group_above_every_hlc_and_shows_the_chip_without_an_edit() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let sent = send(&store, "Passed the exam!");
    store
        .set_message_flag(&sent.id, MessageFlag::Favorite, true, &later(1))
        .unwrap();
    assert!(
        group(dir.path(), &sent.id).is_none(),
        "an entry starts without the group"
    );
    let before = message_groups(dir.path(), "message", &sent.id);

    let view = set(&store, &sent.id, HEART, true, &later(2));
    assert_eq!(view.reactions.len(), 1);
    assert_eq!(view.reactions[0].emoji, HEART);
    assert_eq!(view.reactions[0].put_at, later(2).ms);
    assert_eq!(view.reactions[0].put_tz, "Europe/Moscow");
    assert_eq!(view.reactions[0].years_later, 0);
    assert_eq!(view.reaction_keys_had, 1);

    // The other groups are byte-identical, and the new group's HLC is above all of them.
    let after = message_groups(dir.path(), "message", &sent.id);
    for (name, bytes) in &before {
        assert_eq!(&after[name], bytes, "group {name}");
    }
    assert_eq!(after.len(), before.len() + 1);
    let new_hlc = hlc_of(dir.path(), &sent.id, "reactions");
    for name in before.keys() {
        assert!(hlc_of(dir.path(), &sent.id, name) < new_hlc, "{name}");
    }
    let item = &group_json(dir.path(), &sent.id)["items"][0];
    assert_eq!(item["h"], json!(new_hlc));
    assert_eq!(item["at"], json!(later(2).ms));
    assert_eq!(item["tz"], "Europe/Moscow");

    // Not an edit: no revision, no `edited_at`, the favorite is where it was.
    assert_eq!(
        count(dir.path(), "SELECT COUNT(*) FROM message_revision"),
        0
    );
    let (edited, favorite): (Option<i64>, Option<i64>) = raw(dir.path())
        .query_row(
            "SELECT edited_at, favorite_at FROM message WHERE id = ?1",
            [&sent.id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!((edited, favorite), (None, Some(later(1).ms)));
    assert_eq!(rows(dir.path(), &sent.id).len(), 1);
}

#[test]
fn a_put_of_a_live_emoji_writes_nothing_and_a_removal_leaves_a_marker_and_a_new_put_a_new_time() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let sent = send(&store, "text");
    set(&store, &sent.id, HEART, true, &later(1));
    let bytes = group(dir.path(), &sent.id);
    let clock_before = clock(dir.path());
    // Again, and in the other form of the same emoji: the time is kept and nothing is written.
    set(&store, &sent.id, HEART, true, &later(9));
    let view = set(&store, &sent.id, "\u{2764}", true, &later(9));
    assert_eq!(view.reactions[0].put_at, later(1).ms);
    assert_eq!(
        (group(dir.path(), &sent.id), clock(dir.path())),
        (bytes, clock_before.clone())
    );
    // Removing something the entry never had writes nothing either.
    set(&store, &sent.id, "🔥", false, &later(9));
    assert_eq!(clock(dir.path()), clock_before);

    let removed = set(&store, &sent.id, HEART, false, &later(10));
    assert!(removed.reactions.is_empty());
    assert_eq!(removed.reaction_keys_had, 1, "the marker still counts");
    assert!(
        rows(dir.path(), &sent.id).is_empty(),
        "the derived row is gone"
    );
    let item = &group_json(dir.path(), &sent.id)["items"][0];
    assert_eq!(
        (item["at"].clone(), item["tz"].clone()),
        (Value::Null, Value::Null)
    );
    assert_eq!(item["h"], json!(hlc_of(dir.path(), &sent.id, "reactions")));

    let again = set(&store, &sent.id, HEART, true, &later(20));
    assert_eq!(
        again.reactions[0].put_at,
        later(20).ms,
        "putting it back gives the new time"
    );
    assert_eq!(again.reaction_keys_had, 1);
}

#[test]
fn a_put_changes_one_item_and_every_other_item_stays_byte_identical() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let sent = send(&store, "text");
    for (i, e) in ["😂", "🔥", "😢"].iter().enumerate() {
        set(&store, &sent.id, e, true, &later(i as u32 + 1));
    }
    let items = |dir: &std::path::Path| -> Vec<(String, String)> {
        group_json(dir, &sent.id)["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| (i["e"].as_str().unwrap().to_string(), i.to_string()))
            .collect()
    };
    let before = items(dir.path());
    set(&store, &sent.id, "🔥", false, &later(5));
    set(&store, &sent.id, "🎉", true, &later(6));
    let after = items(dir.path());
    assert_eq!(after.len(), before.len() + 1);
    for (emoji, bytes) in &before {
        let now = after.iter().find(|(e, _)| e == emoji).unwrap();
        if emoji == "🔥" {
            assert_ne!(&now.1, bytes, "the changed item");
        } else {
            assert_eq!(&now.1, bytes, "{emoji} is untouched");
        }
    }
}

#[test]
fn rewriting_flags_keeps_the_reactions_bytes_verbatim_with_the_reference_vector() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let sent = send(&store, "text");
    set(&store, &sent.id, HEART, true, &at(2026, 11, 18, 9, 5));
    let bytes = group(dir.path(), &sent.id).unwrap();
    let h = hlc_of(dir.path(), &sent.id, "reactions");
    // The reference vector: the exact bytes of the first reaction (ADR-0001 appendix §C.9).
    assert_eq!(
        bytes,
        format!(
            r#"{{"items":[{{"at":{},"e":"{HEART}","h":"{h}","tz":"Europe/Moscow"}}]}}"#,
            at(2026, 11, 18, 9, 5).ms
        )
    );
    store
        .set_message_flag(&sent.id, MessageFlag::Task, true, &later(1))
        .unwrap();
    store.edit_message(&sent.id, "new text", &later(2)).unwrap();
    assert_eq!(group(dir.path(), &sent.id).unwrap(), bytes);
    assert_eq!(hlc_of(dir.path(), &sent.id, "reactions"), h);
}

#[test]
fn input_rules_refuse_without_writing() {
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
        )
    };
    let before = snapshot(dir.path());
    let now = later(5);
    let code = |result: Result<kinige_domain::MessageView, StoreError>| result.unwrap_err().code();

    // The id.
    for id in [
        "",
        "x",
        "00000000-0000-4000-8000-00000000000",
        &sent.id.to_uppercase(),
    ] {
        assert_eq!(
            code(store.set_message_reaction(id, "🔥", true, &now)),
            ErrorCode::MessageNotFound,
            "{id:?}"
        );
    }
    let unknown = "11111111-2222-4333-8444-555555555555";
    assert_eq!(
        code(store.set_message_reaction(unknown, "🔥", true, &now)),
        ErrorCode::MessageNotFound
    );
    // The emoji: empty, text, two emoji, lone VS16 or ZWJ, controls, a megabyte, markup.
    let big = "x".repeat(1 << 20);
    let long = "😂".repeat(50);
    for text in [
        "",
        " ",
        "a",
        "ab",
        "😂😂",
        "😂 ",
        "\u{FE0F}",
        "\u{200D}",
        "😂\u{200D}",
        "\u{7}",
        "😂\u{0}",
        "<b>",
        &long,
        &big,
    ] {
        for on in [true, false] {
            assert_eq!(
                code(store.set_message_reaction(&sent.id, text, on, &now)),
                ErrorCode::ReactionInvalid,
                "{:.12?}",
                text
            );
        }
    }
    assert_eq!(snapshot(dir.path()), before, "nothing was written");

    // The trash: refused for a put and for a removal; restoring brings the reaction back.
    store.delete_message(&sent.id, &now).unwrap();
    for on in [true, false] {
        assert_eq!(
            code(store.set_message_reaction(&sent.id, HEART, on, &now)),
            ErrorCode::MessageInTrash
        );
    }
    store.restore_message(&sent.id, &now).unwrap();
    let restored = store.list_messages(INBOX_ID, None, 10, &now).unwrap();
    assert_eq!(restored.messages.last().unwrap().reactions.len(), 1);
}

#[test]
fn a_read_only_entry_a_corrupt_group_and_a_record_with_mismatched_maps_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let now = later(5);

    // Read-only: a group of a newer `sv`.
    let newer = send(&store, "from the future");
    common::edit_raw_groups(dir.path(), &newer.id, |_, hlcs| {
        let h = serde_json::from_str::<Value>(hlcs["body"].get()).unwrap()["h"].clone();
        hlcs.insert(
            "flags".into(),
            serde_json::value::RawValue::from_string(json!({"h": h, "sv": 2}).to_string()).unwrap(),
        );
    });
    raw(dir.path())
        .execute(
            "UPDATE message SET schema_version = 2 WHERE id = ?1",
            [&newer.id],
        )
        .unwrap();
    let error = store
        .set_message_reaction(&newer.id, HEART, true, &now)
        .unwrap_err();
    assert_eq!(error.code(), ErrorCode::ReadOnly);

    // A group that is not well-formed: a corrupt group, as for any other group; nothing is shown from it.
    let corrupt = send(&store, "corrupt");
    let h = hlc_of(dir.path(), &corrupt.id, "body");
    put_group(dir.path(), &corrupt.id, r#"{"items":"no"}"#, &h, 1);
    let bytes = group(dir.path(), &corrupt.id);
    for on in [true, false] {
        let error = store
            .set_message_reaction(&corrupt.id, HEART, on, &now)
            .unwrap_err();
        assert!(matches!(error, StoreError::Internal(_)), "{error:?}");
    }
    assert_eq!(group(dir.path(), &corrupt.id), bytes);

    // 7b's whole-record refusal covers the add path: the two maps name different groups.
    let split = send(&store, "split");
    common::edit_raw_groups(dir.path(), &split.id, |_, hlcs| {
        hlcs.remove("flags");
    });
    let before = (group(dir.path(), &split.id), clock(dir.path()));
    let error = store
        .set_message_reaction(&split.id, HEART, true, &now)
        .unwrap_err();
    assert!(matches!(error, StoreError::Internal(_)), "{error:?}");
    assert_eq!((group(dir.path(), &split.id), clock(dir.path())), before);
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
    assert_eq!((shown.reactions.len(), shown.reaction_keys_had), (0, 32));
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
    assert_eq!(view.reaction_keys_had, 12, "nothing was trimmed");
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
    wait_closed_dir(dir.path());
    let store = open_without_mirror(dir.path());
    let page = store.list_messages(INBOX_ID, None, 10, &later(3)).unwrap();
    let got = &page.messages.iter().find(|m| m.id == id).unwrap().reactions;
    let pairs: Vec<_> = got.iter().map(|r| (r.emoji.as_str(), r.put_at)).collect();
    assert_eq!(pairs, [(HEART, later(1).ms), ("🔥", later(2).ms)]);
}

fn wait_closed_dir(dir: &std::path::Path) {
    common::ops::wait_closed(dir);
}
