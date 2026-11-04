//! Attachments (slice 7): attaching to a draft, sending, the rows and their groups, the feed view, the Markdown copy, search by
//! file name, the trash and "Delete forever". The failure cases of the pipeline are in `attachments_pipeline.rs`.

#![cfg(test)]

mod common;

use common::attach::*;
use common::days::at;
use common::*;
use kinige_domain::attachment::{AttachmentKind, AttachmentState, is_hex64, is_uuid_v7};
use kinige_domain::groups::INBOX_ID;
use kinige_domain::hlc::is_uuid_v4;
use kinige_store::{Now, Store, StoreError};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// `(id, sort_index, hlc_json, groups_raw, remote_blob_id, blob_key, sha256, server_known)` of an `attachment` row.
type Row = (String, i64, String, String, String, String, String, i64);

fn now() -> Now {
    at(2026, 11, 3, 12, 0)
}

fn attach(
    store: &Store,
    dir: &std::path::Path,
    name: &str,
    bytes: &[u8],
) -> kinige_domain::AttachmentView {
    let source = dir.join("src");
    std::fs::create_dir_all(&source).unwrap();
    let path = write(&source, name, bytes);
    store.attach_path(INBOX_ID, &path, &now()).unwrap()
}

fn ids(views: &[kinige_domain::AttachmentView]) -> Vec<String> {
    views.iter().map(|v| v.id.clone()).collect()
}

#[test]
fn a_photo_is_copied_hashed_and_described_before_it_is_sent() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let bytes = jpeg(640, 480);
    let view = attach(&store, dir.path(), "IMG_0001.JPG", &bytes);

    assert!(is_uuid_v4(&view.id));
    assert_eq!(view.kind, AttachmentKind::Image);
    assert_eq!(view.filename, "IMG_0001.JPG");
    assert_eq!(view.mime, "image/jpeg");
    assert_eq!((view.width, view.height), (Some(640), Some(480)));
    assert_eq!(view.byte_size, bytes.len() as i64);
    let stored = dir
        .path()
        .join("attachments")
        .join(format!("{}.jpg", view.id));
    assert_eq!(
        std::fs::read(&stored).unwrap(),
        bytes,
        "the stored file is a faithful copy"
    );
    // No half-copied file is left in the process's temporary directory.
    let tmp: Vec<String> = std::fs::read_dir(dir.path().join("attachments/.tmp"))
        .unwrap()
        .flatten()
        .flat_map(|d| names(&d.path()))
        .collect();
    assert!(tmp.is_empty(), "{tmp:?}");

    let conn = raw(dir.path());
    let (sha, size, file_name): (String, i64, String) = conn
        .query_row(
            "SELECT sha256, byte_size, file_name FROM draft_attachment WHERE attachment_id = ?1",
            [&view.id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(
        sha,
        kinige_domain::attachment::hex_lower(Sha256::digest(&bytes).as_slice())
    );
    assert_eq!(
        (size, file_name),
        (bytes.len() as i64, format!("{}.jpg", view.id))
    );

    let draft = store.chat_draft(INBOX_ID, &now()).unwrap();
    assert_eq!(ids(&draft.attachments), vec![view.id.clone()]);
    assert_eq!(draft.body, "");
}

#[test]
fn sending_turns_the_draft_attachments_into_synced_rows_with_their_own_keys() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let a = attach(&store, dir.path(), "a.jpg", &jpeg(100, 50));
    let b = attach(&store, dir.path(), "menu.pdf", b"%PDF-1.4 fake");
    store.set_chat_draft(INBOX_ID, "подпись", &now()).unwrap();

    let sent = store
        .send_entry(
            INBOX_ID,
            "подпись",
            None,
            &[b.id.clone(), a.id.clone()],
            &now(),
        )
        .unwrap();
    assert_eq!(
        ids(&sent.attachments),
        vec![b.id.clone(), a.id.clone()],
        "in the order given"
    );
    assert_eq!(sent.attachments[0].kind, AttachmentKind::File);
    assert_eq!(sent.attachments[0].state, AttachmentState::Local);

    let conn = raw(dir.path());
    let mut statement = conn
        .prepare("SELECT id, sort_index, hlc_json, groups_raw, remote_blob_id, blob_key, sha256, server_known FROM attachment ORDER BY sort_index")
        .unwrap();
    let rows: Vec<Row> = statement
        .query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
                r.get(7)?,
            ))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(rows.len(), 2);
    let mut hlcs = Vec::new();
    let mut keys = Vec::new();
    for (index, (id, sort, hlc_json, groups_raw, blob_id, blob_key, sha, known)) in
        rows.iter().enumerate()
    {
        assert_eq!(*sort, index as i64);
        assert_eq!(id, &sent.attachments[index].id);
        assert_eq!(*known, 0);
        assert!(
            is_uuid_v7(blob_id),
            "remote_blob_id is a UUID v7: {blob_id}"
        );
        assert!(is_hex64(blob_key) && is_hex64(sha));
        let groups: Value = serde_json::from_str(groups_raw).unwrap();
        assert_eq!(groups.as_object().unwrap().len(), 3, "core, order, deleted");
        assert_eq!(groups["order"]["sort_index"], index);
        assert_eq!(groups["core"]["blob_key"], blob_key.as_str());
        assert_eq!(groups["core"]["remote_blob_id"], blob_id.as_str());
        assert_eq!(groups["core"]["message_id"], sent.id.as_str());
        let hlc: Value = serde_json::from_str(hlc_json).unwrap();
        let stamps: Vec<&str> = ["core", "order", "deleted"]
            .iter()
            .map(|g| hlc[g]["h"].as_str().unwrap())
            .collect();
        assert!(
            stamps.iter().all(|s| *s == stamps[0]),
            "one HLC for the three groups"
        );
        assert_eq!(hlc["core"]["sv"], 1);
        hlcs.push(stamps[0].to_string());
        keys.push(blob_key.clone());
    }
    assert_ne!(hlcs[0], hlcs[1], "every attachment has an HLC of its own");
    assert_ne!(keys[0], keys[1], "and a key of its own");

    // The draft and its tiles are gone; the files are in place and nothing else is in `attachments/`.
    let draft = store.chat_draft(INBOX_ID, &now()).unwrap();
    assert_eq!((draft.body.as_str(), draft.attachments.len()), ("", 0));
    let left: i64 = conn
        .query_row("SELECT COUNT(*) FROM draft_attachment", [], |r| r.get(0))
        .unwrap();
    assert_eq!(left, 0);
    let on_disk = names(&dir.path().join("attachments"));
    assert!(
        on_disk.contains(&format!("{}.jpg", a.id)) && on_disk.contains(&format!("{}.pdf", b.id)),
        "{on_disk:?}"
    );

    // The feed gives the same views back, in order.
    let page = store
        .list_messages_page(INBOX_ID, kinige_store::PageAt::Latest, 50, None, &now())
        .unwrap();
    assert_eq!(page.messages.len(), 1);
    assert_eq!(page.messages[0].attachments, sent.attachments);
}

#[test]
fn an_entry_can_be_sent_without_text_only_with_an_attachment() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    assert!(matches!(
        store.send_entry(INBOX_ID, "  \n ", None, &[], &now()),
        Err(StoreError::Empty)
    ));
    let photo = attach(&store, dir.path(), "p.png", &png(32, 32));
    let sent = store
        .send_entry(INBOX_ID, "", None, std::slice::from_ref(&photo.id), &now())
        .unwrap();
    assert_eq!(sent.body_md, "");
    let groups: Value = serde_json::from_str(
        &raw(dir.path())
            .query_row(
                "SELECT groups_raw FROM message WHERE id = ?1",
                [&sent.id],
                |r| r.get::<_, String>(0),
            )
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        groups["body"].to_string(),
        r#"{"base":null,"body_md":"","edited_at":null,"supersedes":[]}"#
    );

    // Appending from the capture window to this entry (the photo has no text yet): the addition is the whole text, with no blank
    // lines in front of it.
    let appended = store
        .capture_append("подпись к фото", &now())
        .expect("append to the entry without text");
    assert_eq!(appended.message.id, sent.id);
    assert_eq!(appended.message.body_md, "подпись к фото");
    assert_eq!(appended.message.attachments.len(), 1);

    // Editing: the text may be cleared while an attachment is there, and not on an entry without one.
    let edited = store
        .edit_message(&sent.id, "потом дописал", &now())
        .unwrap();
    assert_eq!(edited.body_md, "потом дописал");
    assert_eq!(
        store.edit_message(&sent.id, "", &now()).unwrap().body_md,
        ""
    );
    let plain = store
        .send_message(INBOX_ID, "просто текст", &now())
        .unwrap();
    assert!(matches!(
        store.edit_message(&plain.id, "  ", &now()),
        Err(StoreError::Empty)
    ));
}

#[test]
fn garbage_attachment_ids_write_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let other = store.create_chat("Другой", None, &now()).unwrap();
    let mine = attach(&store, dir.path(), "a.png", &png(8, 8));
    let path = write(dir.path(), "b.png", &png(8, 8));
    let foreign = store.attach_path(&other.id, &path, &now()).unwrap();

    let long = "x".repeat(1 << 20);
    let eleven: Vec<String> = (0..11).map(|_| mine.id.clone()).collect();
    let cases: Vec<(Vec<String>, &str)> = vec![
        (vec!["not-a-uuid".into()], "not a uuid"),
        (vec![long], "a megabyte"),
        (vec![String::new()], "empty"),
        (vec![foreign.id.clone()], "another chat's"),
        (vec![mine.id.clone(), mine.id.clone()], "duplicates"),
        (vec![mine.id.to_uppercase()], "upper case"),
        (
            vec!["00000000-0000-4000-8000-0000000000aa".into()],
            "unknown",
        ),
        (eleven, "eleven ids"),
    ];
    for (ids, why) in cases {
        let error = store
            .send_entry(INBOX_ID, "текст", None, &ids, &now())
            .expect_err(why);
        assert!(
            matches!(
                error,
                StoreError::AttachmentNotFound | StoreError::AttachmentLimit
            ),
            "{why}: {error}"
        );
    }
    let conn = raw(dir.path());
    for table in ["message", "attachment", "attachment_local"] {
        let n: i64 = conn
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
            .unwrap();
        // The Inbox has no messages; nothing was written by any of the rejected sends.
        assert_eq!(n, 0, "{table}");
    }
    let tiles: i64 = conn
        .query_row("SELECT COUNT(*) FROM draft_attachment", [], |r| r.get(0))
        .unwrap();
    assert_eq!(tiles, 2, "both tiles are still there");
    // The valid send still works afterwards.
    assert!(
        store
            .send_entry(INBOX_ID, "ok", None, std::slice::from_ref(&mine.id), &now())
            .is_ok()
    );
}

#[test]
fn the_eleventh_tile_is_refused_before_anything_is_copied() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    for n in 0..10 {
        attach(
            &store,
            dir.path(),
            &format!("f{n}.txt"),
            format!("file {n}").as_bytes(),
        );
    }
    let path = write(dir.path(), "eleven.txt", b"x");
    assert!(matches!(
        store.attach_path(INBOX_ID, &path, &now()),
        Err(StoreError::AttachmentLimit)
    ));
    assert!(matches!(
        store.attach_bytes(INBOX_ID, Some("p.png"), &png(4, 4), &now()),
        Err(StoreError::AttachmentLimit)
    ));
    assert_eq!(
        store
            .chat_draft(INBOX_ID, &now())
            .unwrap()
            .attachments
            .len(),
        10
    );
    assert!(matches!(
        store.attach_path("00000000-0000-4000-8000-000000000bad", &path, &now()),
        Err(StoreError::ChatNotFound)
    ));
}

#[test]
fn removing_a_tile_deletes_its_row_and_file_and_the_empty_draft_row() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let view = attach(&store, dir.path(), "a.gif", &gif(10, 10));
    let stored = dir
        .path()
        .join("attachments")
        .join(format!("{}.gif", view.id));
    assert!(stored.is_file());
    assert!(matches!(
        store.remove_draft_attachment(INBOX_ID, "nope"),
        Err(StoreError::AttachmentNotFound)
    ));
    store.remove_draft_attachment(INBOX_ID, &view.id).unwrap();
    assert!(!stored.exists());
    assert!(
        store
            .chat_draft(INBOX_ID, &now())
            .unwrap()
            .attachments
            .is_empty()
    );
    let rows: i64 = raw(dir.path())
        .query_row("SELECT COUNT(*) FROM draft", [], |r| r.get(0))
        .unwrap();
    assert_eq!(rows, 0, "nothing is left of the draft");
    // Saving an empty text keeps the draft while a tile is there.
    let again = attach(&store, dir.path(), "b.gif", &gif(10, 10));
    store.set_chat_draft(INBOX_ID, "", &now()).unwrap();
    assert_eq!(
        store
            .chat_draft(INBOX_ID, &now())
            .unwrap()
            .attachments
            .len(),
        1
    );
    store.remove_draft_attachment(INBOX_ID, &again.id).unwrap();
}

#[test]
fn pasted_bytes_get_a_clean_name_and_the_paste_limit() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let nameless = store
        .attach_bytes(INBOX_ID, None, &png(16, 16), &at(2026, 11, 3, 14, 5))
        .unwrap();
    assert_eq!(nameless.filename, "pasted-140500.png");
    assert_eq!(nameless.kind, AttachmentKind::Image);
    assert!(
        dir.path()
            .join("attachments")
            .join(format!("{}.png", nameless.id))
            .is_file()
    );
    let hostile = store
        .attach_bytes(
            INBOX_ID,
            Some("../../etc/pass\u{0}wd\u{202E}.png"),
            &png(16, 16),
            &now(),
        )
        .unwrap();
    assert_eq!(hostile.filename, "passwd.png");
    let bare_hint = store
        .attach_bytes(INBOX_ID, Some("image"), &png(16, 16), &now())
        .unwrap();
    assert_eq!(bare_hint.filename, "image.png");
    let too_big = vec![0u8; kinige_domain::attachment::MAX_PASTE_BYTES + 1];
    assert!(matches!(
        store.attach_bytes(INBOX_ID, Some("big.bin"), &too_big, &now()),
        Err(StoreError::AttachmentTooLarge)
    ));
}

#[test]
fn search_finds_an_entry_by_the_stem_of_a_file_name() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let menu = attach(&store, dir.path(), "меню_ресторана.pdf", b"%PDF-1.4");
    let sent = store
        .send_entry(INBOX_ID, "", None, std::slice::from_ref(&menu.id), &now())
        .unwrap();
    let other = store
        .send_message(INBOX_ID, "совсем другое", &now())
        .unwrap();
    for query in ["меню", "ресторан", "меню ресторана"] {
        let found = store.search(query, &Default::default(), None, 20).unwrap();
        let hits: Vec<&str> = found.hits.iter().map(|h| h.message.id.as_str()).collect();
        assert_eq!(hits, vec![sent.id.as_str()], "{query}");
    }
    let nothing = store.search("pdf", &Default::default(), None, 20).unwrap();
    assert!(nothing.hits.is_empty(), "the extension is not indexed");
    let _ = other;
}
