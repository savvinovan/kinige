//! Чаты (слайс 3): создание со всеми группами, имя, эмодзи, закрепление, архив, порядок разделов, Inbox, записи в чате,
//! папки Markdown-копии и меню значка.

mod common;

use common::ops::*;
use common::*;
use kinige_domain::groups::INBOX_ID;
use kinige_domain::{ChatSummary, ErrorCode};
use kinige_store::{Now, StoreError};
use serde_json::json;
use std::fs;

fn at(minutes: i64) -> Now {
    moscow(T0 + minutes * MINUTE)
}

fn titles(chats: &[ChatSummary]) -> Vec<String> {
    chats.iter().map(|c| c.title.clone()).collect()
}

#[test]
fn a_new_chat_has_all_six_groups_under_one_hlc_and_goes_to_the_end() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let books = store.create_chat("  Книги ", Some("📚"), &at(1)).unwrap();
    let films = store.create_chat("Фильмы", None, &at(2)).unwrap();

    assert_eq!(
        (books.title.as_str(), books.icon.as_deref()),
        ("Книги", Some("📚"))
    );
    assert_eq!(films.icon.as_deref(), Some("💬"), "без эмодзи — 💬");
    assert_eq!(books.count, 0);
    assert!(!books.is_inbox && books.pinned_at.is_none() && books.archived_at.is_none());
    assert_eq!(
        titles(&store.list_chats().unwrap()),
        ["", "Книги", "Фильмы"],
        "Inbox первый, новые — в конец"
    );

    let g = groups(dir.path(), "chat", &books.id);
    assert_eq!(
        g.keys().collect::<Vec<_>>(),
        ["core", "deleted", "meta", "order", "privacy", "state"]
    );
    assert_eq!(
        g["core"],
        format!(r#"{{"created_at":{},"is_inbox":false}}"#, T0 + MINUTE)
    );
    assert_eq!(g["meta"], r#"{"color":null,"icon":"📚","title":"Книги"}"#);
    assert_eq!(g["state"], r#"{"archived_at":null,"pinned_at":null}"#);
    assert_eq!(g["privacy"], r#"{"ai_excluded":false}"#);
    assert_eq!(g["deleted"], r#"{"deleted_at":null}"#);
    let hlcs = hlc_entries(dir.path(), "chat", &books.id);
    let first = group_hlc(dir.path(), "chat", &books.id, "core");
    for entry in hlcs.values() {
        assert_eq!(
            entry,
            &format!(r#"{{"h":"{first}","sv":1}}"#),
            "одно событие — один HLC, sv = 1"
        );
    }
    let sort_key = |id: &str| -> String {
        raw(dir.path())
            .query_row("SELECT sort_key FROM chat WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .unwrap()
    };
    assert!(sort_key(INBOX_ID) < sort_key(&books.id) && sort_key(&books.id) < sort_key(&films.id));
    let (schema, server_known): (i64, i64) = raw(dir.path())
        .query_row(
            "SELECT schema_version, server_known FROM chat WHERE id = ?1",
            [&books.id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!((schema, server_known), (1, 0));
}

#[test]
fn title_and_icon_rules() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    for (title, code) in [
        ("", ErrorCode::ChatTitleEmpty),
        ("   ", ErrorCode::ChatTitleEmpty),
        ("a\nb", ErrorCode::ChatTitleEmpty),
        (&"я".repeat(129), ErrorCode::ChatTitleTooLong),
    ] {
        assert_eq!(
            store.create_chat(title, None, &at(1)).unwrap_err().code(),
            code,
            "{title:?}"
        );
    }
    assert!(store.create_chat(&"я".repeat(128), None, &at(1)).is_ok());
    assert_eq!(
        store
            .create_chat("Х", Some("два слова"), &at(1))
            .unwrap_err()
            .code(),
        ErrorCode::ChatIconInvalid
    );
    assert_eq!(
        count_rows(dir.path(), "chat"),
        2,
        "ничего лишнего не создано"
    );
}

#[test]
fn rename_and_icon_rewrite_only_meta_and_keep_foreign_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let chat = store.create_chat("Книги", Some("📚"), &at(1)).unwrap();
    add_foreign_chat(dir.path(), &chat.id);
    let before = groups(dir.path(), "chat", &chat.id);
    let hlcs_before = hlc_entries(dir.path(), "chat", &chat.id);
    let h_before = group_hlc(dir.path(), "chat", &chat.id, "meta");

    let renamed = store.rename_chat(&chat.id, " Проза ", &at(2)).unwrap();
    assert_eq!(renamed.title, "Проза");
    let after = groups(dir.path(), "chat", &chat.id);
    assert_only_changed(&before, &after, &["meta"]);
    assert_eq!(
        after["meta"],
        r#"{"color":null,"icon":"📚","title":"Проза"}"#
    );
    assert_eq!(after["future"], FUTURE_GROUP);
    let hlcs_after = hlc_entries(dir.path(), "chat", &chat.id);
    for (name, entry) in &hlcs_before {
        if name != "meta" {
            assert_eq!(&hlcs_after[name], entry, "{name}");
        }
    }
    let h_after = group_hlc(dir.path(), "chat", &chat.id, "meta");
    assert!(h_after > h_before);

    let iconed = store.set_chat_icon(&chat.id, "🎬", &at(3)).unwrap();
    assert_eq!(iconed.icon.as_deref(), Some("🎬"));
    assert_eq!(
        groups(dir.path(), "chat", &chat.id)["meta"],
        r#"{"color":null,"icon":"🎬","title":"Проза"}"#
    );
    assert!(group_hlc(dir.path(), "chat", &chat.id, "meta") > h_after);
    let (title, icon): (String, String) = raw(dir.path())
        .query_row(
            "SELECT title, icon FROM chat WHERE id = ?1",
            [&chat.id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(
        (title.as_str(), icon.as_str()),
        ("Проза", "🎬"),
        "колонки из групп"
    );

    // То же имя — ничего не пишется.
    let clock_before = clock(dir.path());
    store.rename_chat(&chat.id, "Проза", &at(4)).unwrap();
    assert_eq!(clock(dir.path()), clock_before);
}

#[test]
fn pin_and_archive_rewrite_only_state_and_form_the_sections() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let a = store.create_chat("A", None, &at(1)).unwrap();
    let b = store.create_chat("B", None, &at(2)).unwrap();
    let c = store.create_chat("C", None, &at(3)).unwrap();
    add_foreign_chat(dir.path(), &a.id);
    let before = groups(dir.path(), "chat", &a.id);

    store.set_chat_pinned(&c.id, true, &at(4)).unwrap();
    store.set_chat_pinned(&b.id, true, &at(5)).unwrap();
    store.set_chat_archived(&a.id, true, &at(6)).unwrap();
    let after = groups(dir.path(), "chat", &a.id);
    assert_only_changed(&before, &after, &["state"]);
    assert_eq!(
        after["state"],
        format!(r#"{{"archived_at":{},"pinned_at":null}}"#, T0 + 6 * MINUTE)
    );
    assert_eq!(after["future"], FUTURE_GROUP);

    let chats = store.list_chats().unwrap();
    assert_eq!(
        titles(&chats),
        ["C", "B", "", "A"],
        "закреплённые по времени закрепления, затем чаты, затем архив"
    );
    assert!(chats[0].pinned_at.is_some() && chats[3].archived_at.is_some());

    // Открепить и вернуть из архива.
    store.set_chat_pinned(&c.id, false, &at(7)).unwrap();
    store.set_chat_archived(&a.id, false, &at(8)).unwrap();
    assert_eq!(titles(&store.list_chats().unwrap()), ["B", "", "A", "C"]);
    // Повтор не пишет.
    let clock_before = clock(dir.path());
    store.set_chat_archived(&a.id, false, &at(9)).unwrap();
    assert_eq!(clock(dir.path()), clock_before);
}

#[test]
fn inbox_can_be_pinned_but_not_renamed_archived_or_given_another_icon() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let books = store.create_chat("Книги", None, &at(1)).unwrap();
    store.set_chat_pinned(INBOX_ID, true, &at(2)).unwrap();
    store.set_chat_pinned(&books.id, true, &at(3)).unwrap();
    assert_eq!(
        titles(&store.list_chats().unwrap()),
        ["", "Книги"],
        "Inbox закреплён первым"
    );
    store.set_chat_pinned(INBOX_ID, false, &at(4)).unwrap();

    let before = (groups(dir.path(), "chat", INBOX_ID), clock(dir.path()));
    for error in [
        store.rename_chat(INBOX_ID, "Другое", &at(5)).unwrap_err(),
        store.set_chat_icon(INBOX_ID, "🎬", &at(5)).unwrap_err(),
        store.set_chat_archived(INBOX_ID, true, &at(5)).unwrap_err(),
    ] {
        assert!(matches!(error, StoreError::InboxProtected));
        assert_eq!(error.code(), ErrorCode::InboxProtected);
    }
    assert_eq!(
        store.set_chat_archived(INBOX_ID, false, &at(5)).unwrap().id,
        INBOX_ID,
        "вернуть из архива, которого нет, можно: ничего не меняется"
    );
    let after = (groups(dir.path(), "chat", INBOX_ID), clock(dir.path()));
    assert_eq!(after.0["meta"], before.0["meta"]);
    assert_eq!(after.0["state"], before.0["state"]);
    assert!(matches!(
        store.rename_chat("нет", "X", &at(5)),
        Err(StoreError::ChatNotFound)
    ));
}

#[test]
fn messages_go_to_the_chosen_chat_and_each_chat_keeps_its_own_count_and_draft() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let books = store.create_chat("Книги", Some("📚"), &at(1)).unwrap();
    store
        .set_chat_draft(&books.id, "черновик книг", &at(2))
        .unwrap();
    store
        .set_chat_draft(INBOX_ID, "черновик inbox", &at(2))
        .unwrap();

    store.send_message(&books.id, "Дюна", &at(3)).unwrap();
    let chats = store.list_chats().unwrap();
    assert_eq!(
        (chats[0].count, chats[1].count),
        (0, 1),
        "у Книг 1, у Inbox прежний"
    );
    assert_eq!(
        store.get_chat_draft(&books.id).unwrap(),
        "",
        "отправка очистила черновик своего чата"
    );
    assert_eq!(store.get_chat_draft(INBOX_ID).unwrap(), "черновик inbox");
    let page = store.list_messages(&books.id, None, 10, &at(4)).unwrap();
    assert_eq!(page.messages.len(), 1);
    assert!(
        store
            .list_messages(INBOX_ID, None, 10, &at(4))
            .unwrap()
            .messages
            .is_empty()
    );
}

#[test]
fn the_tray_menu_lists_up_to_five_recent_chats_by_their_last_entry() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let mut ids = Vec::new();
    for (i, title) in ["Один", "Два", "Три", "Четыре", "Пять", "Шесть"]
        .iter()
        .enumerate()
    {
        let chat = store.create_chat(title, None, &at(i as i64)).unwrap();
        store
            .send_message(&chat.id, "запись", &at(10 + i as i64))
            .unwrap();
        ids.push(chat.id);
    }
    let empty = store.create_chat("Пустой", None, &at(30)).unwrap();
    store.send_message(INBOX_ID, "в inbox", &at(40)).unwrap();
    store.set_chat_archived(&ids[5], true, &at(41)).unwrap();

    let recent = store.recent_chats(5).unwrap();
    assert_eq!(
        recent.iter().map(|c| c.title.as_str()).collect::<Vec<_>>(),
        ["Пять", "Четыре", "Три", "Два", "Один"],
        "по последней записи; без Inbox, архива и чатов без записей"
    );
    assert!(recent.iter().all(|c| c.id != empty.id && c.id != INBOX_ID));
    assert_eq!(recent[0].icon.as_deref(), Some("💬"));

    // Новая запись в старом чате поднимает его.
    store.send_message(&ids[0], "снова", &at(50)).unwrap();
    assert_eq!(store.recent_chats(2).unwrap()[0].title, "Один");
    // Запись в корзине не считается последней.
    let last = store.send_message(&ids[1], "последняя", &at(60)).unwrap();
    store.delete_message(&last.id, &at(61)).unwrap();
    assert_ne!(store.recent_chats(1).unwrap()[0].title, "Два");
}

// ---------- папки Markdown-копии ----------

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
