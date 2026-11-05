//! Where attachments show up besides the feed (slice 7): the forest (weight and photos of a day), the reply quote, "On this day",
//! and what an AI agent sees (metadata only, search by file name).

#![cfg(test)]

mod common;

use common::agent::*;
use common::attach::*;
use common::days::at;
use common::*;
use kinige_domain::attachment::AttachmentKind;
use kinige_domain::forest::FirstWeekday;
use kinige_domain::groups::INBOX_ID;
use kinige_domain::{AgentMode, Language};
use kinige_store::Now;
use serde_json::json;

fn now() -> Now {
    at(2026, 11, 3, 12, 0)
}

fn send_photos(
    store: &kinige_store::Store,
    dir: &std::path::Path,
    chat: &str,
    names: &[&str],
    body: &str,
    when: &Now,
) -> kinige_domain::MessageView {
    let source = dir.join("src");
    std::fs::create_dir_all(&source).unwrap();
    let ids: Vec<String> = names
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let folder = source.join(format!("{}-{i}", name.len()));
            std::fs::create_dir_all(&folder).unwrap();
            let bytes = if name.ends_with(".jpg") {
                jpeg(24, 18)
            } else {
                b"%PDF fake".to_vec()
            };
            store
                .attach_path(chat, &write(&folder, name, &bytes), when)
                .unwrap()
                .id
        })
        .collect();
    store.send_entry(chat, body, None, &ids, when).unwrap()
}

#[test]
fn a_photo_adds_weight_and_the_day_counts_its_photos() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    // A day with one short note, and a day with a photo sent alone and a file with a caption.
    store
        .send_message(INBOX_ID, "ok", &at(2026, 11, 2, 12, 0))
        .unwrap();
    send_photos(
        &store,
        dir.path(),
        INBOX_ID,
        &["a.jpg", "b.jpg"],
        "",
        &at(2026, 11, 3, 9, 0),
    );
    send_photos(
        &store,
        dir.path(),
        INBOX_ID,
        &["menu.pdf"],
        "обед",
        &at(2026, 11, 3, 13, 0),
    );

    let year = store
        .get_forest_year(2026, FirstWeekday::Monday, &now())
        .unwrap();
    let short_day = year.days.iter().find(|d| d.date == "2026-11-02").unwrap();
    assert_eq!(
        (short_day.notes, short_day.photos, short_day.stage),
        (1, 0, 0),
        "a note under 3 characters does not count"
    );
    let day = year.days.iter().find(|d| d.date == "2026-11-03").unwrap();
    assert_eq!(
        (day.notes, day.photos),
        (2, 2),
        "two notes, two photos (the PDF is not a photo)"
    );
    assert!(
        day.stage >= 1,
        "attachments grow the tree: 0.5 per attachment"
    );
    assert_eq!(day.excerpt.as_deref(), None, "the first entry has no text");
    let summary = day
        .first_summary
        .as_ref()
        .expect("a photo-only first entry is described by what it holds");
    assert_eq!(
        (summary.photos, summary.files, summary.name.as_deref()),
        (2, 0, None)
    );
}

#[test]
fn a_reply_to_an_entry_without_text_quotes_what_it_holds() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let photo = send_photos(
        &store,
        dir.path(),
        INBOX_ID,
        &["a.jpg"],
        "",
        &at(2025, 11, 3, 9, 0),
    );
    let file = send_photos(
        &store,
        dir.path(),
        INBOX_ID,
        &["menu.pdf"],
        "",
        &at(2025, 11, 3, 10, 0),
    );
    let reply = store
        .send_reply(INBOX_ID, "ответ", Some(&photo.id), &now())
        .unwrap();
    let quote = reply.reply_to.unwrap();
    assert_eq!(
        (
            quote.excerpt.as_str(),
            quote.attachments.photos,
            quote.attachments.files
        ),
        ("", 1, 0)
    );
    let draft_quote = {
        store
            .save_chat_draft(INBOX_ID, "", Some(&file.id), &now())
            .unwrap();
        store
            .chat_draft(INBOX_ID, &now())
            .unwrap()
            .reply_to
            .unwrap()
    };
    assert_eq!(draft_quote.attachments.name.as_deref(), Some("menu.pdf"));
    // An entry with text and a photo is quoted by its text, and still says it has a photo.
    let both = send_photos(
        &store,
        dir.path(),
        INBOX_ID,
        &["b.jpg"],
        "подпись",
        &at(2025, 11, 4, 9, 0),
    );
    let quote = store
        .send_reply(INBOX_ID, "ещё", Some(&both.id), &now())
        .unwrap()
        .reply_to
        .unwrap();
    assert_eq!(
        (quote.excerpt.as_str(), quote.attachments.photos),
        ("подпись", 1)
    );
}

#[test]
fn on_this_day_entries_carry_their_attachments() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    send_photos(
        &store,
        dir.path(),
        INBOX_ID,
        &["a.jpg", "menu.pdf"],
        "",
        &at(2025, 11, 3, 9, 0),
    );
    let card = store
        .get_on_this_day(&now())
        .unwrap()
        .expect("an entry of this date a year ago");
    let entry = &card.years[0].entries[0];
    assert_eq!(entry.text, "");
    assert_eq!(entry.message.attachments.len(), 2);
    assert_eq!(entry.message.attachments[0].kind, AttachmentKind::Image);
}

#[test]
fn an_agent_sees_attachment_metadata_and_finds_entries_by_file_name_but_never_a_key_or_a_path() {
    let a = agent(AgentMode::Read, true);
    let sent = send_photos(
        &a.store,
        a.dir.path(),
        &a.work,
        &["IMG_0001.jpg", "меню.pdf"],
        "обед с командой",
        &moscow(T0),
    );
    let (blob_key, blob_id): (String, String) = raw(a.dir.path())
        .query_row(
            "SELECT blob_key, remote_blob_id FROM attachment LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();

    let by_id = call(&a.store, "get_entry", json!({"id": sent.id}), T0).unwrap();
    let attachments = &by_id["entry"]["attachments"];
    assert_eq!(attachments.as_array().map(Vec::len), Some(2), "{by_id}");
    assert_eq!(
        attachments[0],
        json!({"kind": "image", "name": "IMG_0001.jpg", "bytes": sent.attachments[0].byte_size, "width": 24, "height": 18})
    );
    assert_eq!(attachments[1]["kind"], "file");
    assert!(attachments[1].get("width").is_none());

    // Search by file name works for the agent too.
    let found = call(&a.store, "search", json!({"query": "меню"}), T0).unwrap();
    assert_eq!(entry_ids(&found), vec![sent.id.clone()], "{found}");

    for reply in [&by_id, &found] {
        let text = reply.to_string();
        for secret in [
            blob_key.as_str(),
            blob_id.as_str(),
            "attachments/",
            a.dir.path().to_str().unwrap(),
        ] {
            assert!(!text.contains(secret), "the agent saw {secret}");
        }
    }
    // The access log keeps no file names either (only the allowlisted arguments).
    for (_, _, args_json, _, _) in log_rows(a.dir.path()) {
        assert!(!args_json.contains(&blob_key) && !args_json.contains("IMG_0001"));
    }
    let _ = Language::En;
}

#[test]
fn an_agent_sees_a_voice_note_with_its_length_and_never_the_bytes() {
    let a = agent(AgentMode::Read, true);
    let mut bytes = vec![0, 0, 0, 0x20];
    bytes.extend_from_slice(b"ftypM4A ");
    bytes.resize(2048, 0x5a);
    let sent = a
        .store
        .send_voice(
            kinige_domain::groups::INBOX_ID,
            None,
            &bytes,
            40_000,
            &moscow(T0),
        )
        .unwrap();
    let by_id = call(&a.store, "get_entry", json!({"id": sent.id}), T0).unwrap();
    let voice = &by_id["entry"]["attachments"][0];
    assert_eq!(voice["kind"], "audio", "{by_id}");
    assert_eq!(voice["duration_ms"], 40_000);
    assert_eq!(voice["bytes"], 2048);
    assert!(voice.get("width").is_none());
    let text = by_id.to_string();
    assert!(!text.contains("attachments/") && !text.contains(a.dir.path().to_str().unwrap()));
    // The word "voice" finds it for the agent too.
    let found = call(&a.store, "search", json!({"query": "voice"}), T0).unwrap();
    assert_eq!(entry_ids(&found), vec![sent.id.clone()], "{found}");
}
