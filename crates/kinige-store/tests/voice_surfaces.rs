//! Voice notes and the rest of the app (slice 7a, ADR-0015): search by the word "voice", the Markdown line with the length, the
//! summaries, the forest day card, survival of a kill and the keys that never reach the view.

#![cfg(test)]

mod common;

use common::voice::*;
use common::*;
use kinige_domain::SearchFilters;
use kinige_domain::groups::INBOX_ID;

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
