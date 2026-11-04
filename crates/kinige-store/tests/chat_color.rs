//! Chat colour (KINIG-59): an explicit pick is written to `meta.color` and the `chat.color` column; chats never picked store
//! `null` and show the automatic colour; Inbox is always grey; the colour reaches the sidebar, search hits and trash plates.

// The root of the integration tests is entirely test code: `cfg(test)` gives helpers outside #[test] the same
// clippy.toml relaxations (unwrap/expect/panic/indexing) as the tests in src/.
#![cfg(test)]

mod common;

use common::import::*;
use common::ops::*;
use common::*;
use kinige_domain::chat_color::automatic_color;
use kinige_domain::groups::INBOX_ID;
use kinige_domain::{ChatColor, SearchFilters};
use kinige_store::{Now, StoreError};
use serde_json::{Value, json};

fn at(minutes: i64) -> Now {
    moscow(T0 + minutes * MINUTE)
}

fn stored_column(dir: &std::path::Path, id: &str) -> Option<String> {
    raw(dir)
        .query_row("SELECT color FROM chat WHERE id = ?1", [id], |r| r.get(0))
        .unwrap()
}

/// The chat's `meta` stamp is made unsupported (`sv = 2`), as if a newer version had written it.
fn make_chat_read_only(dir: &std::path::Path, id: &str) {
    let conn = raw(dir);
    let hlc_json: String = conn
        .query_row("SELECT hlc_json FROM chat WHERE id = ?1", [id], |r| {
            r.get(0)
        })
        .unwrap();
    let mut hlcs: Value = serde_json::from_str(&hlc_json).unwrap();
    hlcs["meta"]["sv"] = json!(2);
    conn.execute(
        "UPDATE chat SET hlc_json = ?1 WHERE id = ?2",
        (hlcs.to_string(), id),
    )
    .unwrap();
}

#[test]
fn a_new_chat_stores_null_and_shows_the_automatic_colour() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let chat = store.create_chat("Проекты", None, &at(1)).unwrap();
    assert_eq!(chat.color, automatic_color(&chat.id));
    assert_ne!(chat.color, ChatColor::Grey);
    assert_eq!(
        groups(dir.path(), "chat", &chat.id)["meta"],
        r#"{"color":null,"icon":"💬","title":"Проекты"}"#,
        "creation bytes are unchanged"
    );
    assert_eq!(stored_column(dir.path(), &chat.id), None);
    let inbox = store.list_chats().unwrap().remove(0);
    assert_eq!(
        (inbox.id.as_str(), inbox.color),
        (INBOX_ID, ChatColor::Grey)
    );
}

#[test]
fn a_pick_rewrites_meta_and_the_column_and_keeps_the_rest() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let chat = store.create_chat("Проекты", Some("📚"), &at(1)).unwrap();
    add_foreign_chat(dir.path(), &chat.id);
    let before = groups(dir.path(), "chat", &chat.id);
    let hlcs_before = hlc_entries(dir.path(), "chat", &chat.id);
    let h_before = group_hlc(dir.path(), "chat", &chat.id, "meta");

    let picked = store
        .set_chat_color(&chat.id, ChatColor::Grey, &at(2))
        .unwrap();
    assert_eq!(picked.color, ChatColor::Grey);
    let after = groups(dir.path(), "chat", &chat.id);
    assert_only_changed(&before, &after, &["meta"]);
    assert_eq!(
        after["meta"],
        r#"{"color":"grey","icon":"📚","title":"Проекты"}"#
    );
    assert_eq!(after["future"], FUTURE_GROUP);
    for (name, entry) in &hlcs_before {
        if name != "meta" {
            assert_eq!(&hlc_entries(dir.path(), "chat", &chat.id)[name], entry);
        }
    }
    assert!(group_hlc(dir.path(), "chat", &chat.id, "meta") > h_before);
    assert_eq!(stored_column(dir.path(), &chat.id).as_deref(), Some("grey"));
    assert_eq!(
        store
            .list_chats()
            .unwrap()
            .iter()
            .find(|c| c.id == chat.id)
            .map(|c| c.color),
        Some(ChatColor::Grey)
    );
    assert_columns_match_chat(dir.path(), &chat.id);

    // A later rename and emoji change keep the pick.
    store.rename_chat(&chat.id, "Работа", &at(3)).unwrap();
    store.set_chat_icon(&chat.id, "🎬", &at(4)).unwrap();
    assert_eq!(
        groups(dir.path(), "chat", &chat.id)["meta"],
        r#"{"color":"grey","icon":"🎬","title":"Работа"}"#
    );
}

fn assert_columns_match_chat(dir: &std::path::Path, id: &str) {
    let (color, groups_raw): (Option<String>, String) = raw(dir)
        .query_row(
            "SELECT color, groups_raw FROM chat WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    let parsed = kinige_domain::chat::chat_columns(&groups_raw).unwrap();
    assert_eq!(color, parsed.color);
}

#[test]
fn the_same_stored_value_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let chat = store.create_chat("Проекты", None, &at(1)).unwrap();
    store
        .set_chat_color(&chat.id, ChatColor::Violet, &at(2))
        .unwrap();
    let bytes = groups(dir.path(), "chat", &chat.id);
    let clock_before = clock(dir.path());
    let h_before = group_hlc(dir.path(), "chat", &chat.id, "meta");
    let again = store
        .set_chat_color(&chat.id, ChatColor::Violet, &at(3))
        .unwrap();
    assert_eq!(again.color, ChatColor::Violet);
    assert_eq!(groups(dir.path(), "chat", &chat.id), bytes);
    assert_eq!(group_hlc(dir.path(), "chat", &chat.id, "meta"), h_before);
    assert_eq!(clock(dir.path()), clock_before);
}

#[test]
fn picking_the_colour_that_is_already_shown_still_stores_it() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let chat = store.create_chat("Проекты", None, &at(1)).unwrap();
    store.set_chat_color(&chat.id, chat.color, &at(2)).unwrap();
    assert_eq!(
        stored_column(dir.path(), &chat.id).as_deref(),
        Some(chat.color.as_str()),
        "an explicit pick is stored even when it equals the automatic colour"
    );
}

#[test]
fn inbox_missing_and_read_only_chats_write_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let chat = store.create_chat("Проекты", None, &at(1)).unwrap();
    make_chat_read_only(dir.path(), &chat.id);
    let all_chats = |dir: &std::path::Path| -> Vec<(String, String, String)> {
        let conn = raw(dir);
        let mut statement = conn
            .prepare("SELECT id, groups_raw, hlc_json FROM chat ORDER BY id")
            .unwrap();
        statement
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };
    let before = (all_chats(dir.path()), clock(dir.path()));

    assert!(matches!(
        store.set_chat_color(INBOX_ID, ChatColor::Pink, &at(2)),
        Err(StoreError::InboxProtected)
    ));
    assert!(matches!(
        store.set_chat_color(
            "00000000-0000-4000-8000-000000000000",
            ChatColor::Pink,
            &at(2)
        ),
        Err(StoreError::ChatNotFound)
    ));
    assert!(matches!(
        store.set_chat_color("не-идентификатор", ChatColor::Pink, &at(2)),
        Err(StoreError::ChatNotFound)
    ));
    assert!(matches!(
        store.set_chat_color(&chat.id, ChatColor::Pink, &at(2)),
        Err(StoreError::ReadOnly)
    ));
    assert_eq!((all_chats(dir.path()), clock(dir.path())), before);
}

#[test]
fn an_unknown_stored_value_shows_the_automatic_colour_and_survives_other_edits() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let chat = store.create_chat("Проекты", None, &at(1)).unwrap();
    // Another client wrote a colour this version does not know.
    let conn = raw(dir.path());
    let groups_raw: String = conn
        .query_row(
            "SELECT groups_raw FROM chat WHERE id = ?1",
            [&chat.id],
            |r| r.get(0),
        )
        .unwrap();
    let foreign = groups_raw.replace(r#""color":null"#, r##""color":"#ff00aa""##);
    assert_ne!(foreign, groups_raw);
    conn.execute(
        "UPDATE chat SET groups_raw = ?1, color = '#ff00aa' WHERE id = ?2",
        (foreign, &chat.id),
    )
    .unwrap();
    drop(conn);

    let shown = store
        .list_chats()
        .unwrap()
        .into_iter()
        .find(|c| c.id == chat.id)
        .unwrap();
    assert_eq!(shown.color, automatic_color(&chat.id));

    store.rename_chat(&chat.id, "Работа", &at(2)).unwrap();
    assert_eq!(
        groups(dir.path(), "chat", &chat.id)["meta"],
        r##"{"color":"#ff00aa","icon":"💬","title":"Работа"}"##,
        "the foreign value stays verbatim"
    );
}

#[test]
fn search_hits_trash_plates_and_chat_rows_carry_the_resolved_colour() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let chat = store.create_chat("Проекты", None, &at(1)).unwrap();
    store
        .set_chat_color(&chat.id, ChatColor::Grey, &at(2))
        .unwrap();
    let kept = store
        .send_message(&chat.id, "уникальноеслово", &at(3))
        .unwrap();
    let inbox = store
        .send_message(INBOX_ID, "уникальноеслово тоже", &at(4))
        .unwrap();
    let trashed = store.send_message(&chat.id, "в корзину", &at(5)).unwrap();
    store.delete_message(&trashed.id, &at(6)).unwrap();

    let page = store
        .search("уникальноеслово", &SearchFilters::default(), None, 50)
        .unwrap();
    let colour_of = |id: &str| {
        page.hits
            .iter()
            .find(|hit| hit.message.id == id)
            .map(|hit| hit.chat_color)
    };
    assert_eq!(colour_of(&kept.id), Some(ChatColor::Grey));
    assert_eq!(colour_of(&inbox.id), Some(ChatColor::Grey), "Inbox is grey");

    let by_name = store
        .search("Проекты", &SearchFilters::default(), None, 50)
        .unwrap();
    assert_eq!(by_name.chats.len(), 1);
    assert_eq!(by_name.chats[0].color, ChatColor::Grey);

    let trash = store.list_trash(&at(7)).unwrap();
    assert_eq!(trash.len(), 1);
    assert_eq!(trash[0].chat_color, ChatColor::Grey);
}

#[test]
fn an_imported_chat_gets_null_and_an_automatic_colour_that_survives_a_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let theirs = format!("user{PETER}");
    let messages = vec![msg(1, SAVED_T, "привет", &theirs)];
    let personal = single("personal_chat", PETER, Some("Пётр Тестов"), &messages);
    let export = parse(&full(&[saved_n(1), personal]));
    let id = {
        let store = open_without_mirror(dir.path());
        run(&store, &export, &[]).unwrap();
        let chats = store.list_chats().unwrap();
        let chat = chats.iter().find(|c| !c.is_inbox).unwrap();
        assert_eq!(chat.color, automatic_color(&chat.id));
        chat.id.clone()
    };
    assert_eq!(stored_column(dir.path(), &id), None);
    assert!(
        groups(dir.path(), "chat", &id)["meta"].contains(r#""color":null"#),
        "an import writes null as a new chat does"
    );
    let reopened = open_without_mirror(dir.path());
    let chat = reopened
        .list_chats()
        .unwrap()
        .into_iter()
        .find(|c| c.id == id)
        .unwrap();
    assert_eq!(chat.color, automatic_color(&id), "stable across restarts");
}
