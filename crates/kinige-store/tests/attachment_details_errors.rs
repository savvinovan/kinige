//! Slice 7b, the failure side: a rule broken or a bad target is an error and nothing is written (groups, clock, copy marks, index), a
//! draft tile refuses garbage, and a fault before the commit leaves everything as it was. The behaviour on success is in
//! `attachment_details.rs`.

#![cfg(test)]

mod common;

use common::days::at;
use common::details::*;
use common::ops::clock;
use common::*;
use kinige_domain::ErrorCode;
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

fn code(error: StoreError) -> ErrorCode {
    error.code()
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
    let invalid = ErrorCode::AttachmentDetailsInvalid;
    let too_long = "я".repeat(1025);
    let huge = "x".repeat(1 << 20);
    let long_stem = "é".repeat(127);
    for bad in [
        json!({"name": "a\u{0}b"}),
        json!({"name": "a\u{1b}b"}),
        json!({"name": "a\u{7f}b"}),
        json!({"name": "a\u{85}b"}),
        json!({"name": format!("evil{}gpj", char::from_u32(0x202e).unwrap())}),
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
            ErrorCode::AttachmentNotFound,
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
        assert_eq!(code(error), ErrorCode::AttachmentNotFound, "{id:?}");
    }
    assert_eq!(snapshot(&store), before);

    // The entry is in the trash: the same answer as for a move; restored, the edit works.
    store.delete_message(&sent.id, &now()).unwrap();
    let error = store
        .set_attachment_details(&photo, &changes(json!({"name": "x"})), &later())
        .unwrap_err();
    assert_eq!(code(error), ErrorCode::MessageInTrash);
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
    assert_eq!(code(error), ErrorCode::ReadOnly);

    // A purged entry's attachment is gone.
    store.delete_message(&sent.id, &now()).unwrap();
    store.purge_message(&sent.id, &now()).unwrap();
    let error = store
        .set_attachment_details(&pdf, &changes(json!({"name": "x"})), &later())
        .unwrap_err();
    assert_eq!(code(error), ErrorCode::AttachmentNotFound);
}

#[test]
fn a_draft_tile_refuses_garbage_and_belongs_to_the_draft_only() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let source = dir.path().join("src");
    std::fs::create_dir_all(&source).unwrap();
    let file = common::attach::write(&source, "menu.pdf", b"%PDF-1.4");
    let tile = store.attach_path(INBOX_ID, &file, &now()).unwrap();
    let columns = |dir: &std::path::Path| {
        raw(dir)
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
            .unwrap()
    };
    let before = columns(dir.path());
    for bad in [
        json!({"spoiler": true}),
        json!({"spoiler": false}),
        json!({"name": "a/b"}),
        json!({"description": "x".repeat(1025)}),
    ] {
        let error = store
            .set_draft_attachment_details(&tile.id, &changes(bad))
            .unwrap_err();
        assert_eq!(code(error), ErrorCode::AttachmentDetailsInvalid);
    }
    for id in ["", "not-a-uuid", "00000000-0000-4000-8000-000000000000"] {
        let error = store
            .set_draft_attachment_details(id, &changes(json!({"name": "x"})))
            .unwrap_err();
        assert_eq!(code(error), ErrorCode::AttachmentNotFound);
    }
    assert_eq!(columns(dir.path()), before);
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
