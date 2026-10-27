//! Папки Markdown-копии по чатам (слайс 3): имя папки по названию чата, суффикс при совпадении, переименование и архив.

mod common;

use common::ops::*;
use common::*;
use kinige_domain::groups::INBOX_ID;
use kinige_store::Now;
use serde_json::json;
use std::fs;

fn at(minutes: i64) -> Now {
    moscow(T0 + minutes * MINUTE)
}

fn folder(dir: &std::path::Path, name: &str, date: &str) -> std::path::PathBuf {
    mirror_dir(dir)
        .join(name)
        .join(&date[..4])
        .join(format!("{date}.md"))
}

#[test]
fn the_copy_puts_each_chat_into_a_folder_named_after_it_and_inbox_stays_inbox() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path());
    let books = store
        .create_chat("Книги / проза", Some("📚"), &at(1))
        .unwrap();
    store.send_message(&books.id, "Дюна", &at(2)).unwrap();
    store.send_message(INBOX_ID, "входящее", &at(3)).unwrap();
    wait_until("файл чата", || {
        folder(dir.path(), "Книги _ проза", "2026-10-21").exists()
    });
    wait_until("файл inbox", || {
        folder(dir.path(), "Inbox", "2026-10-21").exists()
    });
    assert!(
        fs::read_to_string(folder(dir.path(), "Книги _ проза", "2026-10-21"))
            .unwrap()
            .contains("Дюна")
    );
}

#[test]
fn chats_with_the_same_name_get_an_id_suffix_and_the_oldest_keeps_the_plain_name() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path());
    let first = store.create_chat("Книги", None, &at(1)).unwrap();
    let second = store.create_chat("книги", None, &at(2)).unwrap();
    store.send_message(&first.id, "первая", &at(3)).unwrap();
    store.send_message(&second.id, "вторая", &at(4)).unwrap();
    let suffixed = format!("книги ({})", &second.id[..8]);
    wait_until("обе папки", || {
        folder(dir.path(), "Книги", "2026-10-21").exists()
            && folder(dir.path(), &suffixed, "2026-10-21").exists()
    });
}

#[test]
fn renaming_a_chat_moves_its_folder_and_rewrites_every_day() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path());
    let chat = store.create_chat("Книги", None, &at(1)).unwrap();
    let yesterday = at(2).ms - 24 * 60 * MINUTE;
    store.send_message(&chat.id, "сегодня", &at(2)).unwrap();
    store
        .send_message(&chat.id, "вчера", &moscow(yesterday))
        .unwrap();
    wait_until("файлы до переименования", || {
        folder(dir.path(), "Книги", "2026-10-21").exists()
            && folder(dir.path(), "Книги", "2026-10-20").exists()
    });

    store.rename_chat(&chat.id, "Проза", &at(3)).unwrap();
    wait_until("папка переехала", || {
        folder(dir.path(), "Проза", "2026-10-21").exists()
            && folder(dir.path(), "Проза", "2026-10-20").exists()
    });
    assert!(
        !mirror_dir(dir.path()).join("Книги").exists(),
        "старой папки нет"
    );
    assert!(
        fs::read_to_string(folder(dir.path(), "Проза", "2026-10-20"))
            .unwrap()
            .contains("вчера")
    );
    wait_until("отметки сняты", || {
        count_rows(dir.path(), "mirror_dirty") == 0
    });
    let known: String = raw(dir.path())
        .query_row(
            "SELECT folder FROM mirror_chat WHERE chat_id = ?1",
            [&chat.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(known, "Проза");

    // Имя только в другом регистре тоже переезжает.
    store.rename_chat(&chat.id, "проза", &at(4)).unwrap();
    wait_until("регистр", || {
        folder(dir.path(), "проза", "2026-10-21").exists()
    });
    let names: Vec<String> = fs::read_dir(mirror_dir(dir.path()))
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    assert!(
        names.iter().filter(|n| n.to_lowercase() == "проза").count() == 1,
        "{names:?}"
    );
}

#[test]
fn renaming_the_oldest_of_two_same_named_chats_hands_the_plain_folder_to_the_other() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path());
    let first = store.create_chat("Книги", None, &at(1)).unwrap();
    let second = store.create_chat("Книги", None, &at(2)).unwrap();
    store.send_message(&first.id, "первая", &at(3)).unwrap();
    store.send_message(&second.id, "вторая", &at(4)).unwrap();
    let suffixed = format!("Книги ({})", &second.id[..8]);
    wait_until("две папки", || {
        folder(dir.path(), &suffixed, "2026-10-21").exists()
    });

    store.rename_chat(&first.id, "Фильмы", &at(5)).unwrap();
    wait_until("вторая заняла простое имя", || {
        folder(dir.path(), "Книги", "2026-10-21").exists()
            && folder(dir.path(), "Фильмы", "2026-10-21").exists()
    });
    assert!(!mirror_dir(dir.path()).join(&suffixed).exists());
    let text = fs::read_to_string(folder(dir.path(), "Книги", "2026-10-21")).unwrap();
    assert!(text.contains("вторая") && !text.contains("первая"));
}

#[test]
fn an_archived_chat_keeps_its_folder_and_a_message_in_the_trash_leaves_the_copy() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path());
    let chat = store.create_chat("Идеи", None, &at(1)).unwrap();
    let sent = store.send_message(&chat.id, "в корзину", &at(2)).unwrap();
    store.send_message(&chat.id, "остаётся", &at(3)).unwrap();
    wait_until("файл", || {
        folder(dir.path(), "Идеи", "2026-10-21").exists()
    });
    store.set_chat_archived(&chat.id, true, &at(4)).unwrap();
    store.delete_message(&sent.id, &at(5)).unwrap();
    wait_until("запись ушла из копии", || {
        fs::read_to_string(folder(dir.path(), "Идеи", "2026-10-21"))
            .is_ok_and(|t| !t.contains("в корзину"))
    });
    store.restore_message(&sent.id, &at(6)).unwrap();
    wait_until("запись вернулась в копию", || {
        fs::read_to_string(folder(dir.path(), "Идеи", "2026-10-21"))
            .is_ok_and(|t| t.contains("в корзину"))
    });
    assert_eq!(columns(dir.path(), &sent.id)["deleted_at"], json!(null));
}
