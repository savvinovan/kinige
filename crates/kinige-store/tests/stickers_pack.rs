//! Slice 5c, the library and a pack write (ADR-0021): a pack is written all or nothing, a file that is not a sticker is counted and left out,
//! a replaced pack keeps its place, a crash between the renames and the rows leaves nothing visible and the sweep clears the files, and a
//! removed pack takes its files and nothing else.

#![cfg(test)]

mod common;

use common::attach::*;
use common::stickers::*;
use common::*;
use kinige_domain::attachment::AttachmentKind;
use kinige_domain::groups::INBOX_ID;
use kinige_domain::stickers::PackSource;
use kinige_store::{AttachFault, NewPack, PackItem, StoreError};
use std::sync::Arc;

#[test]
fn a_pack_is_written_whole_and_the_library_lists_it() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let added = add_pack(&store, Some("TestPack"), "Test Pack", &drawn(3)).unwrap();
    assert_eq!(
        (added.title.as_str(), added.added, added.skipped),
        ("Test Pack", 3, 0)
    );
    let library = store.sticker_library(false).unwrap();
    assert!(!library.has_token);
    assert_eq!(library.packs.len(), 1);
    let pack = &library.packs[0];
    assert_eq!(
        (pack.source, pack.title.as_str()),
        (PackSource::TelegramLink, "Test Pack")
    );
    assert_eq!(pack.stickers.len(), 3);
    assert_eq!(pack.stickers[0].emoji.as_deref(), Some("😺"));
    assert_eq!(
        (pack.stickers[1].width, pack.stickers[1].height),
        (201, 160)
    );
    assert_eq!(library_files(dir.path()).len(), 3);
    assert!(
        stage_dirs(dir.path()).is_empty(),
        "the staging folder is gone"
    );
    assert!(store.sticker_library(true).unwrap().has_token);
    // The protocol's lookup: a UUID of a sticker whose file is there, and nothing else.
    let (path, mime) = store.sticker_file(&pack.stickers[0].id).unwrap().unwrap();
    assert!(path.is_file());
    assert_eq!(mime, "image/webp");
    for id in ["", "x", "../x", "00000000-0000-4000-8000-000000000000"] {
        assert_eq!(store.sticker_file(id).unwrap(), None, "{id:?}");
    }
}

#[test]
fn what_is_not_a_sticker_is_counted_and_left_out() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let mut big = webp(100, 100);
    big.resize(2 * 1024 * 1024 + 1, 0);
    let files = vec![
        (webp(100, 100), None),
        (webp(100, 100), None), // the same content twice
        (webp(512, 400), None),
        (webp(513, 100), None),
        (webp(100, 513), None),
        (animated_webp(64, 64), None),
        (apng(64, 64), None),
        (png(64, 64), None),
        (jpeg(64, 64), None),
        (gif(64, 64), None),
        (big, None),
        (b"not an image".to_vec(), None),
        (Vec::new(), None),
    ];
    let added = add_pack(&store, None, "Mixed", &files).unwrap();
    assert_eq!((added.added, added.skipped), (3, 10));
    let kinds: Vec<(u32, u32)> = store.sticker_library(false).unwrap().packs[0]
        .stickers
        .iter()
        .map(|s| (s.width, s.height))
        .collect();
    assert_eq!(kinds, vec![(100, 100), (512, 400), (64, 64)]);
}

#[test]
fn nothing_is_added_when_no_file_passes() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let files = vec![
        (jpeg(64, 64), None),
        (animated_webp(64, 64), None),
        (Vec::new(), None),
    ];
    let error = add_pack(&store, Some("Nope"), "Nope", &files).unwrap_err();
    assert!(matches!(error, StoreError::StickerUnreadable), "{error}");
    assert_eq!(count(dir.path(), "SELECT COUNT(*) FROM sticker_pack"), 0);
    assert_eq!(count(dir.path(), "SELECT COUNT(*) FROM sticker"), 0);
    assert!(library_files(dir.path()).is_empty());
    assert!(stage_dirs(dir.path()).is_empty());
    // More than 200 items is refused before a file is read.
    let too_many: Vec<_> = (0..201).map(|_| (webp(10, 10), None)).collect();
    assert!(add_pack(&store, None, "Too many", &too_many).is_err());
    assert_eq!(count(dir.path(), "SELECT COUNT(*) FROM sticker_pack"), 0);
}

#[test]
fn a_hostile_title_and_emoji_are_cleaned() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let files = vec![(webp(40, 40), Some("<script>alert(1)</script>".to_string()))];
    let added = add_pack(&store, None, "../../etc/\u{202E}passwd", &files).unwrap();
    assert_eq!(added.title, "passwd");
    let library = store.sticker_library(false).unwrap();
    assert_eq!(library.packs[0].stickers[0].emoji, None);
    assert_eq!(
        count(
            dir.path(),
            "SELECT COUNT(*) FROM sticker WHERE emoji IS NOT NULL"
        ),
        0
    );
    assert_eq!(
        add_pack(&store, None, "\u{0}", &drawn(1)).unwrap().title,
        "Sticker pack"
    );
}

#[test]
fn a_replaced_pack_keeps_its_place_and_loses_its_old_files() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    add_pack(&store, Some("TestPack"), "First", &drawn(2)).unwrap();
    add_pack(&store, Some("Other"), "Second", &drawn(1)).unwrap();
    let old_ids = ids_of(&store.sticker_library(false).unwrap(), "First");
    let added = add_pack(&store, Some("TestPack"), "First again", &drawn(3)).unwrap();
    assert_eq!(added.added, 3);
    let library = store.sticker_library(false).unwrap();
    let titles: Vec<&str> = library.packs.iter().map(|p| p.title.as_str()).collect();
    assert_eq!(
        titles,
        vec!["First again", "Second"],
        "same place in the strip"
    );
    assert_eq!(library_files(dir.path()).len(), 4);
    for id in old_ids {
        assert_eq!(store.sticker_file(&id).unwrap(), None);
    }
}

#[test]
fn a_crash_between_the_renames_and_the_rows_leaves_no_pack_and_the_sweep_clears_the_files() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    store.inject_attach_fault_for_test(AttachFault::PackAfterRename(Arc::new(|| {
        std::panic::resume_unwind(Box::new("simulated crash"));
    })));
    let error = add_pack(&store, Some("TestPack"), "Crash", &drawn(2)).unwrap_err();
    assert!(matches!(error, StoreError::Internal(_)), "{error}");
    store.inject_attach_fault_for_test(AttachFault::Clear);
    assert_eq!(count(dir.path(), "SELECT COUNT(*) FROM sticker_pack"), 0);
    assert!(store.sticker_library(false).unwrap().packs.is_empty());
    assert_eq!(
        library_files(dir.path()).len(),
        2,
        "renamed files that nobody owns"
    );
    // The dead process's staging folder.
    let dead = dir
        .path()
        .join("stickers/.tmp/00000000-0000-4000-8000-00000000dead/job");
    std::fs::create_dir_all(&dead).unwrap();
    std::fs::write(dead.join("0.part"), b"half").unwrap();
    // A sent sticker's own file in `attachments/` is not the library's: the sweep never touches it.
    let report = store.sweep_stickers().unwrap();
    assert_eq!((report.files_removed, report.stale_dirs), (2, 1));
    assert!(library_files(dir.path()).is_empty());
    assert!(
        !dir.path()
            .join("stickers/.tmp/00000000-0000-4000-8000-00000000dead")
            .exists()
    );
    // The sweep leaves a pack that has its rows alone.
    add_pack(&store, None, "Whole", &drawn(1)).unwrap();
    assert_eq!(store.sweep_stickers().unwrap().files_removed, 0);
    assert_eq!(library_files(dir.path()).len(), 1);
}

#[test]
fn a_fault_before_the_commit_of_a_replacement_leaves_the_old_pack_whole() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    add_pack(&store, Some("TestPack"), "Old", &drawn(2)).unwrap();
    store.inject_attach_fault_for_test(AttachFault::PackBeforeCommit(Arc::new(|| {
        std::panic::resume_unwind(Box::new("simulated crash"));
    })));
    assert!(add_pack(&store, Some("TestPack"), "New", &drawn(3)).is_err());
    store.inject_attach_fault_for_test(AttachFault::Clear);
    let library = store.sticker_library(false).unwrap();
    assert_eq!(library.packs.len(), 1);
    assert_eq!(
        (
            library.packs[0].title.as_str(),
            library.packs[0].stickers.len()
        ),
        ("Old", 2)
    );
    for sticker in &library.packs[0].stickers {
        assert!(
            store.sticker_file(&sticker.id).unwrap().is_some(),
            "the old files are there"
        );
    }
    // The new files (3) had been renamed: they are the sweep's.
    assert_eq!(store.sweep_stickers().unwrap().files_removed, 3);
    assert_eq!(library_files(dir.path()).len(), 2);
}

#[test]
fn a_row_without_its_file_is_hidden_and_a_pack_without_files_is_not_listed() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    add_pack(&store, None, "Lonely", &drawn(1)).unwrap();
    for name in library_files(dir.path()) {
        std::fs::remove_file(dir.path().join("stickers").join(name)).unwrap();
    }
    assert!(store.sticker_library(false).unwrap().packs.is_empty());
    assert_eq!(
        count(dir.path(), "SELECT COUNT(*) FROM sticker"),
        1,
        "the row stays"
    );
}

#[test]
fn a_removed_pack_goes_with_its_files_and_the_sent_stickers_stay() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    add_pack(&store, Some("TestPack"), "Mine", &drawn(2)).unwrap();
    let library = store.sticker_library(false).unwrap();
    let pack = library.packs[0].clone();
    let sent = store
        .send_sticker(INBOX_ID, &pack.stickers[0].id, None, &now())
        .unwrap();
    store.remove_sticker_pack(&pack.id).unwrap();
    assert!(store.sticker_library(false).unwrap().packs.is_empty());
    assert!(library_files(dir.path()).is_empty());
    assert_eq!(count(dir.path(), "SELECT COUNT(*) FROM sticker_pack"), 0);
    assert_eq!(count(dir.path(), "SELECT COUNT(*) FROM sticker"), 0);
    // The entry and its own file are intact; its picture is the attachment's.
    let page = store.sticker_library(false).unwrap();
    assert!(
        page.recent.is_empty(),
        "recent stickers of a removed pack drop out"
    );
    let attachment = &sent.attachments[0];
    assert_eq!(attachment.kind, AttachmentKind::Image);
    assert!(attachment.sticker);
    assert_eq!(
        names(&dir.path().join("attachments"))
            .iter()
            .filter(|n| n.ends_with(".webp"))
            .count(),
        1
    );
    assert!(matches!(
        store.remove_sticker_pack(&pack.id).unwrap_err(),
        StoreError::StickerNotFound
    ));
    for garbage in ["", "x", "../x"] {
        assert!(store.remove_sticker_pack(garbage).is_err());
    }
}

#[test]
fn the_library_files_of_a_purged_sticker_entry_are_its_own() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    add_pack(&store, None, "Mine", &drawn(1)).unwrap();
    let id = store.sticker_library(false).unwrap().packs[0].stickers[0]
        .id
        .clone();
    let sent = store.send_sticker(INBOX_ID, &id, None, &now()).unwrap();
    store.delete_message(&sent.id, &now()).unwrap();
    store.purge_message(&sent.id, &now()).unwrap();
    assert!(
        names(&dir.path().join("attachments"))
            .iter()
            .all(|n| !n.ends_with(".webp")),
        "its own file went"
    );
    assert_eq!(library_files(dir.path()).len(), 1, "the library's stays");
    assert!(store.sticker_file(&id).unwrap().is_some());
}

#[test]
fn a_new_pack_needs_no_link_name_and_two_packs_may_share_a_title() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    add_pack(&store, None, "Same", &drawn(1)).unwrap();
    add_pack(&store, None, "Same", &drawn(2)).unwrap();
    assert_eq!(store.sticker_library(false).unwrap().packs.len(), 2);
    let stage = store.pack_stage().unwrap();
    let empty = NewPack {
        source: PackSource::TelegramLink,
        link_name: None,
        title: "x".into(),
        items: vec![PackItem {
            index: 0,
            emoji: None,
        }],
    };
    // An item whose file was never written is a skipped file.
    assert!(matches!(
        store.add_sticker_pack(stage, empty),
        Err(StoreError::StickerUnreadable)
    ));
}

fn now() -> kinige_store::Now {
    moscow(T0)
}
