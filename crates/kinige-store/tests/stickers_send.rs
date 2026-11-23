//! Slice 5c, sending (ADR-0021, appendix §C.10): a click on a sticker is one entry with an empty body and a copy of the library file,
//! written with the `sticker` group at the creation HLC; the draft's text and tiles stay; a failing re-probe refuses the command; an entry
//! drawn as a sticker is never edited, detailed or appended to; a GIF sent again is a new attachment with a fresh blob id and key.

#![cfg(test)]

mod common;

use common::attach::*;
use common::details::{changes, columns, groups, hlcs};
use common::stickers::*;
use common::*;
use kinige_domain::ErrorCode;
use kinige_domain::groups::INBOX_ID;
use kinige_store::{AttachFault, Store, StoreError};
use serde_json::json;
use std::path::Path;
use std::sync::Arc;

fn now() -> kinige_store::Now {
    moscow(T0)
}

/// A store with a pack of two stickers; the first one's id.
fn with_pack() -> (tempfile::TempDir, Store, Vec<String>) {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    add_pack(&store, Some("TestPack"), "Test Pack", &drawn(2)).unwrap();
    let ids = ids_of(&store.sticker_library(false).unwrap(), "Test Pack");
    (dir, store, ids)
}

fn attachment_rows(dir: &Path) -> i64 {
    count(dir, "SELECT COUNT(*) FROM attachment")
}

#[test]
fn a_sticker_is_sent_at_once_as_its_own_entry_with_the_group_at_the_creation_hlc() {
    let (dir, store, ids) = with_pack();
    let view = store.send_sticker(INBOX_ID, &ids[0], None, &now()).unwrap();
    assert_eq!(view.body_md, "");
    let [a] = view.attachments.as_slice() else {
        panic!("one attachment");
    };
    assert!(a.sticker && !a.spoiler);
    assert_eq!(a.sticker_emoji.as_deref(), Some("😺"));
    assert_eq!(
        (a.filename.as_str(), a.mime.as_str()),
        ("sticker.webp", "image/webp")
    );
    assert_eq!((a.width, a.height), (Some(200), Some(160)));
    let all = groups(dir.path(), &a.id);
    assert_eq!(all["sticker"], r#"{"emoji":"😺","sticker":true}"#);
    let stamps = hlcs(dir.path(), &a.id);
    let hlc = stamps["core"]["h"].clone();
    for group in [
        "core",
        "order",
        "deleted",
        "name",
        "description",
        "privacy",
        "sticker",
    ] {
        assert_eq!(stamps[group], json!({"h": hlc, "sv": 1}), "{group}");
    }
    let row: (i64, Option<String>) = raw(dir.path())
        .query_row(
            "SELECT sticker, sticker_emoji FROM attachment WHERE id = ?1",
            [&a.id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(row, (1, Some("😺".to_string())));
    // The entry owns its file; the library's is untouched; the sticker leads the recent strip.
    assert_eq!(library_files(dir.path()).len(), 2);
    assert_eq!(store.sticker_library(false).unwrap().recent[0].id, ids[0]);
    let again = store.send_sticker(INBOX_ID, &ids[1], None, &now()).unwrap();
    assert!(again.attachments[0].sticker);
    let recent: Vec<String> = store
        .sticker_library(false)
        .unwrap()
        .recent
        .iter()
        .map(|s| s.id.clone())
        .collect();
    assert_eq!(recent, vec![ids[1].clone(), ids[0].clone()]);
}

#[test]
fn the_draft_text_and_tiles_stay_and_a_pending_reply_goes_with_the_sticker() {
    let (dir, store, ids) = with_pack();
    let source = dir.path().join("src");
    std::fs::create_dir_all(&source).unwrap();
    let target = store.send_message(INBOX_ID, "вопрос", &now()).unwrap();
    let tile = store
        .attach_path(INBOX_ID, &write(&source, "a.jpg", &jpeg(20, 20)), &now())
        .unwrap();
    store
        .save_chat_draft(INBOX_ID, "ещё не отправил", Some(&target.id), &now())
        .unwrap();
    let sent = store
        .send_sticker(INBOX_ID, &ids[0], Some(&target.id), &now())
        .unwrap();
    assert_eq!(
        sent.reply_to.as_ref().map(|r| r.id.as_str()),
        Some(target.id.as_str())
    );
    let draft = store.chat_draft(INBOX_ID, &now()).unwrap();
    assert_eq!(draft.body, "ещё не отправил");
    assert!(draft.reply_to.is_none(), "the reply was used");
    assert_eq!(draft.attachments.len(), 1);
    assert_eq!(draft.attachments[0].id, tile.id);
}

#[test]
fn bad_ids_and_a_missing_chat_or_reply_target_write_nothing() {
    let (dir, store, ids) = with_pack();
    let before = attachment_rows(dir.path());
    for id in ["", "x", "../x", "00000000-0000-4000-8000-000000000000"] {
        let error = store.send_sticker(INBOX_ID, id, None, &now()).unwrap_err();
        assert_eq!(error.code(), ErrorCode::StickerNotFound, "{id:?}");
    }
    let error = store.send_sticker(
        "00000000-0000-4000-8000-0000000000aa",
        &ids[0],
        None,
        &now(),
    );
    assert!(matches!(error, Err(StoreError::ChatNotFound)));
    let error = store.send_sticker(
        INBOX_ID,
        &ids[0],
        Some("00000000-0000-4000-8000-0000000000bb"),
        &now(),
    );
    assert!(matches!(error, Err(StoreError::ReplyTargetNotFound)));
    assert_eq!(attachment_rows(dir.path()), before);
    assert_eq!(count(dir.path(), "SELECT COUNT(*) FROM message"), 0);
    assert!(
        names(&dir.path().join("attachments"))
            .iter()
            .all(|n| n.starts_with('.'))
    );
}

#[test]
fn a_library_file_that_fails_the_probe_is_refused_and_never_sent_as_a_photo() {
    let (dir, store, ids) = with_pack();
    let file = store.sticker_file(&ids[0]).unwrap().unwrap().0;
    // Swapped for a JPEG, an animated WebP, a too large WebP and garbage: each refuses the command.
    for bytes in [
        jpeg(64, 64),
        animated_webp(64, 64),
        webp(600, 100),
        b"x".to_vec(),
        Vec::new(),
    ] {
        std::fs::write(&file, &bytes).unwrap();
        let error = store
            .send_sticker(INBOX_ID, &ids[0], None, &now())
            .unwrap_err();
        assert!(matches!(error, StoreError::StickerUnreadable), "{error}");
    }
    assert_eq!(attachment_rows(dir.path()), 0);
    assert_eq!(count(dir.path(), "SELECT COUNT(*) FROM message"), 0);
    // A file that is gone is "not found".
    std::fs::remove_file(&file).unwrap();
    let error = store
        .send_sticker(INBOX_ID, &ids[0], None, &now())
        .unwrap_err();
    assert!(matches!(error, StoreError::StickerNotFound), "{error}");
    assert!(
        names(&dir.path().join("stickers/.tmp"))
            .iter()
            .all(|p| names(&dir.path().join("stickers/.tmp").join(p)).is_empty())
    );
}

#[test]
fn a_crash_between_the_rename_and_the_commit_leaves_no_entry() {
    let (dir, store, ids) = with_pack();
    store.inject_attach_fault_for_test(AttachFault::AfterRename(Arc::new(|| {
        std::panic::resume_unwind(Box::new("simulated crash"));
    })));
    assert!(store.send_sticker(INBOX_ID, &ids[0], None, &now()).is_err());
    store.inject_attach_fault_for_test(AttachFault::Clear);
    assert_eq!(count(dir.path(), "SELECT COUNT(*) FROM message"), 0);
    assert_eq!(attachment_rows(dir.path()), 0);
    assert!(
        store.sticker_library(false).unwrap().recent.is_empty(),
        "no recent without the entry"
    );
    assert_eq!(library_files(dir.path()).len(), 2);
}

#[test]
fn an_entry_drawn_as_a_sticker_is_never_edited_detailed_or_appended_to() {
    let (dir, store, ids) = with_pack();
    let sent = store.send_sticker(INBOX_ID, &ids[0], None, &now()).unwrap();
    let id = sent.attachments[0].id.clone();
    let error = store.edit_message(&sent.id, "text", &now()).unwrap_err();
    assert_eq!(error.code(), ErrorCode::StickerEntryReadOnly);
    for change in [
        json!({"name": "x"}),
        json!({"description": "d"}),
        json!({"spoiler": true}),
        json!({}),
    ] {
        let error = store
            .set_attachment_details(&id, &changes(change), &now())
            .unwrap_err();
        assert_eq!(error.code(), ErrorCode::StickerEntryReadOnly);
    }
    // The capture window's ⌘Enter never appends to it: the text becomes a new entry.
    let view = store.capture_view(&now()).unwrap();
    assert_eq!(view.append_target, None);
    let outcome = store.capture_append("мысль", &now()).unwrap();
    assert_eq!(outcome.action, kinige_domain::CaptureAction::Created);
    assert_eq!(count(dir.path(), "SELECT COUNT(*) FROM message"), 2);
    // Nothing of the sticker's bytes changed.
    assert_eq!(
        groups(dir.path(), &id)["sticker"],
        r#"{"emoji":"😺","sticker":true}"#
    );
}

#[test]
fn a_covered_sticker_is_not_a_sticker_entry_and_its_cover_can_be_removed() {
    let (dir, store, ids) = with_pack();
    let sent = store.send_sticker(INBOX_ID, &ids[0], None, &now()).unwrap();
    let id = sent.attachments[0].id.clone();
    // A cover that came from another device: the privacy group says so.
    raw(dir.path())
        .execute(
            "UPDATE attachment SET spoiler = 1, groups_raw = replace(groups_raw, '{\"spoiler\":false}', '{\"spoiler\":true}') WHERE id = ?1",
            [&id],
        )
        .unwrap();
    let page = store.list_messages(INBOX_ID, None, 10, &now()).unwrap();
    let view = &page.messages[0];
    assert!(view.attachments[0].spoiler && view.attachments[0].sticker);
    let shown = store
        .set_attachment_details(&id, &changes(json!({"spoiler": false})), &now())
        .unwrap();
    assert!(!shown.spoiler);
    assert_eq!(columns(dir.path(), &id).2, 0);
    // Uncovered, it is a sticker entry again.
    assert!(
        store
            .set_attachment_details(&id, &changes(json!({"name": "x"})), &now())
            .is_err()
    );
}

#[test]
fn a_gif_sent_again_is_a_new_attachment_with_a_fresh_blob_id_and_key_and_no_details() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let source = dir.path().join("src");
    std::fs::create_dir_all(&source).unwrap();
    let tile = store
        .attach_path(INBOX_ID, &write(&source, "cat.gif", &gif(32, 24)), &now())
        .unwrap();
    store
        .set_draft_attachment_details(
            &tile.id,
            &changes(json!({"name": "kitty", "description": "a cat"})),
        )
        .unwrap();
    let original = store
        .send_entry(INBOX_ID, "", None, std::slice::from_ref(&tile.id), &now())
        .unwrap();
    let source_id = original.attachments[0].id.clone();
    let again = store
        .send_gif_again(INBOX_ID, &source_id, None, &moscow(T0 + MINUTE))
        .unwrap();
    let copy = &again.attachments[0];
    assert_ne!(copy.id, source_id);
    assert_eq!(
        (copy.mime.as_str(), copy.filename.as_str()),
        ("image/gif", "cat.gif")
    );
    assert!(!copy.sticker && copy.description.is_none() && copy.name == "cat.gif");
    let blob = |id: &str| -> (String, String) {
        raw(dir.path())
            .query_row(
                "SELECT remote_blob_id, blob_key FROM attachment WHERE id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap()
    };
    let (a, b) = (blob(&source_id), blob(&copy.id));
    assert!(a.0 != b.0 && a.1 != b.1);
    assert_eq!(
        groups(dir.path(), &copy.id)["sticker"],
        r#"{"emoji":null,"sticker":false}"#
    );
    assert_eq!(groups(dir.path(), &copy.id)["name"], r#"{"name":null}"#);
    // The tab lists one GIF (the same content), newest first.
    let listed = store.list_journal_gifs().unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].attachment_id, again.attachments[0].id);
    assert_eq!((listed[0].width, listed[0].height), (32, 24));
}

#[test]
fn a_covered_trashed_missing_or_not_gif_source_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let source = dir.path().join("src");
    std::fs::create_dir_all(&source).unwrap();
    let send = |name: &str, bytes: Vec<u8>| {
        let tile = store
            .attach_path(INBOX_ID, &write(&source, name, &bytes), &now())
            .unwrap();
        store
            .send_entry(INBOX_ID, "", None, &[tile.id], &now())
            .unwrap()
    };
    let covered = send("a.gif", gif(8, 8));
    let trashed = send("b.gif", gif(9, 9));
    let photo = send("c.png", png(8, 8));
    store
        .set_attachment_details(
            &covered.attachments[0].id,
            &changes(json!({"spoiler": true})),
            &now(),
        )
        .unwrap();
    store.delete_message(&trashed.id, &now()).unwrap();
    let refused = |id: &str| {
        store
            .send_gif_again(INBOX_ID, id, None, &now())
            .unwrap_err()
            .code()
    };
    assert_eq!(
        refused(&covered.attachments[0].id),
        ErrorCode::AttachmentNotFound
    );
    assert_eq!(
        refused(&trashed.attachments[0].id),
        ErrorCode::AttachmentNotFound
    );
    assert_eq!(
        refused(&photo.attachments[0].id),
        ErrorCode::AttachmentNotFound
    );
    for id in ["", "x", "00000000-0000-4000-8000-000000000000"] {
        assert_eq!(refused(id), ErrorCode::AttachmentNotFound, "{id:?}");
    }
    assert!(
        store.list_journal_gifs().unwrap().is_empty(),
        "covered and trashed GIFs are not listed"
    );
    assert_eq!(count(dir.path(), "SELECT COUNT(*) FROM message"), 3);
}
