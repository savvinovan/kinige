//! Перенос записи и Markdown-копия «до конца»: файл дня уходит из папки старого чата и появляется в папке нового (с
//! настоящим фоновым писателем копии), в том числе для архивного чата и чата без строки в `mirror_chat`.

mod common;

use common::*;
use kinige_domain::groups::INBOX_ID;
use std::fs;
use std::path::{Path, PathBuf};

const DAY: &str = "2026-10-21";

fn day_file(dir: &Path, folder: &str) -> PathBuf {
    mirror_dir(dir)
        .join(folder)
        .join("2026")
        .join(format!("{DAY}.md"))
}

fn text(dir: &Path, folder: &str) -> String {
    fs::read_to_string(day_file(dir, folder)).unwrap_or_default()
}

fn dirty_count(dir: &Path) -> i64 {
    raw(dir)
        .query_row("SELECT COUNT(*) FROM mirror_dirty", [], |r| r.get(0))
        .unwrap()
}

fn known_folder(dir: &Path, chat: &str) -> Option<String> {
    raw(dir)
        .query_row(
            "SELECT folder FROM mirror_chat WHERE chat_id = ?1",
            [chat],
            |r| r.get(0),
        )
        .ok()
}

/// Ждёт, пока все отметки сняты (писатель копии отработал) и день записан или удалён.
fn settled(dir: &Path, what: &str, mut condition: impl FnMut() -> bool) {
    wait_until(what, || dirty_count(dir) == 0 && condition());
}

#[test]
fn the_day_file_moves_between_chat_folders_for_a_chat_without_a_mirror_row() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path());
    let first = store.send_message(INBOX_ID, "первая", &moscow(T0)).unwrap();
    settled(dir.path(), "файл Inbox", || {
        day_file(dir.path(), "Inbox").exists()
    });
    // Новый чат: файлов и строки `mirror_chat` у него ещё нет.
    let books = store.create_chat("Книги", Some("📚"), &moscow(T0)).unwrap();
    assert_eq!(known_folder(dir.path(), &books.id), None);

    store
        .move_message(&first.id, &books.id, &moscow(T0 + MINUTE))
        .unwrap();
    settled(
        dir.path(),
        "файл в «Книгах», Inbox пуст",
        || day_file(dir.path(), "Книги").exists() && !day_file(dir.path(), "Inbox").exists(),
    );
    assert_eq!(
        text(dir.path(), "Книги"),
        "# 2026-10-21\n\n## 09:12\n\nпервая\n"
    );
    assert!(
        !mirror_dir(dir.path()).join("Inbox").exists(),
        "пустая папка Inbox убрана"
    );

    // Вторая запись и обратный путь: день в «Книгах» остаётся с одной записью, в Inbox возвращается первая.
    let second = store
        .send_message(INBOX_ID, "вторая", &moscow(T0 + 2 * MINUTE))
        .unwrap();
    store
        .move_message(&second.id, &books.id, &moscow(T0 + 3 * MINUTE))
        .unwrap();
    settled(
        dir.path(),
        "обе записи в «Книгах»",
        || text(dir.path(), "Книги").contains("вторая") && !day_file(dir.path(), "Inbox").exists(),
    );
    assert_eq!(
        text(dir.path(), "Книги"),
        "# 2026-10-21\n\n## 09:12\n\nпервая\n\n## 09:14\n\nвторая\n"
    );
    store
        .move_message(&first.id, INBOX_ID, &moscow(T0 + 4 * MINUTE))
        .unwrap();
    settled(
        dir.path(),
        "первая вернулась в Inbox",
        || {
            text(dir.path(), "Inbox").contains("первая")
                && !text(dir.path(), "Книги").contains("первая")
        },
    );
    assert_eq!(
        text(dir.path(), "Книги"),
        "# 2026-10-21\n\n## 09:14\n\nвторая\n"
    );
}

#[test]
fn the_copy_follows_a_move_into_an_archived_chat_out_of_it_and_for_a_chat_with_a_recorded_folder() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path());
    let old = store.create_chat("Старое", None, &moscow(T0)).unwrap();
    let own = store
        .send_message(&old.id, "своя запись", &moscow(T0))
        .unwrap();
    let inbox = store
        .send_message(INBOX_ID, "из Inbox", &moscow(T0 + MINUTE))
        .unwrap();
    settled(dir.path(), "файлы двух чатов", || {
        day_file(dir.path(), "Старое").exists() && day_file(dir.path(), "Inbox").exists()
    });
    // Переименование записывает папку в `mirror_chat`: у чата есть строка с прежним именем папки.
    store
        .rename_chat(&old.id, "Прежнее", &moscow(T0 + 2 * MINUTE))
        .unwrap();
    settled(dir.path(), "папка переехала", || {
        day_file(dir.path(), "Прежнее").exists() && !day_file(dir.path(), "Старое").exists()
    });
    assert_eq!(
        known_folder(dir.path(), &old.id).as_deref(),
        Some("Прежнее")
    );
    store
        .set_chat_archived(&old.id, true, &moscow(T0 + 3 * MINUTE))
        .unwrap();

    // В архивный чат с записанной папкой.
    store
        .move_message(&inbox.id, &old.id, &moscow(T0 + 4 * MINUTE))
        .unwrap();
    settled(
        dir.path(),
        "запись в архивном чате",
        || {
            text(dir.path(), "Прежнее").contains("из Inbox")
                && !day_file(dir.path(), "Inbox").exists()
        },
    );
    assert_eq!(
        text(dir.path(), "Прежнее"),
        "# 2026-10-21\n\n## 09:12\n\nсвоя запись\n\n## 09:13\n\nиз Inbox\n"
    );

    // Из архивного чата-источника в Inbox: день в архивной папке остаётся с одной записью, в Inbox появляется файл.
    store
        .move_message(&own.id, INBOX_ID, &moscow(T0 + 5 * MINUTE))
        .unwrap();
    settled(
        dir.path(),
        "запись ушла из архивного чата",
        || {
            text(dir.path(), "Inbox").contains("своя запись")
                && !text(dir.path(), "Прежнее").contains("своя запись")
        },
    );
    assert_eq!(
        text(dir.path(), "Прежнее"),
        "# 2026-10-21\n\n## 09:13\n\nиз Inbox\n"
    );
    assert_eq!(
        text(dir.path(), "Inbox"),
        "# 2026-10-21\n\n## 09:12\n\nсвоя запись\n"
    );
}
