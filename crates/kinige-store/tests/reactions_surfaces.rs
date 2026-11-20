//! Slice 5a outside the write: the view in the feed, the "On this day" card, search and the trash; purge; the derived rows (rebuilt
//! equal incremental, a rules version mismatch, migration 10); the Markdown copy line; MCP; hostile items in the group.

#![cfg(test)]

mod common;

use common::agent::{agent, call};
use common::days::{at, ms};
use common::ops::wait_closed;
use common::reactions::*;
use common::*;
use kinige_domain::AgentMode;
use kinige_domain::groups::INBOX_ID;
use kinige_store::Now;
use serde_json::{Value, json};

const DEVICE: &str = "5f0c3e2a-9b1d-4c7e-8a64-0d2f1b3c4e5f";

fn stamp(l: u64) -> String {
    format!("{l:013}-00000-{DEVICE}")
}

#[test]
fn years_later_counts_the_day_in_the_reactions_own_zone() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    // The entry is from 2025-11-19; both reactions are put at 22:30 UTC on 2026-11-18, which is 01:30 on the 19th in Moscow (still
    // the 18th for the journal: before 04:00) and 11:30 on the 19th in Auckland.
    let entry = store
        .send_entry(INBOX_ID, "last year", None, &[], &at(2025, 11, 19, 10, 0))
        .unwrap();
    let moment = ms(2026, 11, 19, 1, 30); // `ms` takes Moscow time: 22:30 UTC on the 18th
    let moscow_put = Now::at(moment, "Europe/Moscow");
    let auckland_put = Now::at(moment + 1_000, "Pacific/Auckland");
    set(&store, &entry.id, "🥲", true, &moscow_put);
    let view = set(&store, &entry.id, "🔥", true, &auckland_put);
    let by_emoji = |e: &str| {
        view.reactions
            .iter()
            .find(|r| r.emoji == e)
            .unwrap()
            .clone()
    };
    assert_eq!(
        by_emoji("🥲").years_later,
        0,
        "the 18th in Moscow: 364 days"
    );
    assert_eq!(
        by_emoji("🔥").years_later,
        1,
        "the 19th in Auckland: a full year"
    );
    assert_eq!(by_emoji("🔥").put_tz, "Pacific/Auckland");
    // A reaction put before the entry's own day (a wrong clock) is clamped at 0.
    let early = Now::at(ms(2024, 1, 1, 12, 0), "UTC");
    assert_eq!(
        set(&store, &entry.id, "🎉", true, &early)
            .reactions
            .iter()
            .find(|r| r.emoji == "🎉")
            .unwrap()
            .years_later,
        0
    );
}

#[test]
fn the_feed_the_card_search_and_the_trash_show_the_chips_and_search_does_not_look_inside_them() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let old = store
        .send_entry(INBOX_ID, "exam story", None, &[], &at(2025, 11, 19, 10, 0))
        .unwrap();
    let today = store
        .send_entry(INBOX_ID, "exam today", None, &[], &at(2026, 11, 19, 8, 0))
        .unwrap();
    let now = at(2026, 11, 19, 9, 0);
    set(&store, &old.id, "🥲", true, &now);
    set(&store, &today.id, HEART, true, &now);

    let page = store.list_messages(INBOX_ID, None, 10, &now).unwrap();
    assert_eq!(
        page.messages
            .iter()
            .map(|m| m.reactions.len())
            .collect::<Vec<_>>(),
        [1, 1]
    );

    let card = store.get_on_this_day(&now).unwrap().expect("a card");
    let in_card = &card.years[0].entries[0].message;
    assert_eq!(
        (
            in_card.id.as_str(),
            in_card.reactions[0].emoji.as_str(),
            in_card.reactions[0].years_later
        ),
        (old.id.as_str(), "🥲", 1)
    );

    let found = store.search("exam", &Default::default(), None, 10).unwrap();
    assert_eq!(found.hits.len(), 2);
    assert!(
        found
            .hits
            .iter()
            .all(|hit| hit.message.reactions.len() == 1),
        "the rows carry their chips"
    );
    // The index is the text only: neither the emoji nor a word of it finds the entry.
    for query in ["🥲", "❤", "HEART", "reaction"] {
        let hits = store
            .search(query, &Default::default(), None, 10)
            .unwrap()
            .hits;
        assert!(hits.is_empty(), "{query}");
    }

    store.delete_message(&old.id, &now).unwrap();
    let trash = store.list_trash(&now).unwrap();
    assert_eq!(trash[0].message.reactions[0].emoji, "🥲");
    store.restore_message(&old.id, &now).unwrap();
    assert_eq!(
        rows(dir.path(), &old.id).len(),
        1,
        "the rows stayed through the trash"
    );
}

#[test]
fn purge_deletes_the_rows_with_the_entry_and_the_tombstone_names_the_group() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let sent = send(&store, "gone");
    set(&store, &sent.id, HEART, true, &later(1));
    set(&store, &sent.id, "🔥", true, &later(2));
    set(&store, &sent.id, "🔥", false, &later(3));
    assert_eq!(all_rows(dir.path()).1.len(), 1);
    store.delete_message(&sent.id, &later(4)).unwrap();
    store.purge_message(&sent.id, &later(5)).unwrap();
    assert_eq!(all_rows(dir.path()), (Vec::new(), Vec::new()));
    let purged_of: String = raw(dir.path())
        .query_row(
            "SELECT purged_of FROM tombstone WHERE entity = 'message' AND id = ?1",
            [&sent.id],
            |r| r.get(0),
        )
        .unwrap();
    assert!(purged_of.contains("\"reactions\""), "{purged_of}");
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
fn the_copy_has_one_line_with_each_reaction_and_the_day_it_was_put_on_in_its_zone() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path());
    let sent = send(&store, "Сдал экзамен!");
    let plain = store
        .send_entry(INBOX_ID, "no reactions here", None, &[], &later(1))
        .unwrap();
    set(
        &store,
        &sent.id,
        HEART,
        true,
        &Now::at(ms(2026, 11, 18, 9, 5), "Europe/Moscow"),
    );
    // 22:30 UTC on the 18th is 01:30 on the 19th in Moscow, still the journal day of the 18th (before 04:00): `2026-11-18`.
    let night = ms(2026, 11, 19, 1, 30);
    set(
        &store,
        &sent.id,
        "🔥",
        true,
        &Now::at(night, "Europe/Moscow"),
    );
    // The same moment in Auckland (UTC+13) is 11:30 on the 19th: a reaction put there is dated `2026-11-19`.
    set(
        &store,
        &sent.id,
        "😂",
        true,
        &Now::at(night + 2_000, "Pacific/Auckland"),
    );
    let day_file = mirror_dir(dir.path()).join("Inbox/2026/2026-11-18.md");
    wait_until("the copy with the reactions", || {
        std::fs::read_to_string(&day_file).is_ok_and(|text| text.contains("😂"))
    });
    let text = std::fs::read_to_string(&day_file).unwrap();
    let line = format!("{HEART} 2026-11-18 · 🔥 2026-11-18 · 😂 2026-11-19");
    assert!(
        text.contains(&format!("Сдал экзамен!\n\n{line}\n")),
        "{text}"
    );
    assert!(text.contains("no reactions here\n"), "{text}");
    assert_eq!(
        text.lines().filter(|l| l.contains(" · ")).count(),
        1,
        "one line only"
    );
    // Removing the last live reaction removes the line; an entry without reactions has none.
    for e in [HEART, "🔥", "😂"] {
        set(&store, &sent.id, e, false, &later(5));
    }
    wait_until("the copy without the line", || {
        std::fs::read_to_string(&day_file).is_ok_and(|text| !text.contains("😂"))
    });
    let text = std::fs::read_to_string(&day_file).unwrap();
    assert!(
        text.contains("Сдал экзамен!\n\n## ") || text.trim_end().ends_with("no reactions here"),
        "{text}"
    );
    assert!(
        !text.contains("2026-11-18 ·") && !text.contains(&format!("{HEART} 2026")),
        "{text}"
    );
    let _ = plain;
}

#[test]
fn an_agent_reads_the_reactions_with_their_times_and_cannot_see_those_of_a_hidden_chat() {
    let a = agent(AgentMode::Read, true);
    let sent = a
        .store
        .send_entry(INBOX_ID, "visible", None, &[], &morning())
        .unwrap();
    let secret = a
        .store
        .send_entry(&a.secret, "hidden chat", None, &[], &morning())
        .unwrap();
    let bare = a
        .store
        .send_entry(INBOX_ID, "bare", None, &[], &morning())
        .unwrap();
    set(&a.store, &sent.id, HEART, true, &later(1));
    set(&a.store, &sent.id, "🔥", true, &later(2));
    set(&a.store, &secret.id, "🔥", true, &later(2));
    let reply = call(&a.store, "get_entry", json!({"id": sent.id}), T0).unwrap();
    assert_eq!(
        reply["entry"]["reactions"],
        json!([
            {"emoji": HEART, "put": "2026-11-18T09:06:00+03:00"},
            {"emoji": "🔥", "put": "2026-11-18T09:07:00+03:00"},
        ])
    );
    let bare = call(&a.store, "get_entry", json!({"id": bare.id}), T0).unwrap();
    assert!(
        bare["entry"].get("reactions").is_none(),
        "absent when empty: {bare}"
    );
    // The hidden chat stays hidden: the entry, and so its reactions, cannot be read or found.
    assert!(call(&a.store, "get_entry", json!({"id": secret.id}), T0).is_err());
    let listed = call(&a.store, "get_entries", json!({"date": "2026-11-18"}), T0).unwrap();
    assert!(!listed.to_string().contains(&secret.id), "{listed}");
    assert!(
        listed.to_string().contains("🔥"),
        "the visible entry's reactions are in the list: {listed}"
    );
    // The agent can only read: no tool takes a reaction, and none is a tool of the list.
    assert!(
        call(
            &a.store,
            "put_reaction",
            json!({"id": sent.id, "emoji": "🔥"}),
            T0
        )
        .is_err(),
        "no such tool"
    );
    assert!(
        kinige_domain::agent::AgentTool::ALL
            .iter()
            .all(|tool| !tool.name().contains("reaction"))
    );
}

/// Items that must never reach the view, the copy or MCP: an `e` that is not an emoji, a zone that is not valid, a time out of range.
#[test]
fn hostile_items_in_a_group_from_elsewhere_show_nowhere() {
    let a = agent(AgentMode::Read, true);
    let dir = a.dir.path();
    let sent = a
        .store
        .send_entry(INBOX_ID, "hostile", None, &[], &morning())
        .unwrap();
    let day_file = mirror_dir(dir).join("Inbox/2026/2026-11-18.md");
    let item = |e: &str, at: Value, tz: Value| json!({"at": at, "e": e, "h": stamp(5), "tz": tz});
    let items = vec![
        item(
            "<script>alert(1)</script>",
            json!(1_792_000_000_000i64),
            json!("UTC"),
        ),
        item("😂\u{0}", json!(1_792_000_000_000i64), json!("UTC")),
        item("🔥", json!(1_792_000_000_000i64), json!("Europe/\nMoscow")),
        item("🎉", json!(1_792_000_000_000i64), json!("")),
        item("🙏", json!(1_792_000_000_000i64), json!(5)),
        item("😢", json!(1_792_000_000_000i64), Value::Null),
        item("👍", json!(1_792_000_000_000i64), json!("UTC")),
    ];
    let bytes = kinige_domain::jcs::canonical_string(&json!({ "items": items })).unwrap();
    put_group(dir, &sent.id, &bytes, &stamp(9), 1);
    raw(dir)
        .execute(
            "DELETE FROM app_setting WHERE key = 'reaction_rules_version'",
            [],
        )
        .unwrap();
    // Only the last item passes every check; the rest are in the group and nowhere else.
    let store = common::open(dir);
    let view = store.list_messages(INBOX_ID, None, 10, &later(1)).unwrap();
    let got: Vec<_> = view
        .messages
        .iter()
        .flat_map(|m| m.reactions.iter().map(|r| r.emoji.clone()))
        .collect();
    assert_eq!(got, ["👍"]);
    store
        .send_entry(INBOX_ID, "trigger the copy", None, &[], &later(2))
        .unwrap();
    wait_until("the copy", || {
        std::fs::read_to_string(&day_file).is_ok_and(|t| t.contains("trigger the copy"))
    });
    let text = std::fs::read_to_string(&day_file).unwrap();
    for bad in ["<script>", "\u{0}", "🔥", "🎉", "🙏", "😢"] {
        assert!(!text.contains(bad), "{bad}: {text}");
    }
    // MCP: the entry is read through the agent's own tool, and the same one item shows.
    let reply = call(&a.store, "get_entry", json!({"id": sent.id}), T0).unwrap();
    assert_eq!(
        reply["entry"]["reactions"].as_array().map(Vec::len),
        Some(1),
        "{reply}"
    );
    assert_eq!(reply["entry"]["reactions"][0]["emoji"], "👍");
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
