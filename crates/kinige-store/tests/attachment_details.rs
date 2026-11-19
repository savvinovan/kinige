//! Slice 7b: the details of an attachment. The three groups written at creation, the draft's values on send, the per-field commands
//! (only the sent field's group is written, under a new HLC, everything else byte-identical), an older row gets only the group that
//! changed. The failure side is in `attachment_details_errors.rs`; search, the Markdown copy, MCP and the migration are in
//! `attachment_details_surfaces.rs`.

#![cfg(test)]

mod common;

use common::days::at;
use common::details::*;
use common::ops::{clock, groups as message_groups};
use common::*;
use kinige_domain::groups::INBOX_ID;
use kinige_store::Now;
use serde_json::json;

fn now() -> Now {
    at(2026, 11, 18, 12, 0)
}

fn later() -> Now {
    at(2026, 11, 18, 12, 5)
}

#[test]
fn a_new_attachment_has_the_three_groups_with_the_defaults_under_its_creation_hlc() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let sent = photo_and_pdf(&store, dir.path(), &now());
    for view in &sent.attachments {
        let id = &view.id;
        let found = groups(dir.path(), id);
        assert_eq!(found["name"], r#"{"name":null}"#);
        assert_eq!(found["description"], r#"{"description":null}"#);
        assert_eq!(found["privacy"], r#"{"spoiler":false}"#);
        let stamps = hlcs(dir.path(), id);
        assert_eq!(stamps.len(), 6);
        let first = stamps["core"]["h"].clone();
        assert!(
            stamps
                .values()
                .all(|entry| entry["h"] == first && entry["sv"] == 1)
        );
        assert_eq!(columns(dir.path(), id), (None, None, 0));
        assert_eq!(view.name, view.filename);
        assert_eq!((view.description.clone(), view.spoiler), (None, false));
    }
}

#[test]
fn the_drafts_details_survive_a_reopen_and_become_the_groups_when_it_is_sent() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let source = dir.path().join("src");
    std::fs::create_dir_all(&source).unwrap();
    let path = common::attach::write(&source, "IMG_0001.jpg", &common::attach::jpeg(40, 30));
    let tile = store.attach_path(INBOX_ID, &path, &now()).unwrap();
    let edited = store
        .set_draft_attachment_details(
            &tile.id,
            &changes(
                json!({"name": " beach ", "description": "Beach,\r\nJuly\t", "spoiler": true}),
            ),
        )
        .unwrap();
    assert_eq!(edited.name, "beach.jpg");
    assert_eq!(edited.filename, "IMG_0001.jpg", "the original is immutable");
    assert_eq!(edited.description.as_deref(), Some("Beach,\nJuly"));
    assert!(edited.spoiler);

    drop(store);
    let store = open_without_mirror(dir.path());
    let draft = store.chat_draft(INBOX_ID, &now()).unwrap();
    assert_eq!(draft.attachments, vec![edited.clone()]);

    let sent = store
        .send_entry(INBOX_ID, "", None, std::slice::from_ref(&tile.id), &now())
        .unwrap();
    assert_eq!(sent.attachments[0], edited);
    let found = groups(dir.path(), &tile.id);
    assert_eq!(found["name"], r#"{"name":"beach.jpg"}"#);
    assert_eq!(found["description"], r#"{"description":"Beach,\nJuly"}"#);
    assert_eq!(found["privacy"], r#"{"spoiler":true}"#);
    assert_eq!(
        columns(dir.path(), &tile.id),
        (Some("beach.jpg".into()), Some("Beach,\nJuly".into()), 1)
    );
    assert_eq!(
        count(dir.path(), "SELECT COUNT(*) FROM draft_attachment"),
        0
    );
}

#[test]
fn a_draft_name_or_description_that_fails_the_rules_becomes_null_and_the_spoiler_is_kept() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let source = dir.path().join("src");
    std::fs::create_dir_all(&source).unwrap();
    let path = common::attach::write(&source, "IMG_0001.jpg", &common::attach::jpeg(40, 30));
    let tile = store.attach_path(INBOX_ID, &path, &now()).unwrap();
    // A damaged draft row (the store never writes these values itself).
    raw(dir.path())
        .execute(
            "UPDATE draft_attachment SET display_name = 'a/b.jpg', description = ' padded ', spoiler = 1",
            [],
        )
        .unwrap();
    store
        .send_entry(INBOX_ID, "", None, std::slice::from_ref(&tile.id), &now())
        .unwrap();
    let found = groups(dir.path(), &tile.id);
    assert_eq!(found["name"], r#"{"name":null}"#);
    assert_eq!(found["description"], r#"{"description":null}"#);
    assert_eq!(found["privacy"], r#"{"spoiler":true}"#);
}

#[test]
fn a_details_edit_rewrites_only_the_sent_fields_group_and_leaves_the_entry_alone() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let sent = photo_and_pdf(&store, dir.path(), &now());
    let photo = sent.attachments[0].id.clone();
    let before = groups(dir.path(), &photo);
    let stamps_before = hlcs(dir.path(), &photo);
    let entry_before = message_groups(dir.path(), "message", &sent.id);

    let view = store
        .set_attachment_details(&photo, &changes(json!({"name": "beach"})), &later())
        .unwrap();
    assert_eq!(view.name, "beach.jpg");
    let after = groups(dir.path(), &photo);
    assert_eq!(after["name"], r#"{"name":"beach.jpg"}"#);
    for group in ["core", "order", "deleted", "description", "privacy"] {
        assert_eq!(after[group], before[group], "{group} is byte-identical");
        assert_eq!(
            hlcs(dir.path(), &photo)[group],
            stamps_before[group],
            "{group} keeps its HLC"
        );
    }
    let (old, new) = (
        stamps_before["name"]["h"].as_str().unwrap(),
        hlc_of(dir.path(), &photo, "name"),
    );
    assert!(new.as_str() > old, "{new} is above {old}");
    assert_eq!(columns(dir.path(), &photo).0.as_deref(), Some("beach.jpg"));

    // The entry itself is untouched: no revision, no `edited_at`, the same groups.
    assert_eq!(
        message_groups(dir.path(), "message", &sent.id),
        entry_before
    );
    assert_eq!(common::ops::revisions(dir.path(), &sent.id), 0);
    let shown = view_in_feed(&store, &sent.id, &photo, &later());
    assert_eq!(shown.name, "beach.jpg");
    let edited_at: Option<i64> = raw(dir.path())
        .query_row(
            "SELECT edited_at FROM message WHERE id = ?1",
            [&sent.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(edited_at, None);

    // The second edit of the same detail goes above the first; the third field is its own group.
    store
        .set_attachment_details(
            &photo,
            &changes(json!({"name": "sea"})),
            &at(2026, 11, 18, 12, 9),
        )
        .unwrap();
    assert!(hlc_of(dir.path(), &photo, "name") > new);
    store
        .set_attachment_details(
            &photo,
            &changes(json!({"spoiler": true})),
            &at(2026, 11, 18, 12, 10),
        )
        .unwrap();
    let last = groups(dir.path(), &photo);
    assert_eq!(last["privacy"], r#"{"spoiler":true}"#);
    assert_eq!(last["name"], r#"{"name":"sea.jpg"}"#);
    assert_eq!(last["description"], before["description"]);
}

#[test]
fn equal_values_write_nothing_and_leave_the_clock_alone() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let sent = photo_and_pdf(&store, dir.path(), &now());
    let photo = sent.attachments[0].id.clone();
    store
        .set_attachment_details(
            &photo,
            &changes(json!({"name": "beach", "spoiler": true})),
            &later(),
        )
        .unwrap();
    let (rows, clock_before) = (groups(dir.path(), &photo), clock(dir.path()));
    for same in [
        json!({}),
        json!({"name": "beach"}),
        json!({"name": "  beach  "}),
        json!({"description": null}),
        json!({"description": "   "}),
        json!({"spoiler": true}),
        json!({"name": "beach", "description": null, "spoiler": true}),
    ] {
        store
            .set_attachment_details(&photo, &changes(same.clone()), &at(2026, 11, 18, 13, 0))
            .unwrap();
        assert_eq!(groups(dir.path(), &photo), rows, "{same}");
        assert_eq!(clock(dir.path()), clock_before, "{same}");
    }
    // The original name, said again, is the original: nothing to write on a row that has none.
    let pdf = sent.attachments[1].id.clone();
    let untouched = (groups(dir.path(), &pdf), clock(dir.path()));
    store
        .set_attachment_details(&pdf, &changes(json!({"name": "menu"})), &later())
        .unwrap();
    assert_eq!((groups(dir.path(), &pdf), clock(dir.path())), untouched);
}

#[test]
fn an_older_row_gets_only_the_group_that_changed_and_never_a_default_one() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let sent = photo_and_pdf(&store, dir.path(), &now());
    let photo = sent.attachments[0].id.clone();
    make_legacy(dir.path(), &photo);
    let legacy = groups(dir.path(), &photo);
    assert_eq!(legacy.len(), 3);
    let view = view_in_feed(&store, &sent.id, &photo, &now());
    assert_eq!(
        (view.name.as_str(), view.description.clone(), view.spoiler),
        ("IMG_0001.jpg", None, false),
        "an absent group means its default"
    );

    // "Hide as spoiler" from the menu sends only the spoiler.
    let hidden = store
        .set_attachment_details(&photo, &changes(json!({"spoiler": true})), &later())
        .unwrap();
    assert!(hidden.spoiler);
    let one = groups(dir.path(), &photo);
    assert_eq!(
        one.keys().collect::<Vec<_>>(),
        ["core", "deleted", "order", "privacy"]
    );
    assert_eq!(one["privacy"], r#"{"spoiler":true}"#);
    for group in ["core", "order", "deleted"] {
        assert_eq!(one[group], legacy[group]);
    }
    let privacy_hlc = hlc_of(dir.path(), &photo, "privacy");
    assert!(privacy_hlc > hlc_of(dir.path(), &photo, "core"));

    // A rename adds `name` alone: the privacy group stays as it is, so the rename cannot uncover the photo.
    store
        .set_attachment_details(
            &photo,
            &changes(json!({"name": "beach"})),
            &at(2026, 11, 18, 12, 9),
        )
        .unwrap();
    let two = groups(dir.path(), &photo);
    assert_eq!(
        two.keys().collect::<Vec<_>>(),
        ["core", "deleted", "name", "order", "privacy"]
    );
    assert_eq!(two["privacy"], one["privacy"]);
    assert_eq!(hlc_of(dir.path(), &photo, "privacy"), privacy_hlc);
    let shown = view_in_feed(&store, &sent.id, &photo, &later());
    assert_eq!((shown.name.as_str(), shown.spoiler), ("beach.jpg", true));
    assert_eq!(
        columns(dir.path(), &photo),
        (Some("beach.jpg".into()), None, 1)
    );

    // An edit that changes nothing on an older row adds nothing: no default-valued group, ever.
    let other = sent.attachments[1].id.clone();
    make_legacy(dir.path(), &other);
    let (rows, stamps) = (groups(dir.path(), &other), hlcs(dir.path(), &other));
    store
        .set_attachment_details(
            &other,
            &changes(json!({"description": null, "name": null})),
            &later(),
        )
        .unwrap();
    assert_eq!(
        (groups(dir.path(), &other), hlcs(dir.path(), &other)),
        (rows, stamps)
    );
}

#[test]
fn a_mistyped_group_of_another_client_is_read_safely_and_a_local_edit_replaces_it() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let sent = photo_and_pdf(&store, dir.path(), &now());
    let photo = sent.attachments[0].id.clone();
    set_group(dir.path(), &photo, "privacy", "7", 1);
    set_group(dir.path(), &photo, "name", r#""beach.jpg""#, 1);
    set_group(dir.path(), &photo, "description", r#"["x"]"#, 1);
    let shown = view_in_feed(&store, &sent.id, &photo, &now());
    assert_eq!(shown.name, "IMG_0001.jpg", "a bad name shows the original");
    assert_eq!(shown.description, None, "a bad description shows none");
    assert!(shown.spoiler, "an unclear spoiler stays covered");

    let fixed = store
        .set_attachment_details(
            &photo,
            &changes(json!({"name": "beach", "description": "Beach", "spoiler": false})),
            &later(),
        )
        .unwrap();
    assert_eq!(
        (
            fixed.name.as_str(),
            fixed.description.as_deref(),
            fixed.spoiler
        ),
        ("beach.jpg", Some("Beach"), false)
    );
    let after = groups(dir.path(), &photo);
    assert_eq!(after["privacy"], r#"{"spoiler":false}"#);
    assert_eq!(after["name"], r#"{"name":"beach.jpg"}"#);
}
