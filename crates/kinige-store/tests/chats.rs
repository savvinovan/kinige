//! Chats (slice 3): creation with all groups, the name, the emoji, pinning, the archive, the section order, Inbox, entries in a chat,
//! the Markdown copy folders and the icon menu.

// The root of the integration tests is entirely test code: `cfg(test)` gives helpers outside #[test] the same
// clippy.toml relaxations (unwrap/expect/panic/indexing) as the tests in src/.
#![cfg(test)]

mod common;

use common::ops::*;
use common::*;
use kinige_domain::groups::INBOX_ID;
use kinige_domain::{ChatSummary, ErrorCode};
use kinige_store::{Now, StoreError};

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
    assert_eq!(
        films.icon.as_deref(),
        Some("💬"),
        "without an emoji it is 💬"
    );
    assert_eq!(books.count, 0);
    assert!(!books.is_inbox && books.pinned_at.is_none() && books.archived_at.is_none());
    assert_eq!(
        titles(&store.list_chats().unwrap()),
        ["", "Книги", "Фильмы"],
        "Inbox first, new ones at the end"
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
            "one event, one HLC, sv = 1"
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
        "nothing extra is created"
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
        "columns from the groups"
    );

    // The same name: nothing is written.
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
        "pinned ones by pin time, then chats, then the archive"
    );
    assert!(chats[0].pinned_at.is_some() && chats[3].archived_at.is_some());

    // Unpin and restore from the archive.
    store.set_chat_pinned(&c.id, false, &at(7)).unwrap();
    store.set_chat_archived(&a.id, false, &at(8)).unwrap();
    assert_eq!(titles(&store.list_chats().unwrap()), ["B", "", "A", "C"]);
    // A repeat writes nothing.
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
        "Inbox is pinned first"
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
        "restoring from an archive that is not there is allowed: nothing changes"
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
        "the Books chat has 1, Inbox keeps its previous count"
    );
    assert_eq!(
        store.get_chat_draft(&books.id).unwrap(),
        "",
        "sending cleared the draft of its own chat"
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
        "by the last entry; without Inbox, the archive and chats without entries"
    );
    assert!(recent.iter().all(|c| c.id != empty.id && c.id != INBOX_ID));
    assert_eq!(recent[0].icon.as_deref(), Some("💬"));

    // A new entry in an old chat raises it.
    store.send_message(&ids[0], "снова", &at(50)).unwrap();
    assert_eq!(store.recent_chats(2).unwrap()[0].title, "Один");
    // An entry in the trash does not count as the last one.
    let last = store.send_message(&ids[1], "последняя", &at(60)).unwrap();
    store.delete_message(&last.id, &at(61)).unwrap();
    assert_ne!(store.recent_chats(1).unwrap()[0].title, "Два");
}
