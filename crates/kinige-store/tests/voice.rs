//! Voice notes in the store (slice 7a, ADR-0015): `Store::send_voice` on good and garbage input, the container by magic bytes, the
//! name, the empty-body entry with one `audio` attachment, a reply and pending tiles, a crash between the rename and the commit,
//! and a full disk. The rest (search, the Markdown line, summaries, the forest, a kill) is in `voice_surfaces.rs`.

#![cfg(test)]

mod common;

use common::attach::*;
use common::voice::*;
use common::*;
use kinige_domain::ReplyState;
use kinige_domain::attachment::{AttachmentKind, MAX_PASTE_BYTES, is_voice_file_name};
use kinige_domain::groups::INBOX_ID;
use kinige_store::{AttachFault, StoreError};
use std::sync::Arc;

#[test]
fn a_recording_becomes_an_entry_with_no_text_and_one_audio_attachment() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let bytes = m4a(4096);
    let sent = store
        .send_voice(INBOX_ID, None, &bytes, CLIP_MS, &now())
        .unwrap();

    assert_eq!(sent.body_md, "");
    assert_eq!(sent.attachments.len(), 1);
    let voice = &sent.attachments[0];
    assert_eq!(voice.kind, AttachmentKind::Audio);
    assert_eq!(voice.mime, "audio/mp4");
    assert_eq!(voice.duration_ms, Some(CLIP_MS));
    assert_eq!(voice.byte_size, 4096);
    assert_eq!(voice.filename, "voice-2026-11-03-213000.m4a", "local time");
    assert!(is_voice_file_name(&voice.filename));
    assert_eq!(stored(dir.path()), [format!("{}.m4a", voice.id)]);
    assert_eq!(
        std::fs::read(dir.path().join(format!("attachments/{}.m4a", voice.id))).unwrap(),
        bytes
    );

    // The row carries the duration in the synced `core` group, and the note is in the feed.
    let core: String = raw(dir.path())
        .query_row(
            "SELECT json_extract(groups_raw, '$.core') FROM attachment WHERE id = ?1",
            [&voice.id],
            |row| row.get(0),
        )
        .unwrap();
    assert!(core.contains("\"duration_ms\":40000"), "{core}");
    let feed = store.list_messages(INBOX_ID, None, 50, &now()).unwrap();
    assert_eq!(feed.messages.len(), 1);
    assert_eq!(feed.messages[0].attachments[0].id, voice.id);
}

#[test]
fn a_webm_recording_is_audio_not_video_and_keeps_its_extension() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let sent = store
        .send_voice(INBOX_ID, None, &webm(2048), CLIP_MS, &now())
        .unwrap();
    let voice = &sent.attachments[0];
    assert_eq!(voice.kind, AttachmentKind::Audio);
    assert_eq!(voice.mime, "audio/webm");
    assert_eq!(voice.filename, "voice-2026-11-03-213000.webm");
    assert_eq!(
        count(
            dir.path(),
            "SELECT COUNT(*) FROM attachment WHERE kind = 'audio'"
        ),
        1
    );
}

#[test]
fn the_container_is_decided_by_the_bytes_not_by_anything_the_caller_says() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("empty", Vec::new()),
        ("one byte", vec![0x1A]),
        ("eleven bytes of ftyp-less data", vec![1; 11]),
        ("1 MiB of noise", noise(1024 * 1024)),
        ("a PNG", png(16, 16)),
        ("a JPEG", jpeg(16, 16)),
        ("html", b"<html><script>alert(1)</script></html>".to_vec()),
        ("ftyp at the wrong offset", {
            let mut bytes = b"ftyp".to_vec();
            bytes.resize(64, 0);
            bytes
        }),
    ];
    for (name, bytes) in cases {
        let error = store
            .send_voice(INBOX_ID, None, &bytes, CLIP_MS, &now())
            .unwrap_err();
        assert!(
            matches!(error, StoreError::AttachmentUnreadable),
            "{name}: {error}"
        );
        assert_nothing_written(dir.path());
    }
}

#[test]
fn the_duration_must_be_one_second_to_thirty_minutes_and_five_seconds() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    for ms in [i64::MIN, -1, 0, 1, 999] {
        let error = store
            .send_voice(INBOX_ID, None, &m4a(64), ms, &now())
            .unwrap_err();
        assert!(matches!(error, StoreError::VoiceTooShort), "{ms}: {error}");
    }
    for ms in [1_805_001, 3_600_000, i64::MAX] {
        let error = store
            .send_voice(INBOX_ID, None, &m4a(64), ms, &now())
            .unwrap_err();
        assert!(matches!(error, StoreError::VoiceTooLong), "{ms}: {error}");
    }
    assert_nothing_written(dir.path());
    for ms in [1_000, 1_805_000] {
        store
            .send_voice(INBOX_ID, None, &m4a(64), ms, &now())
            .unwrap();
    }
    assert_eq!(count(dir.path(), "SELECT COUNT(*) FROM attachment"), 2);
}

#[test]
fn more_than_64_mib_is_refused_before_anything_is_copied() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let mut huge = m4a(MAX_PASTE_BYTES + 1);
    let error = store
        .send_voice(INBOX_ID, None, &huge, CLIP_MS, &now())
        .unwrap_err();
    assert!(matches!(error, StoreError::AttachmentTooLarge), "{error}");
    assert_nothing_written(dir.path());
    // Exactly the cap is accepted.
    huge.truncate(MAX_PASTE_BYTES);
    store
        .send_voice(INBOX_ID, None, &huge, CLIP_MS, &now())
        .unwrap();
}

#[test]
fn an_unknown_chat_and_a_missing_reply_target_write_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let error = store
        .send_voice(
            "00000000-0000-4000-8000-0000000000aa",
            None,
            &m4a(64),
            CLIP_MS,
            &now(),
        )
        .unwrap_err();
    assert!(matches!(error, StoreError::ChatNotFound), "{error}");
    assert_nothing_written(dir.path());

    for target in ["00000000-0000-4000-8000-0000000000bb", "not-an-id", ""] {
        let error = store
            .send_voice(INBOX_ID, Some(target), &m4a(64), CLIP_MS, &now())
            .unwrap_err();
        assert!(
            matches!(error, StoreError::ReplyTargetNotFound),
            "{target:?}: {error}"
        );
        assert_nothing_written(dir.path());
    }
}

#[test]
fn a_reply_is_carried_and_pending_tiles_and_the_typed_text_stay_in_the_draft() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let original = store
        .send_message(INBOX_ID, "the question", &now())
        .unwrap();
    store
        .save_chat_draft(INBOX_ID, "half a thought", Some(&original.id), &now())
        .unwrap();
    let tile = store
        .attach_bytes(INBOX_ID, Some("a.png"), &png(12, 12), &now())
        .unwrap();

    let sent = store
        .send_voice(INBOX_ID, Some(&original.id), &m4a(256), CLIP_MS, &now())
        .unwrap();
    assert_eq!(
        sent.reply_to.as_ref().map(|r| r.id.as_str()),
        Some(original.id.as_str())
    );
    assert_eq!(sent.reply_to.unwrap().state, ReplyState::Live);

    let draft = store.chat_draft(INBOX_ID, &now()).unwrap();
    assert!(draft.reply_to.is_none(), "the note took the reply line");
    assert_eq!(draft.body, "half a thought", "the typed text is untouched");
    assert_eq!(draft.attachments.len(), 1, "the tile survives");
    assert_eq!(draft.attachments[0].id, tile.id);
}

#[test]
fn a_note_sent_without_a_reply_leaves_the_reply_line_of_the_draft_alone() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let original = store
        .send_message(INBOX_ID, "the question", &now())
        .unwrap();
    // Sending a text entry clears the draft, so the second target is made before the draft is.
    let other = store.send_message(INBOX_ID, "another", &now()).unwrap();
    store
        .save_chat_draft(INBOX_ID, "", Some(&original.id), &now())
        .unwrap();
    store.delete_message(&original.id, &now()).unwrap();

    // The window sends no reply when the target is in the trash; the quote stays in the field as the person left it.
    let sent = store
        .send_voice(INBOX_ID, None, &m4a(256), CLIP_MS, &now())
        .unwrap();
    assert!(sent.reply_to.is_none());
    let draft = store.chat_draft(INBOX_ID, &now()).unwrap();
    assert_eq!(
        draft.reply_to.map(|r| r.id),
        Some(original.id.clone()),
        "the reply line of the draft is still there"
    );
    assert_eq!(count(dir.path(), "SELECT COUNT(*) FROM draft"), 1);

    // A note that carries a different reply does not clear a line that points elsewhere either.
    store
        .send_voice(INBOX_ID, Some(&other.id), &m4a(256), CLIP_MS, &now())
        .unwrap();
    let draft = store.chat_draft(INBOX_ID, &now()).unwrap();
    assert_eq!(draft.reply_to.map(|r| r.id), Some(original.id));
}

#[test]
fn a_draft_that_held_only_the_reply_is_deleted() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let original = store
        .send_message(INBOX_ID, "the question", &now())
        .unwrap();
    store
        .save_chat_draft(INBOX_ID, "", Some(&original.id), &now())
        .unwrap();
    assert_eq!(count(dir.path(), "SELECT COUNT(*) FROM draft"), 1);
    store
        .send_voice(INBOX_ID, Some(&original.id), &m4a(256), CLIP_MS, &now())
        .unwrap();
    assert_eq!(count(dir.path(), "SELECT COUNT(*) FROM draft"), 0);
}

#[test]
fn a_crash_between_the_rename_and_the_commit_leaves_no_half_rows_and_the_sweep_takes_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    store.inject_attach_fault_for_test(AttachFault::AfterRename(Arc::new(|| {
        std::panic::resume_unwind(Box::new("simulated crash"));
    })));
    let error = store
        .send_voice(INBOX_ID, None, &m4a(512), CLIP_MS, &now())
        .unwrap_err();
    assert!(matches!(error, StoreError::Internal(_)), "{error}");
    store.inject_attach_fault_for_test(AttachFault::Clear);

    assert_eq!(count(dir.path(), "SELECT COUNT(*) FROM message"), 0);
    assert_eq!(count(dir.path(), "SELECT COUNT(*) FROM attachment"), 0);
    assert_eq!(
        count(dir.path(), "SELECT COUNT(*) FROM attachment_local"),
        0
    );
    assert_eq!(count(dir.path(), "SELECT COUNT(*) FROM search_map"), 0);
    assert_eq!(count(dir.path(), "SELECT COUNT(*) FROM mirror_dirty"), 0);
    assert_eq!(stored(dir.path()).len(), 1, "the renamed file has no row");

    let report = store.sweep_attachments(&now()).unwrap();
    assert_eq!(report.orphaned, 1, "{report:?}");
    assert!(stored(dir.path()).is_empty());
    // The store still works.
    store
        .send_voice(INBOX_ID, None, &m4a(512), CLIP_MS, &now())
        .unwrap();
    assert_eq!(count(dir.path(), "SELECT COUNT(*) FROM attachment"), 1);
}

#[test]
fn a_full_disk_rolls_everything_back() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    store.inject_attach_fault_for_test(AttachFault::DiskFullAfter(100));
    let error = store
        .send_voice(INBOX_ID, None, &m4a(4096), CLIP_MS, &now())
        .unwrap_err();
    assert!(matches!(error, StoreError::DiskFull), "{error}");
    store.inject_attach_fault_for_test(AttachFault::Clear);
    assert_nothing_written(dir.path());
}
