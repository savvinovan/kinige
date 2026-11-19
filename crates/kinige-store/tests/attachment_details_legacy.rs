//! Slice 7b and the rows written before it (the guardian's vectors): unknown fields of a details group survive a local edit, a "no
//! change" writes nothing to a row that has no group (the invariant: no default-valued group is ever written to an existing row), a row
//! whose two maps name different groups takes no edit at all, other writes leave such a row byte-identical, and an audio takes no
//! details.

#![cfg(test)]

mod common;

use common::days::at;
use common::details::*;
use common::import::{parse, run, saved_n};
use common::ops::clock;
use common::voice::{CLIP_MS, m4a};
use common::*;
use kinige_domain::ErrorCode;
use kinige_domain::attachment::AttachmentKind;
use kinige_domain::groups::INBOX_ID;
use kinige_store::Now;
use serde_json::json;
use std::path::Path;

fn now() -> Now {
    at(2026, 11, 18, 12, 0)
}

fn later() -> Now {
    at(2026, 11, 18, 12, 5)
}

/// Everything of one attachment row and the clock, verbatim.
fn snapshot(dir: &Path, id: &str) -> impl PartialEq + std::fmt::Debug {
    (groups(dir, id), hlcs(dir, id), columns(dir, id), clock(dir))
}

#[test]
fn unknown_fields_of_a_details_group_survive_a_local_rewrite() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let photo = photo_and_pdf(&store, dir.path(), &now()).attachments[0]
        .id
        .clone();
    set_group(
        dir.path(),
        &photo,
        "privacy",
        r#"{"future":{"a":[1,2]},"spoiler":true}"#,
        1,
    );
    set_group(
        dir.path(),
        &photo,
        "name",
        r#"{"name":"beach.jpg","style":"bold"}"#,
        1,
    );
    store
        .set_attachment_details(
            &photo,
            &changes(json!({"spoiler": false, "name": "sea"})),
            &later(),
        )
        .unwrap();
    let after = groups(dir.path(), &photo);
    assert_eq!(
        after["privacy"],
        r#"{"future":{"a":[1,2]},"spoiler":false}"#
    );
    assert_eq!(after["name"], r#"{"name":"sea.jpg","style":"bold"}"#);
}

#[test]
fn a_change_to_what_a_legacy_image_already_reads_as_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let photo = photo_and_pdf(&store, dir.path(), &now()).attachments[0]
        .id
        .clone();
    make_legacy(dir.path(), &photo);
    let before = snapshot(dir.path(), &photo);
    // The most direct vector of the invariant: `{"spoiler": false}` on a row with no `privacy` group must not write `{"spoiler":false}`.
    store
        .set_attachment_details(&photo, &changes(json!({"spoiler": false})), &later())
        .unwrap();
    store
        .set_attachment_details(
            &photo,
            &changes(json!({"spoiler": false, "name": "IMG_0001", "description": " "})),
            &later(),
        )
        .unwrap();
    store
        .set_attachment_details(
            &photo,
            &changes(json!({"name": null, "description": null})),
            &later(),
        )
        .unwrap();
    assert_eq!(snapshot(dir.path(), &photo), before);
    assert!(!groups(dir.path(), &photo).contains_key("privacy"));
}

#[test]
fn a_row_whose_two_maps_name_different_groups_takes_no_edit() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let photo = photo_and_pdf(&store, dir.path(), &now()).attachments[0]
        .id
        .clone();
    make_legacy(dir.path(), &photo);
    // `privacy` is in the bytes and not in `hlc_json` (a record that could never become an envelope).
    let conn = raw(dir.path());
    let bytes: String = conn
        .query_row(
            "SELECT groups_raw FROM attachment WHERE id = ?1",
            [&photo],
            |r| r.get(0),
        )
        .unwrap();
    let mut map: std::collections::BTreeMap<String, serde_json::Value> =
        serde_json::from_str(&bytes).unwrap();
    map.insert("privacy".into(), json!({"spoiler": false}));
    conn.execute(
        "UPDATE attachment SET groups_raw = ?1 WHERE id = ?2",
        (serde_json::to_string(&map).unwrap(), &photo),
    )
    .unwrap();
    drop(conn);
    let before = snapshot(dir.path(), &photo);
    // The row reads as covered, so hiding it again is no change and writes nothing.
    store
        .set_attachment_details(&photo, &changes(json!({"spoiler": true})), &later())
        .unwrap();
    assert_eq!(snapshot(dir.path(), &photo), before);
    for edit in [
        json!({"spoiler": false}),
        json!({"name": "sea"}),
        json!({"description": "x"}),
        json!({"name": "sea", "description": "x", "spoiler": true}),
    ] {
        let error = store
            .set_attachment_details(&photo, &changes(edit.clone()), &later())
            .unwrap_err();
        // A group is added to a record that cannot be understood: refused as corruption, never written.
        assert_eq!(error.code(), ErrorCode::Internal, "{edit}");
        assert_eq!(snapshot(dir.path(), &photo), before, "{edit}");
    }
}

#[test]
fn a_send_a_voice_note_and_an_import_leave_a_legacy_row_byte_identical() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let photo = photo_and_pdf(&store, dir.path(), &now()).attachments[0]
        .id
        .clone();
    make_legacy(dir.path(), &photo);
    let before = (
        groups(dir.path(), &photo),
        hlcs(dir.path(), &photo),
        columns(dir.path(), &photo),
    );
    let same = |what: &str| {
        assert_eq!(
            (
                groups(dir.path(), &photo),
                hlcs(dir.path(), &photo),
                columns(dir.path(), &photo)
            ),
            before,
            "{what} changed the legacy row"
        );
    };

    photo_and_pdf(&store, dir.path(), &later());
    same("a send");
    store
        .send_voice(INBOX_ID, None, &m4a(4096), CLIP_MS, &later())
        .unwrap();
    same("a voice note");
    run(&store, &parse(&saved_n(2)), &[]).unwrap();
    same("a Telegram import");
}

#[test]
fn an_audio_takes_no_details_in_either_command() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let source = dir.path().join("src");
    std::fs::create_dir_all(&source).unwrap();
    let tile = store
        .attach_path(
            INBOX_ID,
            &common::attach::write(&source, "song.mp3", b"ID3fake"),
            &now(),
        )
        .unwrap();
    assert_eq!(tile.kind, AttachmentKind::Audio);
    let draft_before = columns_of_draft(dir.path(), &tile.id);
    for edit in [
        json!({"name": "renamed"}),
        json!({"description": "about it"}),
        json!({"name": null}),
        json!({"spoiler": true}),
    ] {
        let error = store
            .set_draft_attachment_details(&tile.id, &changes(edit.clone()))
            .unwrap_err();
        assert_eq!(error.code(), ErrorCode::AttachmentDetailsInvalid, "{edit}");
    }
    assert_eq!(columns_of_draft(dir.path(), &tile.id), draft_before);

    let sent = store
        .send_entry(INBOX_ID, "", None, std::slice::from_ref(&tile.id), &now())
        .unwrap();
    let id = sent.attachments[0].id.clone();
    let before = snapshot(dir.path(), &id);
    for edit in [
        json!({"name": "again"}),
        json!({"description": "x"}),
        json!({"name": null, "description": null}),
        json!({"spoiler": true}),
    ] {
        let error = store
            .set_attachment_details(&id, &changes(edit.clone()), &later())
            .unwrap_err();
        assert_eq!(error.code(), ErrorCode::AttachmentDetailsInvalid, "{edit}");
        assert_eq!(snapshot(dir.path(), &id), before, "{edit}");
    }
    // Nothing changed is fine, and writes nothing.
    store
        .set_attachment_details(&id, &changes(json!({})), &later())
        .unwrap();
    assert_eq!(snapshot(dir.path(), &id), before);
}

fn columns_of_draft(dir: &Path, id: &str) -> (Option<String>, Option<String>, i64) {
    raw(dir)
        .query_row(
            "SELECT display_name, description, spoiler FROM draft_attachment WHERE attachment_id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap()
}
