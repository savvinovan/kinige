//! Slice 7b: the details of an attachment. The three groups written at creation, the draft's values on send, the per-field commands
//! (only the sent field's group is written, under a new HLC, everything else byte-identical), an older row gets only the group that
//! changed, the errors write nothing, a fault before the commit leaves everything as it was. Search, the Markdown copy, MCP and the
//! migration are in `attachment_details_surfaces.rs`.

#![cfg(test)]

mod common;

use common::days::at;
use common::details::*;
use common::ops::{clock, groups as message_groups};
use common::*;
use kinige_domain::attachment::AttachmentKind;
use kinige_domain::groups::INBOX_ID;
use kinige_store::{AttachFault, Now, Store, StoreError};
use serde_json::json;
use std::sync::Arc;

fn now() -> Now {
    at(2026, 11, 18, 12, 0)
}

fn later() -> Now {
    at(2026, 11, 18, 12, 5)
}

fn code(error: StoreError) -> kinige_domain::ErrorCode {
    error.code()
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

#[test]
fn a_rule_broken_or_a_bad_target_is_an_error_and_nothing_is_written() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let sent = photo_and_pdf(&store, dir.path(), &now());
    let (photo, pdf) = (
        sent.attachments[0].id.clone(),
        sent.attachments[1].id.clone(),
    );
    assert_eq!(sent.attachments[1].kind, AttachmentKind::File);
    let snapshot = |store: &Store| {
        (
            groups(dir.path(), &photo),
            groups(dir.path(), &pdf),
            clock(dir.path()),
            count(dir.path(), "SELECT COUNT(*) FROM mirror_dirty"),
            store
                .search("beach", &Default::default(), None, 5)
                .unwrap()
                .hits
                .len(),
        )
    };
    let before = snapshot(&store);
    let invalid = kinige_domain::ErrorCode::AttachmentDetailsInvalid;
    let too_long = "я".repeat(1025);
    let huge = "x".repeat(1 << 20);
    let long_stem = "é".repeat(127);
    for bad in [
        json!({"name": "a\u{0}b"}),
        json!({"name": "a\u{1b}b"}),
        json!({"name": "a\u{7f}b"}),
        json!({"name": "a\u{85}b"}),
        json!({"name": "evil\u{202e}gpj"}),
        json!({"name": "a/b"}),
        json!({"name": "a\\b"}),
        json!({"name": long_stem}),
        json!({"description": too_long}),
        json!({"description": huge}),
        json!({"description": "a\u{0}b"}),
        json!({"description": "a\u{1b}[31m"}),
        json!({"name": "ok", "description": "a\u{7f}"}),
    ] {
        let error = store
            .set_attachment_details(&photo, &changes(bad.clone()), &later())
            .unwrap_err();
        assert_eq!(
            code(error),
            invalid,
            "{}",
            bad.to_string().chars().take(40).collect::<String>()
        );
        let draft_error = store.set_draft_attachment_details(&photo, &changes(bad.clone()));
        assert_eq!(
            draft_error.unwrap_err().code(),
            kinige_domain::ErrorCode::AttachmentNotFound,
            "a sent attachment is not a draft tile"
        );
    }
    // A spoiler belongs to an image: any value on a file card is refused.
    for flag in [true, false] {
        let error = store
            .set_attachment_details(&pdf, &changes(json!({"spoiler": flag})), &later())
            .unwrap_err();
        assert_eq!(code(error), invalid);
    }
    for id in [
        "",
        "x",
        "../../etc",
        "00000000-0000-4000-8000-000000000000",
        &photo.to_uppercase(),
    ] {
        let error = store
            .set_attachment_details(id, &changes(json!({"name": "x"})), &later())
            .unwrap_err();
        assert_eq!(
            code(error),
            kinige_domain::ErrorCode::AttachmentNotFound,
            "{id:?}"
        );
    }
    assert_eq!(snapshot(&store), before);

    // The entry is in the trash: the same answer as for a move; restored, the edit works.
    store.delete_message(&sent.id, &now()).unwrap();
    let error = store
        .set_attachment_details(&photo, &changes(json!({"name": "x"})), &later())
        .unwrap_err();
    assert_eq!(code(error), kinige_domain::ErrorCode::MessageInTrash);
    store.restore_message(&sent.id, &now()).unwrap();
    store
        .set_attachment_details(&photo, &changes(json!({"name": "x"})), &later())
        .unwrap();

    // A group in a newer `sv` makes the attachment read-only.
    set_group(
        dir.path(),
        &photo,
        "name",
        r#"{"name":"x.jpg","future":1}"#,
        2,
    );
    let error = store
        .set_attachment_details(&photo, &changes(json!({"spoiler": true})), &later())
        .unwrap_err();
    assert_eq!(code(error), kinige_domain::ErrorCode::ReadOnly);

    // A purged entry's attachment is gone.
    store.delete_message(&sent.id, &now()).unwrap();
    store.purge_message(&sent.id, &now()).unwrap();
    let error = store
        .set_attachment_details(&pdf, &changes(json!({"name": "x"})), &later())
        .unwrap_err();
    assert_eq!(code(error), kinige_domain::ErrorCode::AttachmentNotFound);
}

#[test]
fn a_draft_tile_refuses_garbage_and_belongs_to_the_draft_only() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let source = dir.path().join("src");
    std::fs::create_dir_all(&source).unwrap();
    let file = common::attach::write(&source, "menu.pdf", b"%PDF-1.4");
    let tile = store.attach_path(INBOX_ID, &file, &now()).unwrap();
    let before = raw(dir.path())
        .query_row(
            "SELECT display_name, description, spoiler FROM draft_attachment",
            [],
            |r| {
                Ok((
                    r.get::<_, Option<String>>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            },
        )
        .unwrap();
    for bad in [
        json!({"spoiler": true}),
        json!({"spoiler": false}),
        json!({"name": "a/b"}),
        json!({"description": "x".repeat(1025)}),
    ] {
        let error = store
            .set_draft_attachment_details(&tile.id, &changes(bad))
            .unwrap_err();
        assert_eq!(
            code(error),
            kinige_domain::ErrorCode::AttachmentDetailsInvalid
        );
    }
    for id in ["", "not-a-uuid", "00000000-0000-4000-8000-000000000000"] {
        let error = store
            .set_draft_attachment_details(id, &changes(json!({"name": "x"})))
            .unwrap_err();
        assert_eq!(code(error), kinige_domain::ErrorCode::AttachmentNotFound);
    }
    let after = raw(dir.path())
        .query_row(
            "SELECT display_name, description, spoiler FROM draft_attachment",
            [],
            |r| {
                Ok((
                    r.get::<_, Option<String>>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(after, before);
    // A file card has a name and a description.
    let view = store
        .set_draft_attachment_details(
            &tile.id,
            &changes(json!({"name": "лето", "description": "меню"})),
        )
        .unwrap();
    assert_eq!(
        (view.name.as_str(), view.description.as_deref()),
        ("лето.pdf", Some("меню"))
    );
}

#[test]
fn a_fault_before_the_commit_leaves_groups_columns_index_and_copy_marks_as_they_were() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let sent = photo_and_pdf(&store, dir.path(), &now());
    let photo = sent.attachments[0].id.clone();
    raw(dir.path())
        .execute("DELETE FROM mirror_dirty", [])
        .unwrap();
    let row = |dir: &std::path::Path| {
        raw(dir)
            .query_row(
                "SELECT groups_raw, hlc_json, schema_version, display_name, description, spoiler FROM attachment WHERE id = ?1",
                [&photo],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, i64>(2)?,
                        r.get::<_, Option<String>>(3)?,
                        r.get::<_, Option<String>>(4)?,
                        r.get::<_, i64>(5)?,
                    ))
                },
            )
            .unwrap()
    };
    let before = (
        row(dir.path()),
        clock(dir.path()),
        count(dir.path(), "SELECT COUNT(*) FROM mirror_dirty"),
    );
    store.inject_attach_fault_for_test(AttachFault::DetailsBeforeCommit(Arc::new(|| {
        panic!("injected: the process dies before the commit")
    })));
    let error = store
        .set_attachment_details(
            &photo,
            &changes(json!({"name": "beach", "description": "Beach, July", "spoiler": true})),
            &later(),
        )
        .unwrap_err();
    assert!(matches!(error, StoreError::Internal(_)), "{error:?}");
    store.inject_attach_fault_for_test(AttachFault::Clear);
    let after = (
        row(dir.path()),
        clock(dir.path()),
        count(dir.path(), "SELECT COUNT(*) FROM mirror_dirty"),
    );
    assert_eq!(after, before, "the transaction rolled back");
    for query in ["beach", "July"] {
        let hits = store
            .search(query, &Default::default(), None, 5)
            .unwrap()
            .hits;
        assert!(hits.is_empty(), "{query}: the index did not change");
    }
    // The store still works, and the same edit now goes through.
    store
        .set_attachment_details(&photo, &changes(json!({"name": "beach"})), &later())
        .unwrap();
    assert_eq!(columns(dir.path(), &photo).0.as_deref(), Some("beach.jpg"));
}

fn count(dir: &std::path::Path, sql: &str) -> i64 {
    raw(dir).query_row(sql, [], |r| r.get(0)).unwrap()
}
