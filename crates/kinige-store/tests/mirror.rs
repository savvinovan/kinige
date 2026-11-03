//! The Markdown copy: the day file after sending and "Undo", a day without entries, an inaccessible folder and a retry, a folder change,
//! atomic writes.

// The root of the integration tests is entirely test code: `cfg(test)` gives helpers outside #[test] the same
// clippy.toml relaxations (unwrap/expect/panic/indexing) as the tests in src/.
#![cfg(test)]

mod common;

use common::*;
use kinige_domain::MirrorError;
use kinige_domain::groups::INBOX_ID;
use std::fs;
use std::path::{Path, PathBuf};

fn day_file(dir: &Path, date: &str) -> PathBuf {
    mirror_dir(dir)
        .join("Inbox")
        .join(date.get(..4).unwrap())
        .join(format!("{date}.md"))
}

fn dirty_count(dir: &Path) -> i64 {
    raw(dir)
        .query_row("SELECT COUNT(*) FROM mirror_dirty", [], |r| r.get(0))
        .unwrap()
}

/// An entry goes to the trash by a real operation: the day is marked for rewriting in the same transaction.
fn delete_entry(store: &kinige_store::Store, _dir: &Path, id: &str, _date: &str) {
    store.delete_message(id, &moscow(T0 + 10 * MINUTE)).unwrap();
}

#[test]
fn sending_writes_the_day_file_with_time_and_deleted_entries_leave_it() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path());
    let file = day_file(dir.path(), "2026-10-21");

    store
        .send_message(
            INBOX_ID,
            "Купить **молоко**\n- [ ] позвонить маме",
            &moscow(T0),
        )
        .unwrap();
    wait_until("day file", || file.exists());
    let second = store
        .send_message(
            INBOX_ID,
            "# Заголовок внутри записи\nвторая",
            &moscow(T0 + 2 * MINUTE),
        )
        .unwrap();
    wait_until("second entry in the file", || {
        fs::read_to_string(&file).is_ok_and(|t| t.contains("вторая"))
    });
    assert_eq!(
        fs::read_to_string(&file).unwrap(),
        "# 2026-10-21\n\n## 09:12\n\nКупить **молоко**\n- [ ] позвонить маме\n\n## 09:14\n\n### Заголовок внутри записи\nвторая\n"
    );

    delete_entry(&store, dir.path(), &second.id, "2026-10-21");
    wait_until("the entry from the trash left the copy", || {
        fs::read_to_string(&file).is_ok_and(|t| !t.contains("вторая"))
    });
    assert_eq!(
        fs::read_to_string(&file).unwrap(),
        "# 2026-10-21\n\n## 09:12\n\nКупить **молоко**\n- [ ] позвонить маме\n"
    );
    wait_until("marks cleared", || dirty_count(dir.path()) == 0);
}

#[test]
fn a_day_without_entries_loses_its_file_and_empty_folders() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path());
    let file = day_file(dir.path(), "2026-10-21");
    let sent = store.send_message(INBOX_ID, "одна", &moscow(T0)).unwrap();
    wait_until("day file", || file.exists());

    delete_entry(&store, dir.path(), &sent.id, "2026-10-21");
    wait_until("file deleted", || !file.exists());
    wait_until("marks cleared", || dirty_count(dir.path()) == 0);
    assert!(
        !mirror_dir(dir.path()).join("Inbox").exists(),
        "empty chat and year folders are removed"
    );
    assert!(mirror_dir(dir.path()).exists(), "the copy root stays");
}

#[test]
fn night_entry_goes_to_the_previous_days_file_with_its_local_time() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path());
    store
        .send_message(INBOX_ID, "не спится", &moscow(T_NIGHT))
        .unwrap();
    let file = day_file(dir.path(), "2026-10-20");
    wait_until("file of yesterday", || file.exists());
    assert_eq!(
        fs::read_to_string(&file).unwrap(),
        "# 2026-10-20\n\n## 00:30\n\nне спится\n"
    );
}

#[test]
fn inbox_folder_name_does_not_depend_on_language_or_title() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path());
    store.send_message(INBOX_ID, "x", &moscow(T0)).unwrap();
    wait_until("day file", || day_file(dir.path(), "2026-10-21").exists());
    let names: Vec<String> = fs::read_dir(mirror_dir(dir.path()))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["Inbox"]);
}

#[test]
fn no_temporary_files_are_left_behind() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path());
    for i in 0..5 {
        store
            .send_message(INBOX_ID, &format!("запись {i}"), &moscow(T0 + i * MINUTE))
            .unwrap();
    }
    wait_until("marks cleared", || dirty_count(dir.path()) == 0);
    let year = mirror_dir(dir.path()).join("Inbox").join("2026");
    let names: Vec<String> = fs::read_dir(&year)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["2026-10-21.md"]);
}

#[test]
fn marks_left_by_a_killed_process_are_written_at_startup() {
    let dir = tempfile::tempdir().unwrap();
    {
        let store = open_without_mirror(dir.path());
        store
            .send_message(INBOX_ID, "ждёт копии", &moscow(T0))
            .unwrap();
    }
    assert_eq!(dirty_count(dir.path()), 1);
    assert!(!day_file(dir.path(), "2026-10-21").exists());
    let _store = open(dir.path());
    wait_until("file at startup", || {
        day_file(dir.path(), "2026-10-21").exists()
    });
    wait_until("marks cleared", || dirty_count(dir.path()) == 0);
}

#[test]
fn unavailable_folder_is_reported_marks_are_kept_and_retried() {
    let dir = tempfile::tempdir().unwrap();
    // The "folder" is occupied by a regular file: a directory cannot be created.
    fs::write(mirror_dir(dir.path()), "я файл").unwrap();
    let store = open(dir.path());
    store
        .send_message(INBOX_ID, "запись при недоступной папке", &moscow(T0))
        .unwrap();

    wait_until("folder error in settings", || {
        store.get_settings().unwrap().mirror.error == Some(MirrorError::FolderUnavailable)
    });
    let status = store.get_settings().unwrap().mirror;
    assert_eq!(status.pending_days, 1, "the mark is not lost");
    assert!(status.is_default);

    // The folder is fixed: the copy completes by itself, without a new message.
    fs::remove_file(mirror_dir(dir.path())).unwrap();
    let file = day_file(dir.path(), "2026-10-21");
    wait_until("copy after the folder is fixed", || file.exists());
    wait_until("error cleared", || {
        store.get_settings().unwrap().mirror.error.is_none()
    });
    assert_eq!(store.get_settings().unwrap().mirror.pending_days, 0);
    assert!(
        fs::read_to_string(&file)
            .unwrap()
            .contains("запись при недоступной папке")
    );
}

#[cfg(unix)]
#[test]
fn write_failure_inside_an_existing_folder_is_reported_and_retried() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(mirror_dir(dir.path())).unwrap();
    // The folder exists, but it cannot be written to (permissions). If the test runs as root, permissions do not apply, so we skip.
    fs::set_permissions(mirror_dir(dir.path()), fs::Permissions::from_mode(0o555)).unwrap();
    if fs::create_dir(mirror_dir(dir.path()).join("probe")).is_ok() {
        fs::set_permissions(mirror_dir(dir.path()), fs::Permissions::from_mode(0o755)).unwrap();
        return;
    }
    let store = open(dir.path());
    store
        .send_message(INBOX_ID, "не запишется сразу", &moscow(T0))
        .unwrap();
    wait_until("write error", || {
        store.get_settings().unwrap().mirror.error == Some(MirrorError::WriteFailed)
    });
    assert_eq!(store.get_settings().unwrap().mirror.pending_days, 1);

    fs::set_permissions(mirror_dir(dir.path()), fs::Permissions::from_mode(0o755)).unwrap();
    wait_until("copy after the fix", || {
        day_file(dir.path(), "2026-10-21").exists()
    });
}

#[test]
fn choosing_another_folder_rewrites_the_whole_copy_there() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path());
    store
        .send_message(INBOX_ID, "вчера", &moscow(T0 - 24 * 60 * MINUTE))
        .unwrap();
    store
        .send_message(INBOX_ID, "сегодня", &moscow(T0))
        .unwrap();
    wait_until("copy in the default folder", || {
        day_file(dir.path(), "2026-10-21").exists() && day_file(dir.path(), "2026-10-20").exists()
    });
    wait_until("marks cleared", || dirty_count(dir.path()) == 0);

    let other = dir.path().join("другая папка");
    store.set_mirror_dir(&other).unwrap();
    let settings = store.get_settings().unwrap();
    assert_eq!(settings.mirror.folder.as_deref(), other.to_str());
    assert!(!settings.mirror.is_default);

    wait_until("full copy in the new folder", || {
        other.join("Inbox/2026/2026-10-21.md").exists()
            && other.join("Inbox/2026/2026-10-20.md").exists()
    });
    assert!(
        fs::read_to_string(other.join("Inbox/2026/2026-10-20.md"))
            .unwrap()
            .contains("вчера")
    );
    assert!(
        day_file(dir.path(), "2026-10-21").exists(),
        "the old folder is not touched"
    );

    // The choice survives a restart.
    drop(store);
    let store = open(dir.path());
    assert_eq!(
        store.get_settings().unwrap().mirror.folder.as_deref(),
        other.to_str()
    );
}

#[test]
fn without_any_folder_the_copy_is_off_and_marks_wait() {
    let dir = tempfile::tempdir().unwrap();
    let mut options = options(dir.path());
    options.default_mirror_dir = None;
    let store = kinige_store::Store::open_at(options, &moscow(T0)).unwrap();
    store
        .send_message(INBOX_ID, "без папки", &moscow(T0))
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(200));
    let status = store.get_settings().unwrap().mirror;
    assert_eq!(
        (status.folder, status.error, status.pending_days),
        (None, None, 1)
    );

    let chosen = dir.path().join("выбрана");
    store.set_mirror_dir(&chosen).unwrap();
    wait_until("copy after choosing the folder", || {
        chosen.join("Inbox/2026/2026-10-21.md").exists()
    });
}

#[test]
fn an_empty_journal_does_not_touch_the_copy_folder() {
    // On macOS the first access to "Documents" triggers an access prompt; it must appear with the first entry.
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path());
    std::thread::sleep(std::time::Duration::from_millis(250));
    assert!(!mirror_dir(dir.path()).exists());
    assert_eq!(store.get_settings().unwrap().mirror.error, None);

    store.send_message(INBOX_ID, "первая", &moscow(T0)).unwrap();
    wait_until("folder and file after the first entry", || {
        day_file(dir.path(), "2026-10-21").exists()
    });
}

#[test]
fn a_chat_with_the_inbox_flag_but_another_id_does_not_get_the_inbox_folder() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path());
    let impostor = "11111111-1111-4111-8111-111111111111";
    insert_impostor_inbox(dir.path(), impostor);
    store
        .send_message(impostor, "я не Inbox", &moscow(T0))
        .unwrap();
    // The impostor gets a folder by its own name, like any other chat; `Inbox` stays with the real Inbox.
    let file = mirror_dir(dir.path())
        .join("Самозванец")
        .join("2026")
        .join("2026-10-21.md");
    wait_until("file in the folder named after the chat", || file.exists());
    assert!(
        !mirror_dir(dir.path()).join("Inbox").exists(),
        "the Inbox folder is occupied only by the real Inbox"
    );

    store
        .send_message(INBOX_ID, "я настоящий", &moscow(T0 + MINUTE))
        .unwrap();
    wait_until("Inbox file", || day_file(dir.path(), "2026-10-21").exists());
}

#[test]
fn a_failed_read_of_the_marks_is_retried_not_treated_as_nothing_to_write() {
    let dir = tempfile::tempdir().unwrap();
    {
        let store = open_without_mirror(dir.path());
        store
            .send_message(INBOX_ID, "ждёт чтения отметок", &moscow(T0))
            .unwrap();
    }
    // Reading the marks fails ("no table"); the copy must not decide that there is nothing to write and forget the day.
    raw(dir.path())
        .execute_batch("ALTER TABLE mirror_dirty RENAME TO mirror_dirty_away")
        .unwrap();
    let _store = open(dir.path());
    std::thread::sleep(std::time::Duration::from_millis(250));
    assert!(!day_file(dir.path(), "2026-10-21").exists());

    raw(dir.path())
        .execute_batch("ALTER TABLE mirror_dirty_away RENAME TO mirror_dirty")
        .unwrap();
    wait_until("a retry after the read is fixed", || {
        day_file(dir.path(), "2026-10-21").exists()
    });
}

#[test]
fn a_folder_changed_in_the_middle_of_a_batch_restarts_the_pass_and_fills_the_new_folder() {
    // Two entries on different days. While the pass holds the batch it has read (the step after `dirty_batch`), the folder is changed: the old code
    // would have written the batch into the old folder and cleared the marks, leaving the new folder empty.
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path());
    let target = tempfile::tempdir().unwrap();
    let new_root = target.path().join("Kinige-B");
    fs::create_dir(&new_root).unwrap();

    let done = Arc::new(AtomicBool::new(false));
    let hook_store = store.clone();
    let hook_dir = dir.path().to_path_buf();
    let hook_root = new_root.clone();
    let hook_done = done.clone();
    store.set_mirror_batch_hook_for_test(Some(Arc::new(move || {
        if hook_done.swap(true, Ordering::SeqCst) {
            return;
        }
        // Wait for the second entry (the test sends it), then change the folder in the middle of the batch.
        wait_until("second day mark", || dirty_count(&hook_dir) >= 2);
        hook_store.set_mirror_dir(&hook_root).unwrap();
    })));
    store
        .send_message(INBOX_ID, "первый день", &moscow(T0))
        .unwrap();
    store
        .send_message(INBOX_ID, "второй день", &moscow(T0 + 2 * 24 * 60 * MINUTE))
        .unwrap();

    let in_new = |date: &str| {
        new_root
            .join("Inbox")
            .join(date.get(..4).unwrap())
            .join(format!("{date}.md"))
    };
    wait_until("both days in the new folder", || {
        in_new("2026-10-21").exists() && in_new("2026-10-23").exists()
    });
    wait_until("no marks left", || dirty_count(dir.path()) == 0);
    store.set_mirror_batch_hook_for_test(None);
}
