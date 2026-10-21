//! Markdown-копия: файл дня после отправки и «Отменить», день без записей, недоступная папка и повтор, смена папки,
//! атомарная запись.

mod common;

use common::*;
use kinige_domain::MirrorError;
use kinige_domain::groups::INBOX_ID;
use std::fs;
use std::path::{Path, PathBuf};

fn day_file(dir: &Path, date: &str) -> PathBuf {
    mirror_dir(dir)
        .join("Inbox")
        .join(&date[..4])
        .join(format!("{date}.md"))
}

fn dirty_count(dir: &Path) -> i64 {
    raw(dir)
        .query_row("SELECT COUNT(*) FROM mirror_dirty", [], |r| r.get(0))
        .unwrap()
}

#[test]
fn sending_writes_the_day_file_with_time_and_undo_rewrites_it() {
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
    wait_until("файл дня", || file.exists());
    let second = store
        .send_message(
            INBOX_ID,
            "# Заголовок внутри записи\nвторая",
            &moscow(T0 + 2 * MINUTE),
        )
        .unwrap();
    wait_until("вторая запись в файле", || {
        fs::read_to_string(&file).is_ok_and(|t| t.contains("вторая"))
    });
    assert_eq!(
        fs::read_to_string(&file).unwrap(),
        "# 2026-10-21\n\n## 09:12\n\nКупить **молоко**\n- [ ] позвонить маме\n\n## 09:14\n\n### Заголовок внутри записи\nвторая\n"
    );

    store
        .undo_send(&second.id, &moscow(T0 + 3 * MINUTE))
        .unwrap();
    wait_until(
        "запись из корзины ушла из копии",
        || fs::read_to_string(&file).is_ok_and(|t| !t.contains("вторая")),
    );
    assert_eq!(
        fs::read_to_string(&file).unwrap(),
        "# 2026-10-21\n\n## 09:12\n\nКупить **молоко**\n- [ ] позвонить маме\n"
    );
    wait_until("отметки сняты", || dirty_count(dir.path()) == 0);
}

#[test]
fn a_day_without_entries_loses_its_file_and_empty_folders() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path());
    let file = day_file(dir.path(), "2026-10-21");
    let sent = store.send_message(INBOX_ID, "одна", &moscow(T0)).unwrap();
    wait_until("файл дня", || file.exists());

    store.undo_send(&sent.id, &moscow(T0 + MINUTE)).unwrap();
    wait_until("файл удалён", || !file.exists());
    wait_until("отметки сняты", || dirty_count(dir.path()) == 0);
    assert!(
        !mirror_dir(dir.path()).join("Inbox").exists(),
        "пустые папки чата и года убраны"
    );
    assert!(mirror_dir(dir.path()).exists(), "корень копии остаётся");
}

#[test]
fn night_entry_goes_to_the_previous_days_file_with_its_local_time() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path());
    store
        .send_message(INBOX_ID, "не спится", &moscow(T_NIGHT))
        .unwrap();
    let file = day_file(dir.path(), "2026-10-20");
    wait_until("файл вчерашнего дня", || file.exists());
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
    wait_until("файл дня", || {
        day_file(dir.path(), "2026-10-21").exists()
    });
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
    wait_until("отметки сняты", || dirty_count(dir.path()) == 0);
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
    wait_until("файл при старте", || {
        day_file(dir.path(), "2026-10-21").exists()
    });
    wait_until("отметки сняты", || dirty_count(dir.path()) == 0);
}

#[test]
fn unavailable_folder_is_reported_marks_are_kept_and_retried() {
    let dir = tempfile::tempdir().unwrap();
    // «Папка» занята обычным файлом: создать каталог нельзя.
    fs::write(mirror_dir(dir.path()), "я файл").unwrap();
    let store = open(dir.path());
    store
        .send_message(INBOX_ID, "запись при недоступной папке", &moscow(T0))
        .unwrap();

    wait_until("ошибка папки в настройках", || {
        store.get_settings().unwrap().mirror.error == Some(MirrorError::FolderUnavailable)
    });
    let status = store.get_settings().unwrap().mirror;
    assert_eq!(status.pending_days, 1, "отметка не потеряна");
    assert!(status.is_default);

    // Папка починена — копия дописывается сама, без нового сообщения.
    fs::remove_file(mirror_dir(dir.path())).unwrap();
    let file = day_file(dir.path(), "2026-10-21");
    wait_until("копия после починки папки", || {
        file.exists()
    });
    wait_until("ошибка снята", || {
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
    // Папка есть, но писать в неё нельзя (права). Если тест выполняется от root, права не действуют — тогда пропускаем.
    fs::set_permissions(mirror_dir(dir.path()), fs::Permissions::from_mode(0o555)).unwrap();
    if fs::create_dir(mirror_dir(dir.path()).join("probe")).is_ok() {
        fs::set_permissions(mirror_dir(dir.path()), fs::Permissions::from_mode(0o755)).unwrap();
        return;
    }
    let store = open(dir.path());
    store
        .send_message(INBOX_ID, "не запишется сразу", &moscow(T0))
        .unwrap();
    wait_until("ошибка записи", || {
        store.get_settings().unwrap().mirror.error == Some(MirrorError::WriteFailed)
    });
    assert_eq!(store.get_settings().unwrap().mirror.pending_days, 1);

    fs::set_permissions(mirror_dir(dir.path()), fs::Permissions::from_mode(0o755)).unwrap();
    wait_until("копия после починки", || {
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
    wait_until("копия в папке по умолчанию", || {
        day_file(dir.path(), "2026-10-21").exists() && day_file(dir.path(), "2026-10-20").exists()
    });
    wait_until("отметки сняты", || dirty_count(dir.path()) == 0);

    let other = dir.path().join("другая папка");
    store.set_mirror_dir(&other).unwrap();
    let settings = store.get_settings().unwrap();
    assert_eq!(settings.mirror.folder.as_deref(), other.to_str());
    assert!(!settings.mirror.is_default);

    wait_until("полная копия в новой папке", || {
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
        "старая папка не трогается"
    );

    // Выбор переживает перезапуск.
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
    wait_until("копия после выбора папки", || {
        chosen.join("Inbox/2026/2026-10-21.md").exists()
    });
}

#[test]
fn an_empty_journal_does_not_touch_the_copy_folder() {
    // На macOS первое обращение к «Документам» вызывает запрос доступа; он должен появиться с первой записью.
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path());
    std::thread::sleep(std::time::Duration::from_millis(250));
    assert!(!mirror_dir(dir.path()).exists());
    assert_eq!(store.get_settings().unwrap().mirror.error, None);

    store.send_message(INBOX_ID, "первая", &moscow(T0)).unwrap();
    wait_until(
        "папка и файл после первой записи",
        || day_file(dir.path(), "2026-10-21").exists(),
    );
}
