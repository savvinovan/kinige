//! Slice 5c, the derived columns and the surfaces: the columns come from the one domain reader and are rebuilt when the rules version differs,
//! a row from before 5c has no group and is never given one, a hostile emoji reaches no column, view, copy or agent, and the sticker shows up
//! as "Sticker" in the quote, as `![sticker.webp]` in the Markdown copy, as the stem `sticker` in search and as `"sticker": true` to an agent.

#![cfg(test)]

mod common;

use common::agent::{agent, call};
use common::details::{changes, groups, hlcs, photo_and_pdf};
use common::old_db::downgrade_to;
use common::stickers::*;
use common::*;
use kinige_domain::AgentMode;
use kinige_domain::forest::FirstWeekday;
use kinige_domain::groups::INBOX_ID;
use kinige_domain::{SearchFilters, attachment::sticker_of};
use serde_json::json;
use std::path::Path;

fn now() -> kinige_store::Now {
    moscow(T0)
}

/// One sticker sent into Inbox of a store in `dir`; its attachment id and entry id.
fn send_one(store: &kinige_store::Store, link: &str) -> (String, String) {
    add_pack(store, Some(link), link, &drawn(1)).unwrap();
    let id = ids_of(&store.sticker_library(false).unwrap(), link)[0].clone();
    let view = store.send_sticker(INBOX_ID, &id, None, &now()).unwrap();
    (view.attachments[0].id.clone(), view.id)
}

fn columns_of(dir: &Path, id: &str) -> (i64, Option<String>) {
    raw(dir)
        .query_row(
            "SELECT sticker, sticker_emoji FROM attachment WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap()
}

#[test]
fn the_columns_are_rebuilt_from_the_groups_when_the_rules_version_differs() {
    let dir = tempfile::tempdir().unwrap();
    let id = {
        let store = open_without_mirror(dir.path());
        send_one(&store, "One").0
    };
    common::ops::wait_closed(dir.path());
    raw(dir.path())
        .execute_batch(&format!(
            "UPDATE attachment SET sticker = 0, sticker_emoji = 'x' WHERE id = '{id}'; \
             UPDATE app_setting SET value = '0' WHERE key = 'sticker_rules_version';"
        ))
        .unwrap();
    let store = open_without_mirror(dir.path());
    assert_eq!(columns_of(dir.path(), &id), (1, Some("😺".to_string())));
    let stored: String = raw(dir.path())
        .query_row(
            "SELECT value FROM app_setting WHERE key = 'sticker_rules_version'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(stored, "1");
    drop(store);
    // A matching version is not rebuilt: a column changed by hand stays (the rebuild is the version's business).
    raw(dir.path())
        .execute("UPDATE attachment SET sticker = 0", [])
        .unwrap();
    drop(open_without_mirror(dir.path()));
    assert_eq!(columns_of(dir.path(), &id).0, 0);
}

#[test]
fn a_hostile_emoji_from_another_device_reaches_no_column_view_copy_or_agent() {
    let a = agent(AgentMode::Read, true);
    let dir = a.dir.path();
    let (id, entry) = send_one(&a.store, "One");
    raw(dir)
        .execute(
            "UPDATE attachment SET groups_raw = replace(groups_raw, '\"emoji\":\"😺\"', '\"emoji\":\"<b>x</b>\"'), \
             sticker_emoji = '<b>x</b>' WHERE id = ?1",
            [&id],
        )
        .unwrap();
    assert!(
        groups(dir, &id)["sticker"].contains("<b>x</b>"),
        "kept verbatim in the bytes"
    );
    // The next open rebuilds nothing (same version); force the rebuild the way an update of the rules would.
    raw(dir)
        .execute(
            "UPDATE app_setting SET value = '0' WHERE key = 'sticker_rules_version'",
            [],
        )
        .unwrap();
    let store = {
        drop(a.store);
        common::ops::wait_closed(dir);
        open_without_mirror(dir)
    };
    assert_eq!(columns_of(dir, &id), (1, None));
    let page = store.list_messages(INBOX_ID, None, 10, &now()).unwrap();
    assert_eq!(page.messages[0].attachments[0].sticker_emoji, None);
    assert!(page.messages[0].attachments[0].sticker);
    assert_eq!(entry, page.messages[0].id);
}

#[test]
fn a_row_from_before_5c_has_no_group_is_a_picture_and_is_never_given_one() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let photo = photo_and_pdf(&store, dir.path(), &now()).attachments[0]
        .id
        .clone();
    let strip = |dir: &Path| {
        raw(dir)
            .execute(
                "UPDATE attachment SET groups_raw = json_remove(groups_raw, '$.sticker'), hlc_json = json_remove(hlc_json, '$.sticker')",
                [],
            )
            .unwrap();
    };
    strip(dir.path());
    assert!(!groups(dir.path(), &photo).contains_key("sticker"));
    let before = (groups(dir.path(), &photo), hlcs(dir.path(), &photo));
    // After every attachment command the bytes and the HLC of the sticker group are unchanged: here, absent still.
    store
        .set_attachment_details(
            &photo,
            &changes(json!({"name": "beach", "description": "x", "spoiler": true})),
            &now(),
        )
        .unwrap();
    store
        .set_attachment_details(&photo, &changes(json!({"spoiler": false})), &now())
        .unwrap();
    let after = (groups(dir.path(), &photo), hlcs(dir.path(), &photo));
    assert!(!after.0.contains_key("sticker") && !after.1.contains_key("sticker"));
    assert_eq!(before.0["core"], after.0["core"]);
    assert_eq!(columns_of(dir.path(), &photo), (0, None));
}

#[test]
fn migration_11_leaves_every_row_of_version_10_byte_identical() {
    let dir = tempfile::tempdir().unwrap();
    let ids: Vec<String> = {
        let store = open_without_mirror(dir.path());
        let sent = photo_and_pdf(&store, dir.path(), &now());
        send_one(&store, "One");
        sent.attachments.iter().map(|a| a.id.clone()).collect()
    };
    common::ops::wait_closed(dir.path());
    // Rows from before 5c have no group.
    raw(dir.path())
        .execute(
            "UPDATE attachment SET groups_raw = json_remove(groups_raw, '$.sticker'), hlc_json = json_remove(hlc_json, '$.sticker') WHERE id IN (?1, ?2)",
            [&ids[0], &ids[1]],
        )
        .unwrap();
    let snapshot = |dir: &Path| -> Vec<(String, String, String)> {
        let conn = raw(dir);
        let mut statement = conn
            .prepare(
                "SELECT id, groups_raw, hlc_json FROM attachment WHERE id IN (?1, ?2) ORDER BY id",
            )
            .unwrap();
        statement
            .query_map([&ids[0], &ids[1]], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };
    let before = snapshot(dir.path());
    downgrade_to(dir.path(), 10);
    let version: i64 = raw(dir.path())
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, 10);
    assert_eq!(
        snapshot(dir.path()),
        before,
        "the copy to the old schema kept the bytes"
    );
    let store = open_without_mirror(dir.path());
    assert_eq!(snapshot(dir.path()), before, "migration 11 changed no row");
    for id in &ids {
        assert_eq!(columns_of(dir.path(), id), (0, None));
    }
    assert!(
        store.sticker_library(false).unwrap().packs.is_empty(),
        "the library tables start empty"
    );
}

#[test]
fn the_sticker_shows_up_in_the_quote_the_copy_the_search_the_forest_and_to_an_agent() {
    let a = agent(AgentMode::Read, true);
    let dir = a.dir.path();
    add_pack(&a.store, Some("One"), "One", &drawn(1)).unwrap();
    let id = ids_of(&a.store.sticker_library(false).unwrap(), "One")[0].clone();
    let sent = a.store.send_sticker(INBOX_ID, &id, None, &now()).unwrap();
    // The quote: "Sticker" with the emoji, in the summary of the reply target.
    let reply = a
        .store
        .send_reply(INBOX_ID, "ответ", Some(&sent.id), &now())
        .unwrap();
    let summary = &reply.reply_to.as_ref().unwrap().attachments;
    assert!(summary.sticker);
    assert_eq!(summary.sticker_emoji.as_deref(), Some("😺"));
    assert_eq!(summary.photos, 1);
    // Search finds it by the file stem, and not by the Russian word.
    let found = |query: &str| {
        a.store
            .search(query, &SearchFilters::default(), None, 10)
            .unwrap()
            .hits
            .len()
    };
    assert_eq!(found("sticker"), 1);
    assert_eq!(found("стикер"), 0);
    // The forest does not count it as a photo.
    let year = a
        .store
        .get_forest_year(2026, FirstWeekday::Monday, &now())
        .unwrap();
    let day = year.days.iter().find(|d| d.date == "2026-10-21").unwrap();
    assert_eq!(day.photos, 0, "{day:?}");
    // An agent sees an image attachment with the flag and the emoji.
    let entry = call(&a.store, "get_entry", json!({"id": sent.id}), T0).unwrap();
    assert_eq!(
        entry["entry"]["attachments"][0],
        json!({"kind": "image", "name": "sticker.webp", "sticker": true, "emoji": "😺",
               "bytes": sent.attachments[0].byte_size, "width": 200, "height": 160})
    );
    // A covered sticker is not summarized as one.
    raw(dir)
        .execute(
            "UPDATE attachment SET spoiler = 1 WHERE id = ?1",
            [&sent.attachments[0].id],
        )
        .unwrap();
    let page = a.store.list_messages(INBOX_ID, None, 10, &now()).unwrap();
    assert!(sticker_of(&page.messages[0].body_md, &page.messages[0].attachments).is_none());
    assert!(
        !page.messages[1]
            .reply_to
            .as_ref()
            .unwrap()
            .attachments
            .sticker
    );
}

#[test]
fn the_markdown_copy_writes_the_sticker_as_an_image_by_its_file_name() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path());
    send_one(&store, "One");
    let day = mirror_dir(dir.path()).join("Inbox/2026");
    let file = day.join("2026-10-21.md");
    wait_until("the day file has the sticker", || {
        std::fs::read_to_string(&file)
            .is_ok_and(|text| text.contains("![sticker.webp](<2026-10-21/sticker.webp>)"))
    });
    assert!(day.join("2026-10-21/sticker.webp").is_file());
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(!text.contains("😺"), "the emoji is not part of the copy");
}
