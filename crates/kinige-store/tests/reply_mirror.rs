//! Markdown-копия и ответ (слайс 5): под заголовком времени ответа стоит строка `↩` со ссылкой на файл дня старой записи.

mod common;

use common::days::at;
use common::*;
use kinige_domain::groups::INBOX_ID;
use std::fs;
use std::path::{Path, PathBuf};

fn day_file(dir: &Path, chat: &str, date: &str) -> PathBuf {
    mirror_dir(dir)
        .join(chat)
        .join(&date[..4])
        .join(format!("{date}.md"))
}

fn read_when(file: &Path, needle: &str) -> String {
    wait_until("файл дня с нужным текстом", || {
        fs::read_to_string(file).is_ok_and(|t| t.contains(needle))
    });
    fs::read_to_string(file).unwrap()
}

#[test]
fn a_reply_has_a_link_line_to_the_day_file_of_the_old_entry() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path());
    let old = store
        .send_message(INBOX_ID, "Старая мысль", &at(2023, 10, 31, 12, 5))
        .unwrap();
    store
        .send_reply(
            INBOX_ID,
            "До сих пор так думаю",
            Some(&old.id),
            &at(2026, 10, 31, 9, 0),
        )
        .unwrap();
    let text = read_when(&day_file(dir.path(), "Inbox", "2026-10-31"), "До сих пор");
    assert_eq!(
        text,
        "# 2026-10-31\n\n## 09:00\n\n↩ [2023-10-31 12:05](<../2023/2023-10-31.md>)\n\nДо сих пор так думаю\n"
    );
    // Ссылка указывает на настоящий файл.
    let target = day_file(dir.path(), "Inbox", "2026-10-31")
        .parent()
        .unwrap()
        .join("../2023/2023-10-31.md");
    wait_until("файл старого дня", || target.is_file());
}

#[test]
fn the_link_line_loses_the_link_when_the_old_entry_is_in_the_trash_or_gone() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path());
    let trashed = store
        .send_message(INBOX_ID, "уйдёт в корзину", &at(2023, 10, 31, 12, 5))
        .unwrap();
    let purged = store
        .send_message(INBOX_ID, "исчезнет", &at(2024, 10, 31, 8, 0))
        .unwrap();
    let kept = store
        .send_message(INBOX_ID, "останется", &at(2025, 10, 31, 8, 30))
        .unwrap();
    for (target, text, minute) in [
        (&trashed.id, "первый", 0),
        (&purged.id, "второй", 1),
        (&kept.id, "третий", 2),
    ] {
        store
            .send_reply(INBOX_ID, text, Some(target), &at(2026, 10, 31, 9, minute))
            .unwrap();
    }
    let now = at(2026, 10, 31, 10, 0);
    store.delete_message(&trashed.id, &now).unwrap();
    store.delete_message(&purged.id, &now).unwrap();
    store.purge_message(&purged.id, &now).unwrap();
    // Ответы пересчитываются при правке своего дня.
    store
        .edit_message(
            &store
                .list_messages(INBOX_ID, None, 50, &now)
                .unwrap()
                .messages
                .last()
                .unwrap()
                .id,
            "третий!",
            &now,
        )
        .unwrap();
    let text = read_when(&day_file(dir.path(), "Inbox", "2026-10-31"), "третий!");
    assert_eq!(
        text,
        "# 2026-10-31\n\n## 09:00\n\n↩ 2023-10-31 12:05\n\nпервый\n\n## 09:01\n\n↩\n\nвторой\n\n## 09:02\n\n↩ [2025-10-31 08:30](<../2025/2025-10-31.md>)\n\nтретий!\n"
    );
}

#[test]
fn a_link_to_an_entry_of_another_chat_goes_through_both_chat_folders() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path());
    let books = store
        .create_chat("Книги (2)", None, &at(2020, 1, 1, 9, 0))
        .unwrap();
    let old = store
        .send_message(&books.id, "Дюна", &at(2023, 3, 14, 8, 0))
        .unwrap();
    let reply = store
        .send_reply(
            &books.id,
            "перечитал",
            Some(&old.id),
            &at(2026, 3, 14, 9, 0),
        )
        .unwrap();
    // Старая запись переехала в Inbox: ссылка ведёт через папки обоих чатов.
    store
        .move_message(&old.id, INBOX_ID, &at(2026, 3, 14, 9, 5))
        .unwrap();
    store
        .edit_message(&reply.id, "перечитал!", &at(2026, 3, 14, 9, 6))
        .unwrap();
    let text = read_when(
        &day_file(dir.path(), "Книги (2)", "2026-03-14"),
        "перечитал!",
    );
    assert!(
        text.contains("↩ [2023-03-14 08:00](<../../Inbox/2023/2023-03-14.md>)"),
        "{text}"
    );
}
