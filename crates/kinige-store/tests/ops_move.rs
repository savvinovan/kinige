//! Перенос записи в другой чат (слайс 3, решение приёмки): группа `placement`, день не меняется, ревизии нет, Markdown-копия
//! (оба чата), счётчики, «последние чаты», теги, отказы.

mod common;

use common::ops::*;
use common::*;
use kinige_domain::groups::INBOX_ID;
use kinige_store::{Now, Store, StoreError};
use serde_json::json;
use std::path::Path;

fn at(minutes: i64) -> Now {
    moscow(T0 + minutes * MINUTE)
}

fn setup(dir: &Path) -> (Store, String, String) {
    let store = open_without_mirror(dir);
    let sent = store
        .send_message(INBOX_ID, "Идея #заметка", &at(0))
        .unwrap();
    let books = store.create_chat("Книги", Some("📚"), &at(1)).unwrap();
    (store, sent.id, books.id)
}

fn dirty(dir: &Path) -> Vec<(String, String)> {
    let mut rows: Vec<(String, String)> = raw(dir)
        .prepare("SELECT chat_id, journal_date FROM mirror_dirty")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    rows.sort();
    rows
}

#[test]
fn moving_rewrites_only_placement_keeps_the_day_and_writes_no_revision_in_both_branches() {
    let dir = tempfile::tempdir().unwrap();
    let (store, id, books) = setup(dir.path());
    let h0 = group_hlc(dir.path(), "message", &id, "placement");
    let before = groups(dir.path(), "message", &id);

    let moved = store.move_message(&id, &books, &at(2)).unwrap();

    assert_eq!(moved.chat_id, books);
    assert_eq!(moved.journal_date, "2026-10-21", "день не меняется");
    assert_eq!(moved.created_at, T0, "время создания не меняется");
    let c = columns(dir.path(), &id);
    assert_eq!(c["placement_base"], json!(null));
    assert_eq!(c["placement_supersedes"], json!(format!(r#"["{h0}"]"#)));
    let after = groups(dir.path(), "message", &id);
    assert_eq!(
        after["placement"],
        format!(
            r#"{{"base":null,"chat_id":"{books}","journal_date":"2026-10-21","supersedes":["{h0}"]}}"#
        )
    );
    for name in ["core", "body", "flags", "deleted"] {
        assert_eq!(after[name], before[name], "{name} дословно");
    }
    assert_eq!(revisions(dir.path(), &id), 0, "ревизия `edit` не пишется");
    assert_columns_match_groups(dir.path(), &id);

    // Подтверждённая сервером версия: base = её HLC, supersedes пуст.
    let h1 = group_hlc(dir.path(), "message", &id, "placement");
    raw(dir.path())
        .execute("UPDATE message SET server_known = 1 WHERE id = ?1", [&id])
        .unwrap();
    store.move_message(&id, INBOX_ID, &at(3)).unwrap();
    let c = columns(dir.path(), &id);
    assert_eq!(
        (
            c["placement_base"].clone(),
            c["placement_supersedes"].clone()
        ),
        (json!(h1), json!("[]"))
    );
    assert_eq!(c["chat_id"], json!(INBOX_ID));
}

#[test]
fn counters_the_tray_menu_the_tag_filter_and_both_days_of_the_copy_follow_the_move() {
    let dir = tempfile::tempdir().unwrap();
    let (store, id, books) = setup(dir.path());
    raw(dir.path())
        .execute("DELETE FROM mirror_dirty", [])
        .unwrap();
    assert!(
        store.recent_chats(5).unwrap().is_empty(),
        "в «Книгах» записей ещё нет"
    );

    store.move_message(&id, &books, &at(2)).unwrap();

    let chats = store.list_chats().unwrap();
    let count = |chat: &str| chats.iter().find(|c| c.id == chat).unwrap().count;
    assert_eq!((count(INBOX_ID), count(&books)), (0, 1));
    assert_eq!(store.recent_chats(5).unwrap()[0].id, books);
    let tagged = |chat: &str| {
        store
            .list_messages_tagged(chat, None, 50, Some("заметка"), &at(3))
            .unwrap()
            .messages
            .len()
    };
    assert_eq!(
        (tagged(INBOX_ID), tagged(&books)),
        (0, 1),
        "теги идут за записью"
    );
    assert_eq!(
        dirty(dir.path()),
        {
            let mut expected = vec![
                (INBOX_ID.to_string(), "2026-10-21".to_string()),
                (books.clone(), "2026-10-21".to_string()),
            ];
            expected.sort();
            expected
        },
        "день старого и день нового чата помечены для перезаписи"
    );
}

#[test]
fn the_same_chat_writes_nothing_and_the_target_may_be_archived() {
    let dir = tempfile::tempdir().unwrap();
    let (store, id, books) = setup(dir.path());
    let clock_before = clock(dir.path());
    store.move_message(&id, INBOX_ID, &at(2)).unwrap();
    assert_eq!(
        clock(dir.path()),
        clock_before,
        "тот же чат — ничего не пишется"
    );

    store.set_chat_archived(&books, true, &at(3)).unwrap();
    let moved = store.move_message(&id, &books, &at(4)).unwrap();
    assert_eq!(moved.chat_id, books, "в архивный чат переносить можно");
}

#[test]
fn refusals_leave_the_record_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let (store, id, books) = setup(dir.path());
    let before = columns(dir.path(), &id);

    assert!(matches!(
        store.move_message(&id, "нет-такого-чата", &at(2)),
        Err(StoreError::ChatNotFound)
    ));
    assert!(matches!(
        store.move_message("нет-такой-записи", &books, &at(2)),
        Err(StoreError::MessageNotFound)
    ));
    // Запись в корзине не переносится.
    store.delete_message(&id, &at(3)).unwrap();
    assert!(matches!(
        store.move_message(&id, &books, &at(4)),
        Err(StoreError::MessageNotFound)
    ));
    store.restore_message(&id, &at(5)).unwrap();
    // Запись с группой из будущей версии — только для чтения.
    make_read_only(dir.path(), &id);
    assert!(matches!(
        store.move_message(&id, &books, &at(6)),
        Err(StoreError::ReadOnly)
    ));
    let after = columns(dir.path(), &id);
    assert_eq!(after["chat_id"], before["chat_id"]);
}
