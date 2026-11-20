//! Slice 5c, the stickers of a Telegram export: a static sticker (`image/webp`) that passes the probe becomes an attachment that reads as
//! a sticker and an item of the pack "From Telegram" (one per content); TGS, WebM, animated and failed files stay placeholders and are counted;
//! an entry imported earlier gets its sticker once, and an edited one keeps its text.

#![cfg(test)]

mod common;

use common::attach::*;
use common::import::*;
use common::stickers::*;
use common::*;
use kinige_domain::Language;
use kinige_domain::attachment::sticker_of;
use kinige_domain::groups::INBOX_ID;
use kinige_domain::import::{ImportProgress, ImportReport};
use kinige_store::Store;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

const FROM: &str = "user1000001";

/// A sticker message of the export: `mime` and `file` decide whether it is static.
fn sticker(id: u64, path: &str, mime: &str, emoji: &str) -> String {
    format!(
        r#"{{"id":{id},"type":"message","date":"x","date_unixtime":"{unix}","from":"Я","from_id":"{FROM}","file":{p},"file_name":"sticker ({id}).webp","media_type":"sticker","sticker_emoji":"{emoji}","mime_type":"{mime}","file_size":100,"width":512,"height":512,"text":"","text_entities":[]}}"#,
        unix = SAVED_T + id as i64,
        p = serde_json::to_string(path).unwrap(),
    )
}

fn export_of(messages: &[String]) -> Arc<kinige_domain::telegram::TgExport> {
    parse(&single("saved_messages", OWNER, None, messages))
}

fn import_with(
    store: &Store,
    export: &Arc<kinige_domain::telegram::TgExport>,
    folder: Option<&Path>,
) -> ImportReport {
    store
        .import_telegram_with_media(
            export.clone(),
            all_chats(export),
            Language::Ru,
            &now(),
            |_: ImportProgress| {},
            Arc::new(AtomicBool::new(false)),
            folder,
        )
        .unwrap()
}

fn feed(store: &Store) -> Vec<kinige_domain::MessageView> {
    store
        .list_messages(INBOX_ID, None, 500, &now())
        .unwrap()
        .messages
}

fn folder(files: &[(&str, Vec<u8>)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (path, bytes) in files {
        let full = dir.path().join(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, bytes).unwrap();
    }
    dir
}

#[test]
fn static_stickers_become_sticker_entries_and_one_pack_item_per_content() {
    let data = tempfile::tempdir().unwrap();
    let store = open_without_mirror(data.path());
    let export = folder(&[
        ("stickers/a.webp", webp(300, 300)),
        ("stickers/b.webp", webp(301, 300)),
        ("stickers/a-again.webp", webp(300, 300)),
        ("stickers/anim.webp", animated_webp(64, 64)),
        ("stickers/big.webp", webp(600, 600)),
        ("stickers/s.tgs", b"\x1f\x8b gzip".to_vec()),
        ("stickers/s.webm", b"\x1aE\xdf\xa3".to_vec()),
    ]);
    let parsed = export_of(&[
        sticker(1, "stickers/a.webp", "image/webp", "😺"),
        sticker(2, "stickers/b.webp", "image/webp", "🔥"),
        sticker(3, "stickers/a-again.webp", "image/webp", "😺"),
        sticker(4, "stickers/anim.webp", "image/webp", "🌲"),
        sticker(5, "stickers/big.webp", "image/webp", "🐟"),
        sticker(6, "stickers/s.tgs", "application/x-tgsticker", "🐟"),
        sticker(7, "stickers/s.webm", "video/webm", "🐟"),
        sticker(
            8,
            "(File not included. Change data exporting settings to download.)",
            "image/webp",
            "🐟",
        ),
    ]);
    let report = import_with(&store, &parsed, Some(export.path()));
    assert_eq!(report.imported, 8);
    assert_eq!(report.media_attached, 3, "{report:?}");
    assert_eq!(
        report.media_skipped, 2,
        "the animated and the oversized WebP fail the probe"
    );
    assert_eq!(report.media_missing, 1);
    assert_eq!(
        report.media_placeholders, 5,
        "the three skipped or missing and TGS and WebM"
    );

    let messages = feed(&store);
    let stickers: Vec<_> = messages
        .iter()
        .filter(|m| sticker_of(&m.body_md, &m.attachments).is_some())
        .collect();
    assert_eq!(stickers.len(), 3);
    for m in &stickers {
        assert_eq!(m.body_md, "", "no placeholder line next to the sticker");
        let a = &m.attachments[0];
        assert!(a.sticker && a.filename == "sticker.webp" && a.mime == "image/webp");
    }
    let emoji: Vec<_> = stickers
        .iter()
        .map(|m| m.attachments[0].sticker_emoji.clone())
        .collect();
    assert_eq!(
        emoji,
        vec![
            Some("😺".to_string()),
            Some("🔥".to_string()),
            Some("😺".to_string())
        ]
    );
    // The others keep their placeholder line and have no attachment.
    let placeholders = messages
        .iter()
        .filter(|m| m.body_md.contains("не импортирован"))
        .count();
    assert_eq!(placeholders, 5);
    assert_eq!(
        messages
            .iter()
            .filter(|m| !m.attachments.is_empty())
            .count(),
        3
    );

    // Every sticker message has its own attachment; the library has one item per content, in the one pack "From Telegram".
    let library = store.sticker_library(false).unwrap();
    assert_eq!(library.packs.len(), 1);
    assert_eq!(library.packs[0].title, "From Telegram");
    assert_eq!(library.packs[0].stickers.len(), 2);
    assert_eq!(
        count(
            data.path(),
            "SELECT COUNT(*) FROM attachment WHERE sticker = 1"
        ),
        3
    );
    assert_eq!(library_files(data.path()).len(), 2);
    assert!(
        store
            .sticker_file(&library.packs[0].stickers[0].id)
            .unwrap()
            .is_some()
    );
}

#[test]
fn the_preview_counts_the_stickers_sent_the_different_ones_and_the_ones_that_stay_placeholders() {
    let data = tempfile::tempdir().unwrap();
    let store = open_without_mirror(data.path());
    let export = folder(&[
        ("stickers/a.webp", webp(300, 300)),
        ("stickers/b.webp", webp(301, 300)),
        ("stickers/a-again.webp", webp(300, 300)),
        ("stickers/s.tgs", b"\x1f\x8b gzip".to_vec()),
    ]);
    let parsed = export_of(&[
        sticker(1, "stickers/a.webp", "image/webp", "😺"),
        sticker(2, "stickers/b.webp", "image/webp", "🔥"),
        sticker(3, "stickers/a-again.webp", "image/webp", "😺"),
        sticker(4, "stickers/s.tgs", "application/x-tgsticker", "🐟"),
        sticker(
            5,
            "(File not included. Change data exporting settings to download.)",
            "image/webp",
            "🐟",
        ),
        sticker(6, "../../etc/passwd", "image/webp", "🐟"),
        msg(7, SAVED_T + 7, "просто текст", &format!("user{OWNER}")),
    ]);
    let preview = store
        .import_preview_with_media(&parsed, Some(export.path()))
        .unwrap();
    assert_eq!(
        (
            preview.stickers.sent,
            preview.stickers.different,
            preview.stickers.skipped
        ),
        (3, 2, 3)
    );
    // Without the folder the files cannot be compared: different paths count.
    let blind = store.import_preview(&parsed).unwrap();
    assert_eq!(
        blind.stickers.sent, 4,
        "the path of the hostile one is the importer's to refuse"
    );
    assert_eq!(blind.stickers.different, 4);
    assert_eq!(blind.stickers.skipped, 2);
}

#[test]
fn a_repeated_import_adds_nothing_and_old_entries_get_their_sticker_once() {
    let data = tempfile::tempdir().unwrap();
    let store = open_without_mirror(data.path());
    let export = folder(&[
        ("stickers/a.webp", webp(300, 300)),
        ("stickers/b.webp", webp(301, 300)),
    ]);
    let parsed = export_of(&[
        sticker(1, "stickers/a.webp", "image/webp", "😺"),
        sticker(2, "stickers/b.webp", "image/webp", "🔥"),
    ]);
    // The first import ran without media (before 5c): both entries are placeholders.
    import_with(&store, &parsed, None);
    assert!(
        feed(&store)
            .iter()
            .all(|m| m.attachments.is_empty() && m.body_md.contains("не импортирован"))
    );
    let edited = feed(&store)[1].id.clone();
    store
        .edit_message(&edited, "_[Стикер 🔥 не импортирован]_\nмоя правка", &now())
        .unwrap();

    let report = import_with(&store, &parsed, Some(export.path()));
    assert_eq!(report.imported, 0);
    assert_eq!(report.media_attached, 2);
    let messages = feed(&store);
    // The unedited one is a sticker now, with the placeholder line gone; the edited one keeps its text and shows the sticker as a picture.
    assert_eq!(messages[0].body_md, "");
    assert!(sticker_of(&messages[0].body_md, &messages[0].attachments).is_some());
    assert_eq!(
        messages[1].body_md,
        "_[Стикер 🔥 не импортирован]_\nмоя правка"
    );
    assert!(messages[1].attachments[0].sticker);
    assert!(sticker_of(&messages[1].body_md, &messages[1].attachments).is_none());
    assert_eq!(
        store.sticker_library(false).unwrap().packs[0]
            .stickers
            .len(),
        2
    );

    // A third run changes nothing at all.
    let again = import_with(&store, &parsed, Some(export.path()));
    assert_eq!((again.imported, again.media_attached), (0, 0));
    assert_eq!(count(data.path(), "SELECT COUNT(*) FROM attachment"), 2);
    assert_eq!(library_files(data.path()).len(), 2);
    assert_eq!(count(data.path(), "SELECT COUNT(*) FROM sticker"), 2);
}

#[test]
fn a_hostile_emoji_and_path_in_the_export_reach_nothing() {
    let data = tempfile::tempdir().unwrap();
    let store = open_without_mirror(data.path());
    let export = folder(&[("stickers/a.webp", webp(300, 300))]);
    let hostile = r#"{"id":1,"type":"message","date":"x","date_unixtime":"1552035601","from":"Я","from_id":"user1000001","file":"stickers/a.webp","media_type":"sticker","sticker_emoji":"<img src=x onerror=alert(1)>","mime_type":"image/webp","text":"","text_entities":[]}"#;
    let outside = r#"{"id":2,"type":"message","date":"x","date_unixtime":"1552035602","from":"Я","from_id":"user1000001","file":"../../etc/passwd","media_type":"sticker","sticker_emoji":"😺","mime_type":"image/webp","text":"","text_entities":[]}"#;
    let parsed = export_of(&[hostile.to_string(), outside.to_string()]);
    let report = import_with(&store, &parsed, Some(export.path()));
    assert_eq!((report.media_attached, report.media_skipped), (1, 1));
    let messages = feed(&store);
    let a = &messages[0].attachments[0];
    assert!(a.sticker);
    assert_eq!(
        a.sticker_emoji, None,
        "a text that is not an emoji reaches no column and no view"
    );
    assert_eq!(
        count(
            data.path(),
            "SELECT COUNT(*) FROM attachment WHERE sticker_emoji IS NOT NULL"
        ),
        0
    );
    assert_eq!(
        count(
            data.path(),
            "SELECT COUNT(*) FROM sticker WHERE emoji IS NOT NULL"
        ),
        0
    );
    let group: String = raw(data.path())
        .query_row(
            "SELECT json_extract(groups_raw, '$.sticker') FROM attachment",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(group, r#"{"emoji":null,"sticker":true}"#);
}
