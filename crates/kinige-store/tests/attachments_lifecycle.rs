//! Attachments over time (slice 7): the trash, restore and "Delete forever" with tombstones, the sweep (with a crash between
//! the rename and the commit, with a concurrent attach, with a failed deletion), the Markdown copy, migration 6 to 7 and the
//! rule that the keys never leave the database.

#![cfg(test)]

mod common;

use common::attach::*;
use common::days::at;
use common::*;
use kinige_domain::groups::INBOX_ID;
use kinige_domain::history::DAY_MS;
use kinige_store::{AttachFault, LATEST_VERSION, MediaVariant, Now, Store, StoreError};
use std::path::Path;
use std::sync::Arc;

fn now() -> Now {
    at(2026, 11, 3, 12, 0)
}

fn send_files(
    store: &Store,
    dir: &Path,
    files: &[(&str, Vec<u8>)],
    body: &str,
) -> kinige_domain::MessageView {
    let source = dir.join("src");
    std::fs::create_dir_all(&source).unwrap();
    let mut ids = Vec::new();
    for (index, (name, bytes)) in files.iter().enumerate() {
        // Every file in its own folder: the same name twice is a real case (two "IMG_0001.jpg").
        let folder = source.join(index.to_string());
        std::fs::create_dir_all(&folder).unwrap();
        ids.push(
            store
                .attach_path(INBOX_ID, &write(&folder, name, bytes), &now())
                .unwrap()
                .id,
        );
    }
    store
        .send_entry(INBOX_ID, body, None, &ids, &now())
        .unwrap()
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
fn the_trash_hides_attachments_with_the_entry_and_restore_brings_them_back() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let sent = send_files(
        &store,
        dir.path(),
        &[("a.jpg", jpeg(40, 30)), ("menu.pdf", b"%PDF".to_vec())],
        "обед",
    );
    let ids: Vec<String> = sent.attachments.iter().map(|a| a.id.clone()).collect();
    let groups_before = raw(dir.path())
        .query_row(
            "SELECT groups_raw FROM attachment WHERE id = ?1",
            [&ids[0]],
            |r| r.get::<_, String>(0),
        )
        .unwrap();

    store.delete_message(&sent.id, &now()).unwrap();
    let trash = store.list_trash(&now()).unwrap();
    assert_eq!(trash.len(), 1);
    let shown: Vec<&str> = trash[0]
        .message
        .attachments
        .iter()
        .map(|a| a.id.as_str())
        .collect();
    assert_eq!(
        shown,
        ids.iter().map(String::as_str).collect::<Vec<_>>(),
        "the trash shows them"
    );
    assert_eq!(
        stored(dir.path()).len(),
        2,
        "the files stay while the entry is in the trash"
    );
    let after = raw(dir.path())
        .query_row(
            "SELECT groups_raw FROM attachment WHERE id = ?1",
            [&ids[0]],
            |r| r.get::<_, String>(0),
        )
        .unwrap();
    assert_eq!(
        after, groups_before,
        "their own `deleted` group is not touched"
    );
    assert!(
        store.media(&ids[0], MediaVariant::Full).is_some(),
        "and the viewer can still open them"
    );

    store.restore_message(&sent.id, &now()).unwrap();
    let page = store
        .list_messages_page(INBOX_ID, kinige_store::PageAt::Latest, 50, None, &now())
        .unwrap();
    assert_eq!(page.messages[0].attachments.len(), 2);
}

#[test]
fn delete_forever_writes_tombstones_and_removes_rows_files_and_thumbnails() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let sent = send_files(
        &store,
        dir.path(),
        &[("a.jpg", jpeg(1200, 800)), ("b.png", png(50, 50))],
        "двое",
    );
    let ids: Vec<String> = sent.attachments.iter().map(|a| a.id.clone()).collect();
    assert!(store.media(&ids[0], MediaVariant::Thumb).is_some());
    assert!(dir.path().join("thumbs").read_dir().unwrap().count() > 0);
    store.prepare_open(&ids[1]).unwrap();
    store.delete_message(&sent.id, &now()).unwrap();
    store.purge_message(&sent.id, &now()).unwrap();

    assert_eq!(count(dir.path(), "SELECT COUNT(*) FROM attachment"), 0);
    assert_eq!(
        count(dir.path(), "SELECT COUNT(*) FROM attachment_local"),
        0
    );
    assert!(
        stored(dir.path()).is_empty(),
        "files are gone after the commit"
    );
    assert!(names(&dir.path().join("thumbs")).is_empty());
    assert!(
        !dir.path()
            .join("open")
            .join(common_process_dir(dir.path()))
            .join(&ids[1])
            .exists()
    );
    for id in &ids {
        let (cascade, purged_of): (String, String) = raw(dir.path())
            .query_row(
                "SELECT cascade_of, purged_of FROM tombstone WHERE entity = 'attachment' AND id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(cascade, sent.id);
        let snapshot: serde_json::Value = serde_json::from_str(&purged_of).unwrap();
        let groups: Vec<&str> = snapshot
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            groups,
            ["core", "deleted", "order"],
            "a snapshot of the HLCs of all groups"
        );
        assert!(
            store.media(id, MediaVariant::Full).is_none(),
            "a purged id is a 404"
        );
    }
    assert_eq!(
        count(
            dir.path(),
            "SELECT COUNT(*) FROM tombstone WHERE entity = 'message'"
        ),
        1
    );
}

fn common_process_dir(dir: &Path) -> String {
    names(&dir.join("open"))
        .into_iter()
        .next()
        .unwrap_or_default()
}

#[test]
fn auto_delete_after_thirty_days_removes_the_files_too() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let sent = send_files(&store, dir.path(), &[("a.png", png(20, 20))], "старое");
    store
        .delete_message(&sent.id, &at(2026, 10, 1, 9, 0))
        .unwrap();
    assert_eq!(store.purge_expired(&at(2026, 10, 25, 9, 0)).unwrap(), 0);
    assert_eq!(stored(dir.path()).len(), 1);
    assert_eq!(
        store
            .purge_expired(&at(2026, 10, 1, 9, 0).tap(|n| n.ms += 31 * DAY_MS))
            .unwrap(),
        1
    );
    assert!(stored(dir.path()).is_empty());
    assert_eq!(
        count(
            dir.path(),
            "SELECT COUNT(*) FROM tombstone WHERE entity = 'attachment'"
        ),
        1
    );
}

trait Tap: Sized {
    fn tap(self, f: impl FnOnce(&mut Self)) -> Self;
}
impl Tap for Now {
    fn tap(mut self, f: impl FnOnce(&mut Self)) -> Self {
        f(&mut self);
        self
    }
}

#[test]
fn a_purge_whose_deletion_failed_is_finished_by_the_next_sweep_not_left_for_thirty_days() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let sent = send_files(
        &store,
        dir.path(),
        &[("a.png", png(20, 20)), ("b.png", png(21, 21))],
        "x",
    );
    store.inject_attach_fault_for_test(AttachFault::DeleteFails);
    store.delete_message(&sent.id, &now()).unwrap();
    store.purge_message(&sent.id, &now()).unwrap();
    assert_eq!(
        stored(dir.path()).len(),
        2,
        "the injected failure left the files"
    );
    store.inject_attach_fault_for_test(AttachFault::Clear);
    let report = store.sweep_attachments(&now()).unwrap();
    assert_eq!((report.deleted, report.orphaned), (2, 0), "{report:?}");
    assert!(stored(dir.path()).is_empty());
    assert!(names(&dir.path().join("attachments/.orphans")).is_empty());
}

#[test]
fn a_crash_between_rename_and_commit_leaves_no_row_and_the_sweep_moves_the_file_away() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    store.inject_attach_fault_for_test(AttachFault::AfterRename(Arc::new(|| {
        std::panic::resume_unwind(Box::new("simulated crash"));
    })));
    let path = write(dir.path(), "a.png", &png(10, 10));
    let error = store.attach_path(INBOX_ID, &path, &now()).unwrap_err();
    assert!(matches!(error, StoreError::Internal(_)), "{error}");
    store.inject_attach_fault_for_test(AttachFault::Clear);
    assert_eq!(
        count(dir.path(), "SELECT COUNT(*) FROM draft_attachment"),
        0,
        "no half row"
    );
    assert_eq!(stored(dir.path()).len(), 1, "the renamed file has no row");
    // The leftovers of a crashed process: its `.tmp` directory and a clone directory.
    let dead = dir
        .path()
        .join("attachments/.tmp/00000000-0000-4000-8000-00000000dead");
    std::fs::create_dir_all(&dead).unwrap();
    std::fs::write(dead.join("x.part"), b"half").unwrap();
    std::fs::create_dir_all(
        dir.path()
            .join("open/00000000-0000-4000-8000-00000000dead/x"),
    )
    .unwrap();

    let report = store.sweep_attachments(&now()).unwrap();
    assert_eq!((report.orphaned, report.deleted), (1, 0), "{report:?}");
    assert_eq!(report.stale_dirs, 2);
    assert!(stored(dir.path()).is_empty());
    assert_eq!(names(&dir.path().join("attachments/.orphans")).len(), 1);
    assert!(!dead.exists());
    // The store still works, and attaching the same file again gives one row.
    store.attach_path(INBOX_ID, &path, &now()).unwrap();
    assert_eq!(
        count(dir.path(), "SELECT COUNT(*) FROM draft_attachment"),
        1
    );
}

#[test]
fn the_sweep_runs_while_files_are_being_attached_and_loses_none() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let source = dir.path().join("src");
    std::fs::create_dir_all(&source).unwrap();
    let sweeper = {
        let store = store.clone();
        std::thread::Builder::new()
            .name("sweeper".into())
            .spawn(move || {
                let mut orphaned = 0;
                for _ in 0..40 {
                    orphaned += store.sweep_attachments(&now()).unwrap().orphaned;
                    std::thread::sleep(std::time::Duration::from_millis(2));
                }
                orphaned
            })
            .unwrap()
    };
    let mut attached = Vec::new();
    for n in 0..10 {
        let path = write(&source, &format!("f{n}.png"), &png(30 + n, 30));
        attached.push(store.attach_path(INBOX_ID, &path, &now()).unwrap().id);
        // Free the tile so the limit of ten never gets in the way of the race.
        if n % 5 == 4 {
            let sent = store
                .send_entry(INBOX_ID, "x", None, &attached[n as usize - 4..], &now())
                .unwrap();
            assert_eq!(sent.attachments.len(), 5);
        }
    }
    assert_eq!(
        sweeper.join().unwrap(),
        0,
        "the sweep never took a live file"
    );
    assert_eq!(stored(dir.path()).len(), 10);
    assert_eq!(
        count(
            dir.path(),
            "SELECT COUNT(*) FROM attachment_local WHERE state = 'local'"
        ),
        10
    );
}

#[test]
fn the_sweep_marks_missing_files_and_gives_files_back_from_orphans() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let sent = send_files(
        &store,
        dir.path(),
        &[("a.png", png(20, 20)), ("b.png", png(21, 21))],
        "два",
    );
    let [first, second] = [&sent.attachments[0], &sent.attachments[1]];
    let attachments = dir.path().join("attachments");
    // One file is gone for good, the other lies in `.orphans/` (a newer database after an older backup was restored).
    std::fs::remove_file(attachments.join(format!("{}.png", first.id))).unwrap();
    std::fs::create_dir_all(attachments.join(".orphans")).unwrap();
    std::fs::rename(
        attachments.join(format!("{}.png", second.id)),
        attachments
            .join(".orphans")
            .join(format!("{}.png", second.id)),
    )
    .unwrap();

    let report = store.sweep_attachments(&now()).unwrap();
    assert_eq!((report.missing, report.restored), (1, 1), "{report:?}");
    let page = store
        .list_messages_page(INBOX_ID, kinige_store::PageAt::Latest, 50, None, &now())
        .unwrap();
    let states: Vec<_> = page.messages[0]
        .attachments
        .iter()
        .map(|a| (a.id.clone(), a.state))
        .collect();
    assert_eq!(
        states[0],
        (first.id.clone(), kinige_domain::AttachmentState::Missing)
    );
    assert_eq!(
        states[1],
        (second.id.clone(), kinige_domain::AttachmentState::Local)
    );
    assert!(attachments.join(format!("{}.png", second.id)).is_file());
    // The file comes back (restoring a backup of the files): the next sweep sets `local` again.
    std::fs::write(attachments.join(format!("{}.png", first.id)), png(20, 20)).unwrap();
    store.sweep_attachments(&now()).unwrap();
    assert_eq!(
        count(
            dir.path(),
            "SELECT COUNT(*) FROM attachment_local WHERE state = 'missing'"
        ),
        0
    );
    // A second sweep changes nothing.
    let again = store.sweep_attachments(&now()).unwrap();
    assert_eq!(
        (again.missing, again.restored, again.orphaned, again.deleted),
        (0, 0, 0, 0)
    );
}

#[test]
fn a_file_older_than_thirty_days_that_loses_its_row_waits_thirty_days_from_the_move() {
    // Day 0 a photo is attached, day 40 its row is gone (an older backup restored): the file is set aside, not deleted in the same
    // pass, and a newer backup restored within 30 days still finds it.
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let stored = dir
        .path()
        .join("attachments/00000000-0000-4000-8000-00000000dddd.png");
    std::fs::create_dir_all(stored.parent().unwrap()).unwrap();
    std::fs::write(&stored, b"photo").unwrap();
    let long_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(60 * 24 * 3600);
    std::fs::OpenOptions::new()
        .write(true)
        .open(&stored)
        .unwrap()
        .set_modified(long_ago)
        .unwrap();
    let orphan = dir
        .path()
        .join("attachments/.orphans/00000000-0000-4000-8000-00000000dddd.png");

    let day40 = Now::system();
    let report = store.sweep_attachments(&day40).unwrap();
    assert_eq!(
        (report.orphaned, report.orphans_expired),
        (1, 0),
        "{report:?}"
    );
    assert!(orphan.is_file() && !stored.exists());

    let later = |days: i64| Now::at(day40.ms + days * 24 * 3600 * 1000, &day40.tz);
    store.sweep_attachments(&later(29)).unwrap();
    assert!(orphan.is_file(), "still there after 29 days");
    let report = store.sweep_attachments(&later(31)).unwrap();
    assert_eq!(report.orphans_expired, 1, "{report:?}");
    assert!(!orphan.exists(), "gone 31 days after the move");
}

#[test]
fn orphans_are_deleted_after_thirty_days_and_unowned_thumbnails_at_once() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let attachments = dir.path().join("attachments/.orphans");
    std::fs::create_dir_all(&attachments).unwrap();
    let old = attachments.join("00000000-0000-4000-8000-00000000aaaa.bin");
    let fresh = attachments.join("00000000-0000-4000-8000-00000000bbbb.bin");
    std::fs::write(&old, b"old").unwrap();
    std::fs::write(&fresh, b"fresh").unwrap();
    let long_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(31 * 24 * 3600);
    std::fs::OpenOptions::new()
        .write(true)
        .open(&old)
        .unwrap()
        .set_modified(long_ago)
        .unwrap();
    std::fs::create_dir_all(dir.path().join("thumbs")).unwrap();
    std::fs::write(
        dir.path()
            .join("thumbs/00000000-0000-4000-8000-00000000cccc.jpg"),
        b"t",
    )
    .unwrap();
    std::fs::write(
        dir.path()
            .join("thumbs/.00000000-0000-4000-8000-00000000cccc.tmp"),
        b"t",
    )
    .unwrap();
    std::fs::write(dir.path().join("thumbs/notes.txt"), b"not ours").unwrap();

    let report = store.sweep_attachments(&Now::system()).unwrap();
    assert_eq!(
        (report.orphans_expired, report.thumbs_removed),
        (1, 2),
        "{report:?}"
    );
    assert!(!old.exists() && fresh.exists());
    assert_eq!(
        names(&dir.path().join("thumbs")),
        ["notes.txt"],
        "anything that is not a thumbnail is left alone"
    );
}

#[test]
fn the_markdown_copy_links_the_files_in_a_folder_per_day() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path());
    let sent = send_files(
        &store,
        dir.path(),
        &[
            ("IMG_0001.jpg", jpeg(30, 30)),
            ("IMG_0001.jpg", jpeg(31, 31)),
            ("menu.pdf", b"%PDF-1".to_vec()),
        ],
        "Обед",
    );
    let year = mirror_dir(dir.path()).join("Inbox/2026");
    let day_file = year.join("2026-11-03.md");
    wait_until("the day file with links", || {
        std::fs::read_to_string(&day_file).is_ok_and(|t| t.contains("menu.pdf"))
    });
    let text = std::fs::read_to_string(&day_file).unwrap();
    assert_eq!(
        text,
        "# 2026-11-03\n\n## 12:00\n\n![IMG_0001.jpg](<2026-11-03/IMG_0001.jpg>)\n\n![IMG_0001.jpg](<2026-11-03/IMG_0001 (2).jpg>)\n\n[menu.pdf](<2026-11-03/menu.pdf>)\n\nОбед\n"
    );
    let day_dir = year.join("2026-11-03");
    wait_until("the files", || names(&day_dir).len() == 3);
    assert_eq!(
        names(&day_dir),
        ["IMG_0001 (2).jpg", "IMG_0001.jpg", "menu.pdf"]
    );
    assert_eq!(std::fs::read(day_dir.join("menu.pdf")).unwrap(), b"%PDF-1");
    assert_eq!(count(dir.path(), "SELECT COUNT(*) FROM mirror_file"), 3);

    // A file the user put there is never deleted; ours go when the entry goes to the trash.
    std::fs::write(day_dir.join("my own notes.txt"), b"mine").unwrap();
    store.delete_message(&sent.id, &now()).unwrap();
    wait_until("our files and the day file to go", || {
        names(&day_dir) == ["my own notes.txt"]
            && !day_file.exists()
            && count(dir.path(), "SELECT COUNT(*) FROM mirror_file") == 0
    });
    assert_eq!(
        std::fs::read(day_dir.join("my own notes.txt")).unwrap(),
        b"mine"
    );

    store.restore_message(&sent.id, &now()).unwrap();
    wait_until("the files to return", || names(&day_dir).len() == 4);
    wait_until("the day file", || day_file.is_file());
}

#[test]
fn a_photo_without_text_has_only_its_link_in_the_copy_and_the_folder_goes_with_the_last_file() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path());
    let sent = send_files(&store, dir.path(), &[("solo.png", png(9, 9))], "");
    let day_file = mirror_dir(dir.path()).join("Inbox/2026/2026-11-03.md");
    wait_until("the day file", || day_file.is_file());
    assert_eq!(
        std::fs::read_to_string(&day_file).unwrap(),
        "# 2026-11-03\n\n## 12:00\n\n![solo.png](<2026-11-03/solo.png>)\n"
    );
    store.delete_message(&sent.id, &now()).unwrap();
    store.purge_message(&sent.id, &now()).unwrap();
    wait_until("the copy to be empty", || {
        !mirror_dir(dir.path()).join("Inbox/2026").exists()
    });
}

#[test]
fn migration_six_to_seven_keeps_drafts_and_takes_a_copy_first() {
    let dir = tempfile::tempdir().unwrap();
    {
        let store = open_without_mirror(dir.path());
        store
            .save_chat_draft(INBOX_ID, "черновик до миграции 7", None, &now())
            .unwrap();
    }
    common::ops::wait_closed(dir.path());
    {
        let conn = raw(dir.path());
        conn.execute_batch("DROP TABLE attachment; DROP TABLE attachment_local; DROP TABLE draft_attachment; DROP TABLE mirror_file;")
            .unwrap();
        conn.pragma_update(None, "user_version", 6).unwrap();
    }
    let later = moscow(T0 + 10 * MINUTE);
    let mut opts = options(dir.path());
    opts.start_mirror = false;
    let store = Store::open_at(opts, &later).unwrap();
    let version: i64 = raw(dir.path())
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, LATEST_VERSION);
    assert_eq!(LATEST_VERSION, 7);
    assert_eq!(
        store.chat_draft(INBOX_ID, &later).unwrap().body,
        "черновик до миграции 7"
    );
    let copies = names(&dir.path().join("backups"));
    assert!(
        copies.iter().any(|n| n.starts_with("pre-migration-v6-")),
        "{copies:?}"
    );
    // The new tables work.
    let path = write(dir.path(), "a.png", &png(5, 5));
    assert!(store.attach_path(INBOX_ID, &path, &later).is_ok());
}

#[test]
fn the_keys_never_leave_the_database() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path());
    let sent = send_files(
        &store,
        dir.path(),
        &[("a.jpg", jpeg(20, 20)), ("b.pdf", b"%PDF".to_vec())],
        "секреты",
    );
    let (blob_key, blob_id): (String, String) = raw(dir.path())
        .query_row(
            "SELECT blob_key, remote_blob_id FROM attachment ORDER BY sort_index LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    let all_keys: Vec<(String, String)> = raw(dir.path())
        .prepare("SELECT blob_key, remote_blob_id FROM attachment")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert!(all_keys.contains(&(blob_key.clone(), blob_id.clone())));

    let page = store
        .list_messages_page(INBOX_ID, kinige_store::PageAt::Latest, 50, None, &now())
        .unwrap();
    store.delete_message(&sent.id, &now()).unwrap();
    let trash = store.list_trash(&now()).unwrap();
    store.restore_message(&sent.id, &now()).unwrap();
    let hits = store
        .search("секреты", &Default::default(), None, 20)
        .unwrap();
    let mut surfaces = vec![
        serde_json::to_string(&sent).unwrap(),
        serde_json::to_string(&page).unwrap(),
        serde_json::to_string(&trash).unwrap(),
        serde_json::to_string(&hits).unwrap(),
        serde_json::to_string(&store.chat_draft(INBOX_ID, &now()).unwrap()).unwrap(),
        serde_json::to_string(
            &store
                .get_forest_year(2026, kinige_domain::FirstWeekday::Monday, &now())
                .unwrap(),
        )
        .unwrap(),
    ];
    // Errors name no key and no path either.
    for error in [
        StoreError::AttachmentNotFound,
        StoreError::AttachmentUnreadable,
        StoreError::DiskFull,
        StoreError::AttachmentLimit,
        StoreError::AttachmentTooLarge,
    ] {
        surfaces.push(format!("{error} {error:?} {}", error.category()));
    }
    // The Markdown copy.
    let year = mirror_dir(dir.path()).join("Inbox/2026");
    wait_until("the copy", || year.join("2026-11-03.md").is_file());
    wait_until("the copy files", || {
        year.join("2026-11-03").is_dir() && names(&year.join("2026-11-03")).len() == 2
    });
    surfaces.push(std::fs::read_to_string(year.join("2026-11-03.md")).unwrap());
    for surface in &surfaces {
        for secret in all_keys.iter().flat_map(|(k, b)| [k, b]) {
            assert!(
                !surface.contains(secret.as_str()),
                "a key or a blob id leaked: {surface:.200}"
            );
        }
    }
    // Nor the stored file names, which are attachment ids plus an extension and say where the file is.
    for surface in &surfaces[..6] {
        assert!(!surface.contains("attachments/"), "{surface:.200}");
    }
}

#[test]
fn nothing_about_the_files_reaches_the_log() {
    // `Store` logs only categories; the source has no `log::` call that formats a name or a path. A grep of the attachment
    // module keeps it that way.
    let sources = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = vec![
        sources.join("attachment.rs"),
        sources.join("mirror_files.rs"),
    ];
    files.extend(
        std::fs::read_dir(sources.join("attachment"))
            .unwrap()
            .flatten()
            .map(|e| e.path()),
    );
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap();
        for (number, line) in text.lines().enumerate() {
            if line.contains("log::") {
                assert!(
                    !line.contains('{') && !line.contains("{}"),
                    "{}:{}: a log line must not format data",
                    file.display(),
                    number + 1
                );
            }
        }
    }
}
