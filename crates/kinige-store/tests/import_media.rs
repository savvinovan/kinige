//! Telegram media (slice 7): photos, files, videos, voice messages and audio are attached to new entries and, once, to entries
//! imported before; the placeholder line goes only from an entry nobody has edited; hostile paths in the export are refused; the
//! run stops, repeats and survives a killed process without losing or duplicating a file.

#![cfg(test)]

mod common;

use common::attach::*;
use common::import::*;
use common::*;
use kinige_domain::Language;
use kinige_domain::attachment::AttachmentKind;
use kinige_domain::groups::INBOX_ID;
use kinige_domain::import::ImportProgress;
use kinige_store::{AttachFault, Store};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

const CHILD_ENV: &str = "KINIGE_IMPORT_MEDIA_CHILD_DIR";
const FROM: &str = "user1000001";

/// A synthetic export folder: `result.json` is written by the test, the files are laid out by the helpers below.
struct Export {
    dir: tempfile::TempDir,
}

impl Export {
    fn new() -> Export {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("photos")).unwrap();
        std::fs::create_dir_all(dir.path().join("files")).unwrap();
        Export { dir }
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    fn put(&self, relative: &str, bytes: &[u8]) {
        let path = self.path().join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }
}

/// A message with a photo (and a caption).
fn photo(id: u64, path: &str, caption: &str) -> String {
    format!(
        r#"{{"id":{id},"type":"message","date":"x","date_unixtime":"{unix}","from":"Я","from_id":"{FROM}","photo":{p},"photo_file_size":100,"width":40,"height":30,"text":{t},"text_entities":[{entities}]}}"#,
        unix = SAVED_T + id as i64,
        p = serde_json::to_string(path).unwrap(),
        t = serde_json::to_string(caption).unwrap(),
        entities = if caption.is_empty() {
            String::new()
        } else {
            format!(
                r#"{{"type":"plain","text":{}}}"#,
                serde_json::to_string(caption).unwrap()
            )
        },
    )
}

/// A message with a file; `media_type` is `None` for a plain file.
fn file(id: u64, path: &str, name: &str, media_type: Option<&str>, extra: &str) -> String {
    let media_type = media_type
        .map(|m| format!(r#""media_type":"{m}","#))
        .unwrap_or_default();
    format!(
        r#"{{"id":{id},"type":"message","date":"x","date_unixtime":"{unix}","from":"Я","from_id":"{FROM}","file":{p},"file_name":{n},{media_type}"mime_type":"application/octet-stream"{extra},"text":"","text_entities":[]}}"#,
        unix = SAVED_T + id as i64,
        p = serde_json::to_string(path).unwrap(),
        n = serde_json::to_string(name).unwrap(),
    )
}

fn export_json(messages: &[String]) -> String {
    single("saved_messages", OWNER, None, messages)
}

fn import_with(
    store: &Store,
    export: &Arc<kinige_domain::telegram::TgExport>,
    folder: Option<&Path>,
    language: Language,
) -> kinige_domain::import::ImportReport {
    store
        .import_telegram_with_media(
            export.clone(),
            all_chats(export),
            language,
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

fn stored(dir: &Path) -> Vec<String> {
    names(&dir.join("attachments"))
        .into_iter()
        .filter(|n| !n.starts_with('.'))
        .collect()
}

fn count(dir: &Path, sql: &str) -> i64 {
    raw(dir).query_row(sql, [], |r| r.get(0)).unwrap()
}

#[test]
fn files_are_attached_to_new_entries_and_their_placeholder_lines_are_not_written() {
    let data = tempfile::tempdir().unwrap();
    let store = open_without_mirror(data.path());
    let export = Export::new();
    export.put("photos/IMG_0001.jpg", &jpeg(40, 30));
    export.put("files/menu.pdf", b"%PDF-1.4 menu");
    export.put("voice_messages/audio_1.ogg", b"OggS fake");
    export.put("files/track.mp3", b"ID3 fake");
    let messages = vec![
        photo(1, "photos/IMG_0001.jpg", "вечер"),
        file(2, "files/menu.pdf", "menu.pdf", None, ""),
        file(
            3,
            "voice_messages/audio_1.ogg",
            "audio_1.ogg",
            Some("voice_message"),
            r#","duration_seconds":7"#,
        ),
        file(
            4,
            "files/track.mp3",
            "track.mp3",
            Some("audio_file"),
            r#","duration_seconds":214"#,
        ),
        photo(
            5,
            "(File not included. Change data exporting settings to download.)",
            "нет файла",
        ),
        photo(6, "photos/gone.jpg", ""),
        file(7, "files/s.webp", "s.webp", Some("sticker"), ""),
        msg(8, SAVED_T + 8, "просто текст", FROM),
    ];
    std::fs::write(export.path().join("result.json"), export_json(&messages)).unwrap();
    let parsed = parse(&export_json(&messages));

    let progress = Arc::new(Mutex::new(Vec::new()));
    let seen = progress.clone();
    let report = store
        .import_telegram_with_media(
            parsed.clone(),
            all_chats(&parsed),
            Language::Ru,
            &now(),
            move |p: ImportProgress| seen.lock().unwrap().push(p),
            Arc::new(AtomicBool::new(false)),
            Some(export.path()),
        )
        .unwrap();
    assert_eq!(report.imported, 8);
    assert_eq!(
        (
            report.media_attached,
            report.media_missing,
            report.media_skipped
        ),
        (4, 2, 0),
        "{report:?}"
    );
    assert_eq!(
        report.media_placeholders, 3,
        "no file, no file, and the sticker keep their placeholders"
    );
    assert!(report.media_bytes > 0 && !report.disk_full);
    let last = *progress.lock().unwrap().last().unwrap();
    assert!(last.bytes_total > 0 && last.bytes_done > 0);

    let page = feed(&store);
    assert_eq!(
        page[0].body_md, "вечер",
        "the photo's placeholder line is not written"
    );
    assert_eq!(page[0].attachments.len(), 1);
    let photo_view = &page[0].attachments[0];
    assert_eq!(
        (photo_view.kind, photo_view.filename.as_str()),
        (AttachmentKind::Image, "IMG_0001.jpg")
    );
    assert_eq!((photo_view.width, photo_view.height), (Some(40), Some(30)));
    assert_eq!(
        page[1].body_md, "",
        "a file with no caption has an empty body"
    );
    assert_eq!(page[1].attachments[0].filename, "menu.pdf");
    assert_eq!(page[2].attachments[0].kind, AttachmentKind::Audio);
    assert_eq!(page[2].attachments[0].duration_ms, Some(7000));
    assert_eq!(page[3].attachments[0].duration_ms, Some(214_000));
    assert_eq!(page[4].body_md, "_[Фото не импортировано]_\nнет файла");
    assert!(page[4].attachments.is_empty());
    assert!(page[5].body_md.contains("не импортировано") && page[5].attachments.is_empty());
    assert!(
        page[6].body_md.contains("Стикер") && page[6].attachments.is_empty(),
        "stickers stay placeholders"
    );
    assert_eq!(page[7].body_md, "просто текст");
    assert_eq!(stored(data.path()).len(), 4);
    // Everything is faithfully copied and nothing stays in the temporary directory.
    let menu = &page[1].attachments[0];
    let copy = data
        .path()
        .join("attachments")
        .join(format!("{}.pdf", menu.id));
    assert_eq!(std::fs::read(copy).unwrap(), b"%PDF-1.4 menu");
    let tmp: Vec<String> = std::fs::read_dir(data.path().join("attachments/.tmp"))
        .map(|d| d.flatten().flat_map(|e| names(&e.path())).collect())
        .unwrap_or_default();
    assert!(tmp.is_empty(), "{tmp:?}");
}

#[test]
fn a_repeat_attaches_each_file_once_and_gives_old_entries_their_files() {
    let data = tempfile::tempdir().unwrap();
    let store = open_without_mirror(data.path());
    let export = Export::new();
    export.put("photos/a.jpg", &jpeg(30, 30));
    export.put("files/b.pdf", b"%PDF b");
    let messages = vec![
        photo(1, "photos/a.jpg", "утро"),
        file(2, "files/b.pdf", "b.pdf", None, ""),
        photo(3, "photos/a.jpg", ""),
    ];
    let json = export_json(&messages);
    let parsed = parse(&json);

    // The first import knows no folder: placeholders, as in slice 4.
    let first = import_with(&store, &parsed, None, Language::Ru);
    assert_eq!(
        (
            first.imported,
            first.media_attached,
            first.media_placeholders
        ),
        (3, 0, 3)
    );
    assert!(feed(&store)[0].body_md.contains("Фото не импортировано"));

    // The second knows it: the old entries get their files and lose their placeholder lines.
    let second = import_with(&store, &parsed, Some(export.path()), Language::Ru);
    assert_eq!(
        (
            second.imported,
            second.skipped_existing,
            second.media_attached
        ),
        (0, 3, 3),
        "{second:?}"
    );
    let page = feed(&store);
    assert_eq!(page[0].body_md, "утро");
    assert_eq!(page[0].attachments.len(), 1);
    assert_eq!(page[1].body_md, "");
    assert_eq!(
        page[2].body_md, "",
        "a placeholder-only entry gets an empty body"
    );
    assert_eq!(stored(data.path()).len(), 3);

    // The third changes nothing: no entry, no row, no file.
    let before = (
        count(data.path(), "SELECT COUNT(*) FROM attachment"),
        stored(data.path()),
    );
    let third = import_with(&store, &parsed, Some(export.path()), Language::Ru);
    assert_eq!((third.media_attached, third.imported), (0, 0), "{third:?}");
    assert_eq!(
        (
            count(data.path(), "SELECT COUNT(*) FROM attachment"),
            stored(data.path())
        ),
        before
    );
    assert!(names(&data.path().join("attachments/.orphans")).is_empty());
}

#[test]
fn the_placeholder_goes_in_both_languages_without_a_revision_and_with_a_newer_body_hlc() {
    let data = tempfile::tempdir().unwrap();
    let store = open_without_mirror(data.path());
    let export = Export::new();
    export.put("photos/a.jpg", &jpeg(30, 30));
    export.put("photos/b.jpg", &jpeg(31, 31));
    let json = export_json(&[
        photo(1, "photos/a.jpg", "утро"),
        photo(2, "photos/b.jpg", "day"),
    ]);
    let parsed = parse(&json);
    // One entry was imported in Russian, the other in English; the media arrive in a run in whichever language.
    import_with(
        &store,
        &parse(&export_json(&[photo(1, "photos/a.jpg", "утро")])),
        None,
        Language::Ru,
    );
    import_with(
        &store,
        &parse(&export_json(&[photo(2, "photos/b.jpg", "day")])),
        None,
        Language::En,
    );
    let before = feed(&store);
    let ids: Vec<String> = before.iter().map(|m| m.id.clone()).collect();
    assert!(
        before[0].body_md.contains("не импортировано")
            && before[1].body_md.contains("not imported")
    );
    let core_hlc = common::ops::group_hlc(data.path(), "message", &ids[0], "core");
    assert_eq!(
        common::ops::group_hlc(data.path(), "message", &ids[0], "body"),
        core_hlc
    );

    let report = import_with(&store, &parsed, Some(export.path()), Language::Ru);
    assert_eq!(report.media_attached, 2, "{report:?}");
    let after = feed(&store);
    assert_eq!(
        (after[0].body_md.as_str(), after[1].body_md.as_str()),
        ("утро", "day")
    );
    for id in &ids {
        let body = common::ops::group_hlc(data.path(), "message", id, "body");
        let core = common::ops::group_hlc(data.path(), "message", id, "core");
        assert!(
            body > core,
            "the body HLC is strictly greater: {body} > {core}"
        );
        let edited: Option<i64> = raw(data.path())
            .query_row("SELECT edited_at FROM message WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .unwrap();
        assert!(edited.is_some());
    }
    assert_eq!(
        count(data.path(), "SELECT COUNT(*) FROM message_revision"),
        0,
        "no revision is written"
    );
    // The search finds the text and the file name.
    assert_eq!(
        store
            .search("утро", &Default::default(), None, 10)
            .unwrap()
            .hits
            .len(),
        1
    );
    assert_eq!(
        store
            .search("a", &Default::default(), None, 10)
            .unwrap()
            .hits
            .len(),
        1,
        "the stem of a.jpg"
    );
}

#[test]
fn an_entry_edited_since_import_keeps_its_text_and_still_gets_its_file() {
    let data = tempfile::tempdir().unwrap();
    let store = open_without_mirror(data.path());
    let export = Export::new();
    export.put("photos/a.jpg", &jpeg(30, 30));
    let parsed = parse(&export_json(&[photo(1, "photos/a.jpg", "")]));
    import_with(&store, &parsed, None, Language::Ru);
    let id = feed(&store)[0].id.clone();
    store
        .edit_message(&id, "_[Фото не импортировано]_\nмоя правка", &now())
        .unwrap();
    let revisions = count(data.path(), "SELECT COUNT(*) FROM message_revision");
    let report = import_with(&store, &parsed, Some(export.path()), Language::Ru);
    assert_eq!(report.media_attached, 1);
    let after = feed(&store);
    assert_eq!(
        after[0].body_md, "_[Фото не импортировано]_\nмоя правка",
        "edited text is never touched"
    );
    assert_eq!(after[0].attachments.len(), 1);
    assert_eq!(
        count(data.path(), "SELECT COUNT(*) FROM message_revision"),
        revisions
    );

    // The text is edited back to the exact placeholder: the HLC still says it was edited, so it is not rewritten either.
    let second = Export::new();
    second.put("photos/a.jpg", &jpeg(30, 30));
    let other = parse(&export_json(&[photo(9, "photos/a.jpg", "")]));
    import_with(&store, &other, None, Language::Ru);
    let id9 = feed(&store)
        .into_iter()
        .find(|m| m.body_md.contains("не импортировано") && m.id != id)
        .unwrap()
        .id;
    store.edit_message(&id9, "временно", &now()).unwrap();
    store
        .edit_message(&id9, "_[Фото не импортировано]_", &now())
        .unwrap();
    import_with(&store, &other, Some(second.path()), Language::Ru);
    let kept = feed(&store).into_iter().find(|m| m.id == id9).unwrap();
    assert_eq!(kept.body_md, "_[Фото не импортировано]_");
    assert_eq!(kept.attachments.len(), 1);
}

#[test]
fn an_entry_that_has_files_in_the_trash_is_not_given_them_again() {
    let data = tempfile::tempdir().unwrap();
    let store = open_without_mirror(data.path());
    let export = Export::new();
    export.put("photos/a.jpg", &jpeg(30, 30));
    let parsed = parse(&export_json(&[photo(1, "photos/a.jpg", "кадр")]));
    import_with(&store, &parsed, Some(export.path()), Language::Ru);
    let id = feed(&store)[0].id.clone();
    store.delete_message(&id, &now()).unwrap();
    let report = import_with(&store, &parsed, Some(export.path()), Language::Ru);
    assert_eq!(
        (report.imported, report.media_attached),
        (0, 0),
        "{report:?}"
    );
    assert_eq!(count(data.path(), "SELECT COUNT(*) FROM attachment"), 1);
    assert_eq!(stored(data.path()).len(), 1);
}

#[test]
fn hostile_and_broken_paths_are_counted_and_nothing_outside_the_folder_is_read() {
    let data = tempfile::tempdir().unwrap();
    let store = open_without_mirror(data.path());
    let outer = tempfile::tempdir().unwrap();
    std::fs::write(outer.path().join("secret.jpg"), jpeg(20, 20)).unwrap();
    let export = Export::new();
    export.put("photos/inside.jpg", &jpeg(20, 20));
    export.put("real/target.jpg", &jpeg(22, 22));
    std::fs::create_dir(export.path().join("folder.jpg")).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        symlink(
            outer.path().join("secret.jpg"),
            export.path().join("photos/out.jpg"),
        )
        .unwrap();
        symlink(outer.path(), export.path().join("photos/outdir")).unwrap();
        symlink(
            export.path().join("real/target.jpg"),
            export.path().join("photos/in.jpg"),
        )
        .unwrap();
        let fifo = export.path().join("photos/pipe.jpg");
        let c = std::ffi::CString::new(fifo.to_str().unwrap()).unwrap();
        #[allow(unsafe_code)]
        let made = unsafe { libc::mkfifo(c.as_ptr(), 0o600) };
        assert_eq!(made, 0);
    }
    let absolute = outer
        .path()
        .join("secret.jpg")
        .to_string_lossy()
        .into_owned();
    let paths = [
        "../secret.jpg".to_string(),
        "photos/../../secret.jpg".to_string(),
        absolute,
        "/etc/passwd".to_string(),
        "C:\\Windows\\win.ini".to_string(),
        "C:/Windows/win.ini".to_string(),
        "photos\\inside.jpg".to_string(),
        "photos/a\u{0}b.jpg".to_string(),
        "photos/out.jpg".to_string(),
        "photos/outdir/secret.jpg".to_string(),
        "folder.jpg".to_string(),
        "photos/pipe.jpg".to_string(),
        "photos/nothing.jpg".to_string(),
        "".to_string(),
        ".".to_string(),
        "photos/in.jpg".to_string(),
        "photos/./inside.jpg".to_string(),
    ];
    let messages: Vec<String> = paths
        .iter()
        .enumerate()
        .map(|(i, p)| photo(i as u64 + 1, p, &format!("п{i}")))
        .collect();
    let parsed = parse(&export_json(&messages));
    let report = import_with(&store, &parsed, Some(export.path()), Language::Ru);
    assert_eq!(report.imported, paths.len() as i64);
    // Only the symlink that stays inside the folder and the path with a `.` in it are copied. The empty path is "not in the
    // export" like the note, and a file that is not there is missing; the other thirteen are refused.
    assert_eq!(report.media_attached, 2, "{report:?}");
    assert_eq!(report.media_missing, 2, "{report:?}");
    assert_eq!(report.media_skipped, 13, "{report:?}");
    assert_eq!(count(data.path(), "SELECT COUNT(*) FROM attachment"), 2);
    for entry in feed(&store) {
        for attachment in entry.attachments {
            assert!(
                !attachment.filename.contains("secret"),
                "the outside file was copied"
            );
        }
    }
}

#[test]
fn stopping_between_files_keeps_what_was_done_and_a_repeat_finishes_without_duplicates() {
    let data = tempfile::tempdir().unwrap();
    let store = open_without_mirror(data.path());
    let export = Export::new();
    let messages: Vec<String> = (1..=250)
        .map(|i| {
            let path = format!("photos/p{i}.jpg");
            export.put(&path, &jpeg(16, 16 + (i % 5) as u32));
            photo(i, &path, &format!("кадр {i}"))
        })
        .collect();
    let parsed = parse(&export_json(&messages));
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let batches = Arc::new(Mutex::new(0));
    let counted = batches.clone();
    let report = store
        .import_telegram_with_media(
            parsed.clone(),
            all_chats(&parsed),
            Language::Ru,
            &now(),
            move |_| {
                *counted.lock().unwrap() += 1;
                flag.store(true, Ordering::SeqCst);
            },
            stop,
            Some(export.path()),
        )
        .unwrap();
    assert!(report.stopped);
    assert_eq!(
        *batches.lock().unwrap(),
        1,
        "the batch was cut at 100 entries with files"
    );
    assert_eq!(report.imported, 100, "{report:?}");
    assert_eq!(count(data.path(), "SELECT COUNT(*) FROM attachment"), 100);

    let finished = import_with(&store, &parsed, Some(export.path()), Language::Ru);
    assert_eq!(
        (
            finished.imported,
            finished.skipped_existing,
            finished.media_attached
        ),
        (150, 100, 150),
        "{finished:?}"
    );
    assert_eq!(count(data.path(), "SELECT COUNT(*) FROM attachment"), 250);
    assert_eq!(
        count(
            data.path(),
            "SELECT COUNT(*) FROM (SELECT message_id FROM attachment GROUP BY message_id HAVING COUNT(*) > 1)"
        ),
        0
    );
    assert_eq!(stored(data.path()).len(), 250);
}

#[test]
fn a_full_disk_rolls_the_batch_back_and_says_so() {
    let data = tempfile::tempdir().unwrap();
    let store = open_without_mirror(data.path());
    let export = Export::new();
    let mut messages = Vec::new();
    for i in 1..=120u64 {
        let path = format!("photos/p{i}.bin");
        // The 120th file is the only big one.
        let size = if i == 120 { 4096 } else { 64 };
        export.put(&path, &vec![7u8; size]);
        messages.push(file(i, &path, &format!("p{i}.bin"), None, ""));
    }
    let parsed = parse(&export_json(&messages));
    store.inject_attach_fault_for_test(AttachFault::DiskFullAfter(1000));
    let report = import_with(&store, &parsed, Some(export.path()), Language::Ru);
    assert!(report.disk_full, "{report:?}");
    assert_eq!(
        report.imported, 100,
        "the first batch was committed, the second rolled back"
    );
    assert_eq!(count(data.path(), "SELECT COUNT(*) FROM message"), 100);
    assert_eq!(count(data.path(), "SELECT COUNT(*) FROM attachment"), 100);
    assert_eq!(stored(data.path()).len(), 100);
    let tmp: Vec<String> = std::fs::read_dir(data.path().join("attachments/.tmp"))
        .map(|d| d.flatten().flat_map(|e| names(&e.path())).collect())
        .unwrap_or_default();
    assert!(
        tmp.is_empty(),
        "the copies made for the rolled-back batch are gone: {tmp:?}"
    );
    store.inject_attach_fault_for_test(AttachFault::Clear);
    let rest = import_with(&store, &parsed, Some(export.path()), Language::Ru);
    assert_eq!((rest.imported, rest.media_attached), (20, 20), "{rest:?}");
}

/// Child process: imports 300 photos and is killed by `abort` in the middle of copying the 150th file.
#[test]
fn kill_in_the_middle_of_a_file_loses_at_most_one_batch_and_the_next_run_finishes() {
    if let Ok(dir) = std::env::var(CHILD_ENV) {
        let dir = PathBuf::from(dir);
        let store = open_without_mirror(&dir);
        let copied = Arc::new(Mutex::new(0u32));
        store.inject_attach_fault_for_test(AttachFault::MidCopy(Arc::new(move || {
            let mut n = copied.lock().unwrap();
            *n += 1;
            if *n == 150 {
                std::process::abort();
            }
        })));
        let export = parse(&std::fs::read_to_string(dir.join("export/result.json")).unwrap());
        let _ = import_with(&store, &export, Some(&dir.join("export")), Language::Ru);
        return;
    }
    let data = tempfile::tempdir().unwrap();
    let folder = data.path().join("export");
    std::fs::create_dir_all(folder.join("photos")).unwrap();
    let messages: Vec<String> = (1..=300u64)
        .map(|i| {
            let path = format!("photos/p{i}.jpg");
            std::fs::write(folder.join(&path), jpeg(16, 16 + (i % 7) as u32)).unwrap();
            photo(i, &path, "")
        })
        .collect();
    let json = export_json(&messages);
    std::fs::write(folder.join("result.json"), &json).unwrap();
    let status = Command::new(std::env::current_exe().unwrap())
        .args([
            "kill_in_the_middle_of_a_file_loses_at_most_one_batch_and_the_next_run_finishes",
            "--exact",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD_ENV, data.path())
        .status()
        .unwrap();
    assert!(!status.success(), "the child must have been killed");

    let store = open_without_mirror(data.path());
    let committed = count(data.path(), "SELECT COUNT(*) FROM attachment");
    assert_eq!(
        committed, 100,
        "the first batch of 100 was committed, the one in progress was lost"
    );
    assert_eq!(count(data.path(), "SELECT COUNT(*) FROM message"), 100);
    // The copies of the lost batch are leftovers of a dead process; the sweep removes them.
    let report = store.sweep_attachments(&now()).unwrap();
    assert!(report.stale_dirs >= 1, "{report:?}");
    let parsed = parse(&json);
    let rest = import_with(&store, &parsed, Some(&folder), Language::Ru);
    assert_eq!(
        (rest.imported, rest.skipped_existing, rest.media_attached),
        (200, 100, 200),
        "{rest:?}"
    );
    assert_eq!(count(data.path(), "SELECT COUNT(*) FROM attachment"), 300);
    assert_eq!(stored(data.path()).len(), 300);
}

#[test]
fn files_land_in_the_markdown_copy_of_their_days() {
    let data = tempfile::tempdir().unwrap();
    let store = open(data.path());
    let export = Export::new();
    export.put("photos/IMG_0001.jpg", &jpeg(20, 20));
    let parsed = parse(&export_json(&[photo(1, "photos/IMG_0001.jpg", "кадр")]));
    import_with(&store, &parsed, Some(export.path()), Language::Ru);
    let year = mirror_dir(data.path()).join("Inbox/2019");
    let day_file = year.join("2019-03-08.md");
    wait_until("the day file", || {
        std::fs::read_to_string(&day_file).is_ok_and(|t| t.contains("IMG_0001.jpg"))
    });
    let text = std::fs::read_to_string(&day_file).unwrap();
    assert!(
        text.contains("![IMG_0001.jpg](<2019-03-08/IMG_0001.jpg>)"),
        "{text}"
    );
    wait_until("the file", || {
        year.join("2019-03-08/IMG_0001.jpg").is_file()
    });
}
