//! The local waveform of a voice note (slice 7a, ADR-0015): saved with the recording, saved later by the player's decoder, never
//! part of a synced row, the migration from a slice 7 database, and survival of a kill. The waveform is cosmetic: garbage never
//! costs a note.

#![cfg(test)]

mod common;

use common::attach::*;
use common::voice::*;
use common::*;
use kinige_domain::attachment::WAVEFORM_PEAKS;
use kinige_domain::groups::INBOX_ID;
use kinige_store::{LATEST_VERSION, MAX_DECODE_BYTES, Store, StoreError};
use std::path::Path;

fn peaks(seed: u8) -> Vec<u8> {
    (0..WAVEFORM_PEAKS)
        .map(|i| seed.wrapping_add(u8::try_from(i * 3).unwrap_or(0)))
        .collect()
}

fn note(store: &Store, waveform: Option<&[u8]>) -> kinige_domain::AttachmentView {
    store
        .send_voice_with_waveform(INBOX_ID, None, &m4a(512), CLIP_MS, waveform, &now())
        .unwrap()
        .attachments
        .remove(0)
}

fn shown(store: &Store) -> Vec<kinige_domain::AttachmentView> {
    store
        .list_messages(INBOX_ID, None, 50, &now())
        .unwrap()
        .messages
        .into_iter()
        .flat_map(|message| message.attachments)
        .collect()
}

/// Every column of the synced `attachment` rows, as text, in a stable order.
fn synced_rows(dir: &Path) -> Vec<Vec<Option<String>>> {
    let conn = raw(dir);
    let mut statement = conn
        .prepare("SELECT * FROM attachment ORDER BY id")
        .unwrap();
    let columns = statement.column_count();
    statement
        .query_map([], |row| {
            (0..columns)
                .map(|i| {
                    row.get::<_, rusqlite::types::Value>(i).map(|v| match v {
                        rusqlite::types::Value::Null => None,
                        rusqlite::types::Value::Integer(n) => Some(n.to_string()),
                        rusqlite::types::Value::Text(text) => Some(text),
                        other => Some(format!("{other:?}")),
                    })
                })
                .collect()
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

#[test]
fn the_peaks_of_a_recording_are_saved_with_the_note_and_read_back() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let sent = note(&store, Some(&peaks(9)));
    assert_eq!(sent.waveform, Some(peaks(9)));
    drop(store);
    let reopened = open_without_mirror(dir.path());
    assert_eq!(shown(&reopened)[0].waveform, Some(peaks(9)));
}

#[test]
fn a_note_without_peaks_has_none_until_they_are_saved() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let sent = note(&store, None);
    assert_eq!(sent.waveform, None);
    assert_eq!(shown(&store)[0].waveform, None);
    store.set_attachment_waveform(&sent.id, &peaks(1)).unwrap();
    assert_eq!(shown(&store)[0].waveform, Some(peaks(1)));
}

#[test]
fn peaks_of_the_wrong_length_never_cost_the_recording() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    for bad in [
        vec![],
        vec![1],
        vec![7; WAVEFORM_PEAKS - 1],
        vec![7; WAVEFORM_PEAKS + 1],
        vec![7; 1 << 20],
    ] {
        let sent = note(&store, Some(&bad));
        assert_eq!(sent.waveform, None, "{} bytes", bad.len());
    }
    assert_eq!(count(dir.path(), "SELECT COUNT(*) FROM attachment"), 5);
    assert_eq!(
        count(
            dir.path(),
            "SELECT COUNT(*) FROM attachment_local WHERE waveform IS NOT NULL"
        ),
        0
    );
}

#[test]
fn the_decoder_result_is_saved_once_and_the_first_value_wins() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let id = note(&store, None).id;
    store.set_attachment_waveform(&id, &peaks(5)).unwrap();
    store.set_attachment_waveform(&id, &peaks(6)).unwrap();
    store.set_attachment_waveform(&id, &[]).unwrap();
    assert_eq!(shown(&store)[0].waveform, Some(peaks(5)));
}

#[test]
fn the_failed_marker_is_an_empty_list_and_stops_the_decoder_for_good() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let id = note(&store, None).id;
    store.set_attachment_waveform(&id, &[]).unwrap();
    drop(store);
    let reopened = open_without_mirror(dir.path());
    assert_eq!(shown(&reopened)[0].waveform, Some(Vec::new()));
}

#[test]
fn garbage_peaks_and_ids_are_errors_and_write_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let id = note(&store, None).id;
    for bad in [
        vec![0],
        vec![0; WAVEFORM_PEAKS - 1],
        vec![0; WAVEFORM_PEAKS + 1],
        vec![255; 70_000],
    ] {
        let error = store.set_attachment_waveform(&id, &bad).unwrap_err();
        assert!(
            matches!(error, StoreError::AttachmentUnreadable),
            "{} bytes: {error}",
            bad.len()
        );
    }
    for bad_id in [
        "",
        "x",
        "../etc/passwd",
        "9d3e4f50-6a7b-4c8d-9e0f-1a2b3c4d5e6f",
        "ÿ",
        &"a".repeat(10_000),
    ] {
        let error = store
            .set_attachment_waveform(bad_id, &peaks(1))
            .unwrap_err();
        assert!(
            matches!(error, StoreError::AttachmentNotFound),
            "{bad_id:?}: {error}"
        );
    }
    assert_eq!(shown(&store)[0].waveform, None);
}

#[test]
fn only_an_audio_attachment_has_a_waveform() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let picture = write(dir.path(), "photo.png", &png(8, 8));
    let staged = store.attach_path(INBOX_ID, &picture, &now()).unwrap();
    let sent = store
        .send_entry(INBOX_ID, "", None, std::slice::from_ref(&staged.id), &now())
        .unwrap();
    let image_id = sent.attachments[0].id.clone();
    assert!(matches!(
        store.set_attachment_waveform(&image_id, &peaks(1)),
        Err(StoreError::AttachmentNotFound)
    ));
    assert!(matches!(
        store.audio_for_waveform(&image_id),
        Err(StoreError::AttachmentNotFound)
    ));
}

#[test]
fn a_purged_note_has_no_waveform_to_save() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let sent = store
        .send_voice(INBOX_ID, None, &m4a(512), CLIP_MS, &now())
        .unwrap();
    let id = sent.attachments[0].id.clone();
    store.delete_message(&sent.id, &now()).unwrap();
    store.purge_message(&sent.id, &now()).unwrap();
    assert!(matches!(
        store.set_attachment_waveform(&id, &peaks(1)),
        Err(StoreError::AttachmentNotFound)
    ));
}

#[test]
fn setting_a_waveform_changes_no_synced_byte() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let id = note(&store, None).id;
    let before = synced_rows(dir.path());
    let dirty_before = count(dir.path(), "SELECT COUNT(*) FROM mirror_dirty");
    store.set_attachment_waveform(&id, &peaks(3)).unwrap();
    assert_eq!(
        synced_rows(dir.path()),
        before,
        "attachment rows are byte-identical"
    );
    assert_eq!(
        count(dir.path(), "SELECT COUNT(*) FROM mirror_dirty"),
        dirty_before
    );
    // The same for a note that had its peaks from the recorder: the synced row has no waveform column at all.
    let with = note(&store, Some(&peaks(4)));
    let columns: Vec<String> = raw(dir.path())
        .prepare("SELECT name FROM pragma_table_info('attachment')")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(
        columns
            .iter()
            .all(|name| !name.contains("wave") && !name.contains("peak")),
        "{columns:?}"
    );
    assert!(with.waveform.is_some());
}

#[test]
fn the_decoder_gets_the_stored_bytes_and_never_more_than_sixteen_mebibytes() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let bytes = m4a(4096);
    let sent = store
        .send_voice(INBOX_ID, None, &bytes, CLIP_MS, &now())
        .unwrap();
    assert_eq!(
        store.audio_for_waveform(&sent.attachments[0].id).unwrap(),
        bytes
    );

    let big = m4a(usize::try_from(MAX_DECODE_BYTES).unwrap() + 1);
    let sent = store
        .send_voice(INBOX_ID, None, &big, CLIP_MS, &now())
        .unwrap();
    assert!(matches!(
        store.audio_for_waveform(&sent.attachments[0].id),
        Err(StoreError::AttachmentTooLarge)
    ));
    for bad in ["", "nope", "9d3e4f50-6a7b-4c8d-9e0f-1a2b3c4d5e6f"] {
        assert!(
            matches!(
                store.audio_for_waveform(bad),
                Err(StoreError::AttachmentNotFound)
            ),
            "{bad:?}"
        );
    }
}

#[test]
fn a_missing_file_has_no_bytes_to_decode() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let sent = store
        .send_voice(INBOX_ID, None, &m4a(512), CLIP_MS, &now())
        .unwrap();
    let id = sent.attachments[0].id.clone();
    std::fs::remove_file(dir.path().join(format!("attachments/{id}.m4a"))).unwrap();
    store.sweep_attachments(&now()).unwrap();
    assert!(matches!(
        store.audio_for_waveform(&id),
        Err(StoreError::AttachmentNotFound)
    ));
}

#[test]
fn a_slice_7_database_is_upgraded_in_place_and_keeps_every_row() {
    let dir = tempfile::tempdir().unwrap();
    let (id, message) = {
        let store = open_without_mirror(dir.path());
        let sent = store
            .send_voice(INBOX_ID, None, &m4a(1024), CLIP_MS, &now())
            .unwrap();
        (sent.attachments[0].id.clone(), sent.id)
    };
    let before = synced_rows(dir.path());
    // The slice 7 database: schema 7 as migrations 1-7 make it (no `waveform` column), with the rows it had.
    common::old_db::downgrade_to(dir.path(), 7);
    let store = open_without_mirror(dir.path());
    assert_eq!(user_version(dir.path()), LATEST_VERSION);
    assert_eq!(LATEST_VERSION, 8);
    assert_eq!(synced_rows(dir.path()), before, "no synced row changed");
    assert_eq!(count(dir.path(), "SELECT COUNT(*) FROM message"), 1);
    let found = shown(&store);
    assert_eq!(
        (found[0].id.as_str(), found[0].waveform.clone()),
        (id.as_str(), None)
    );
    assert_eq!(
        store.audio_for_waveform(&id).unwrap(),
        m4a(1024),
        "the old note still plays and decodes"
    );
    store.set_attachment_waveform(&id, &peaks(2)).unwrap();
    assert_eq!(shown(&store)[0].waveform, Some(peaks(2)));
    assert_eq!(
        count(
            dir.path(),
            &format!("SELECT COUNT(*) FROM message WHERE id = '{message}'")
        ),
        1
    );
}

#[test]
fn the_migration_is_idempotent_and_adds_the_column_once() {
    let dir = tempfile::tempdir().unwrap();
    for _ in 0..3 {
        let store = open_without_mirror(dir.path());
        drop(store);
        assert_eq!(user_version(dir.path()), LATEST_VERSION);
        let columns = count(
            dir.path(),
            "SELECT COUNT(*) FROM pragma_table_info('attachment_local') WHERE name = 'waveform'",
        );
        assert_eq!(columns, 1);
    }
    // A database that already has the column (a half-finished manual edit) is refused loudly, not corrupted: the version stays 7.
    {
        let conn = raw(dir.path());
        conn.pragma_update(None, "user_version", 7).unwrap();
    }
    assert!(Store::open(options(dir.path())).is_err());
    assert_eq!(
        user_version(dir.path()),
        7,
        "a failed migration leaves the version where it was"
    );
}

fn user_version(dir: &Path) -> i64 {
    raw(dir)
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap()
}

const CHILD_ENV: &str = "KINIGE_WAVEFORM_ABORT_CHILD_DIR";
const CHILD_TEST: &str = "a_saved_waveform_survives_a_kill_and_a_restart";

/// The child sends a note with its peaks, then saves the peaks of a second one through the decoder's path, and dies by `abort`
/// right after the replies (after `COMMIT`); the parent reopens the data.
#[test]
fn a_saved_waveform_survives_a_kill_and_a_restart() {
    if let Ok(dir) = std::env::var(CHILD_ENV) {
        let store = open_without_mirror(dir.as_ref());
        note(&store, Some(&peaks(11)));
        let later = note(&store, None);
        store
            .set_attachment_waveform(&later.id, &peaks(22))
            .expect("save");
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
    let mut found: Vec<_> = shown(&reopened).into_iter().map(|a| a.waveform).collect();
    found.sort();
    assert_eq!(found, [Some(peaks(11)), Some(peaks(22))]);
}
