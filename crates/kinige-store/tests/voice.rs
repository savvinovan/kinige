//! Voice notes in the store (slice 7a, ADR-0015): `Store::send_voice` on good and garbage input, the container by magic bytes, the
//! name, the empty-body entry with one `audio` attachment, a reply and pending tiles, a crash between the rename and the commit,
//! a full disk, search by the word "voice", the Markdown line with the length, the summaries and survival of a kill.

#![cfg(test)]

mod common;

use common::attach::*;
use common::days::at;
use common::*;
use kinige_domain::attachment::{AttachmentKind, MAX_PASTE_BYTES, is_voice_file_name};
use kinige_domain::groups::INBOX_ID;
use kinige_domain::{ReplyState, SearchFilters};
use kinige_store::{AttachFault, Now, StoreError};
use std::path::Path;
use std::sync::Arc;

const CLIP_MS: i64 = 40_000;

fn now() -> Now {
    at(2026, 11, 3, 21, 30)
}

/// AAC in MP4 as far as the store looks: `ftyp` at offset 4, then bytes.
fn m4a(len: usize) -> Vec<u8> {
    let mut bytes = vec![0, 0, 0, 0x20];
    bytes.extend_from_slice(b"ftypM4A ");
    bytes.resize(len.max(12), 0x5a);
    bytes
}

/// Opus in WebM as far as the store looks: the EBML header.
fn webm(len: usize) -> Vec<u8> {
    let mut bytes = vec![0x1A, 0x45, 0xDF, 0xA3];
    bytes.resize(len.max(4), 0x33);
    bytes
}

/// Deterministic noise with no magic bytes at the start.
fn noise(len: usize) -> Vec<u8> {
    let mut state = 0x1234_5678_u32;
    (0..len)
        .map(|_| {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (state >> 24) as u8
        })
        .collect()
}

fn stored(dir: &Path) -> Vec<String> {
    names(&dir.join("attachments"))
        .into_iter()
        .filter(|name| !name.starts_with('.'))
        .collect()
}

fn count(dir: &Path, sql: &str) -> i64 {
    raw(dir).query_row(sql, [], |row| row.get(0)).unwrap()
}

/// Nothing of a refused note is left: no entry, no row, no file, no copy of the bytes in `.tmp`.
fn assert_nothing_written(dir: &Path) {
    assert_eq!(count(dir, "SELECT COUNT(*) FROM message"), 0, "no entry");
    assert_eq!(count(dir, "SELECT COUNT(*) FROM attachment"), 0, "no row");
    assert!(stored(dir).is_empty(), "no stored file");
    let tmp = dir.join("attachments/.tmp");
    let left: usize = names(&tmp)
        .iter()
        .map(|process| names(&tmp.join(process)).len())
        .sum();
    assert_eq!(left, 0, "no staged copy");
}

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

#[test]
fn a_recorder_file_is_found_by_the_word_voice_and_not_by_its_date() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let voice = store
        .send_voice(INBOX_ID, None, &m4a(256), CLIP_MS, &now())
        .unwrap();
    let found = |query: &str| {
        store
            .search(query, &SearchFilters::default(), None, 10)
            .unwrap()
            .hits
            .into_iter()
            .map(|hit| hit.message.id)
            .collect::<Vec<_>>()
    };
    assert_eq!(found("voice"), std::slice::from_ref(&voice.id));
    for noise in ["11", "2026", "213000", "03"] {
        assert!(
            found(noise).is_empty(),
            "{noise} must not find a voice note"
        );
    }
}

#[test]
fn the_markdown_copy_links_the_file_with_its_length() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path());
    let voice = store
        .send_voice(INBOX_ID, None, &m4a(256), CLIP_MS, &now())
        .unwrap();
    let day = mirror_dir(dir.path()).join("Inbox/2026/2026-11-03.md");
    wait_until("the day file with the voice note", || {
        std::fs::read_to_string(&day).is_ok_and(|text| text.contains("voice-2026-11-03-213000"))
    });
    let text = std::fs::read_to_string(&day).unwrap();
    assert!(
        text.contains(
            "[voice-2026-11-03-213000.m4a](<2026-11-03/voice-2026-11-03-213000.m4a>) (0:40)"
        ),
        "{text}"
    );
    wait_until("the file in the day folder", || {
        mirror_dir(dir.path())
            .join("Inbox/2026/2026-11-03/voice-2026-11-03-213000.m4a")
            .is_file()
    });
    drop(voice);
}

#[test]
fn the_summary_of_a_reply_quote_counts_voice_notes_and_their_length() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let voice = store
        .send_voice(INBOX_ID, None, &m4a(256), CLIP_MS, &now())
        .unwrap();
    store
        .save_chat_draft(INBOX_ID, "", Some(&voice.id), &now())
        .unwrap();
    let quote = store
        .chat_draft(INBOX_ID, &now())
        .unwrap()
        .reply_to
        .unwrap()
        .attachments;
    assert_eq!(
        (quote.photos, quote.files, quote.voices, quote.voice_ms),
        (0, 0, 1, Some(CLIP_MS))
    );
}

#[test]
fn the_forest_day_counts_voice_notes() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    store
        .send_message(INBOX_ID, "a plain note", &now())
        .unwrap();
    store
        .send_voice(INBOX_ID, None, &m4a(256), CLIP_MS, &now())
        .unwrap();
    let year = store
        .get_forest_year(2026, kinige_domain::forest::FirstWeekday::Monday, &now())
        .unwrap();
    let day = year.days.iter().find(|d| d.date == "2026-11-03").unwrap();
    assert_eq!((day.notes, day.voices, day.photos), (2, 1, 0));
}

const CHILD_ENV: &str = "KINIGE_VOICE_ABORT_CHILD_DIR";
const CHILD_TEST: &str = "a_sent_note_survives_a_kill_and_a_restart";

/// The child process sends a note and dies by `abort` right after the reply (after `COMMIT`); the parent reopens the data.
#[test]
fn a_sent_note_survives_a_kill_and_a_restart() {
    if let Ok(dir) = std::env::var(CHILD_ENV) {
        let store = open_without_mirror(dir.as_ref());
        store
            .send_voice(INBOX_ID, None, &m4a(1024), CLIP_MS, &now())
            .expect("send");
        std::process::abort();
    }

    let dir = tempfile::tempdir().unwrap();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([CHILD_TEST, "--exact", "--nocapture", "--test-threads=1"])
        .env(CHILD_ENV, dir.path())
        .status()
        .unwrap();
    assert!(!status.success(), "the child process should have died");

    let reopened = open_without_mirror(dir.path());
    let feed = reopened
        .list_messages(INBOX_ID, None, 50, &now())
        .unwrap()
        .messages;
    assert_eq!(feed.len(), 1);
    let voice = &feed[0].attachments[0];
    assert_eq!(voice.duration_ms, Some(CLIP_MS));
    assert_eq!(voice.state, kinige_domain::AttachmentState::Local);
    assert!(
        dir.path()
            .join(format!("attachments/{}.m4a", voice.id))
            .is_file()
    );
    assert_eq!(count(dir.path(), "SELECT COUNT(*) FROM mirror_dirty"), 1);
}

#[test]
fn the_keys_and_the_stored_name_never_reach_the_view() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let sent = store
        .send_voice(INBOX_ID, None, &m4a(256), CLIP_MS, &now())
        .unwrap();
    let key: String = raw(dir.path())
        .query_row("SELECT blob_key FROM attachment", [], |row| row.get(0))
        .unwrap();
    assert_eq!(key.len(), 64);
    let shown = format!("{sent:?}");
    assert!(!shown.contains(&key), "blob_key leaked into the view");
}
