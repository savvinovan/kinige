//! Хранилище на настоящем SQLite: миграция, Inbox, отправка, порядок и курсор ленты, «Отменить», отказ записи, HLC.

mod common;

use common::*;
use kinige_domain::groups::{INBOX_ID, parse_groups_raw};
use kinige_domain::{ErrorCode, FeedCursor, TextSize};
use kinige_store::{LATEST_VERSION, Now, StoreError};
use serde_json::Value;

#[test]
fn migration_from_scratch_creates_exactly_the_tables_of_the_slice() {
    let dir = tempfile::tempdir().unwrap();
    drop(open_without_mirror(dir.path()));
    let conn = raw(dir.path());
    let mut tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    tables.sort();
    assert_eq!(
        tables,
        [
            "app_setting",
            "chat",
            "message",
            "mirror_dirty",
            "sync_state"
        ]
    );
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, LATEST_VERSION);
    let mode: String = conn
        .pragma_query_value(None, "journal_mode", |r| r.get(0))
        .unwrap();
    assert_eq!(mode, "wal");
}

#[test]
fn message_table_has_all_columns_of_the_protocol() {
    let dir = tempfile::tempdir().unwrap();
    drop(open_without_mirror(dir.path()));
    let conn = raw(dir.path());
    let columns = |table: &str| -> Vec<String> {
        conn.prepare(&format!("SELECT name FROM pragma_table_info('{table}')"))
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };
    let message = columns("message");
    for column in [
        "id",
        "chat_id",
        "body_md",
        "journal_date",
        "created_at",
        "created_tz",
        "edited_at",
        "is_task",
        "task_done_at",
        "favorite_at",
        "pinned_at",
        "reply_to_id",
        "origin",
        "ext_json",
        "deleted_at",
        "body_base",
        "body_supersedes",
        "placement_base",
        "placement_supersedes",
        "hlc_json",
        "server_known",
        "schema_version",
        "groups_raw",
    ] {
        assert!(message.contains(&column.to_string()), "message.{column}");
    }
    let chat = columns("chat");
    for column in [
        "hlc_json",
        "server_known",
        "schema_version",
        "groups_raw",
        "is_inbox",
        "ai_excluded",
    ] {
        assert!(chat.contains(&column.to_string()), "chat.{column}");
    }
}

#[test]
fn inbox_is_created_once_and_survives_restarts() {
    let dir = tempfile::tempdir().unwrap();
    let device_before = {
        let store = open_without_mirror(dir.path());
        let chats = store.list_chats().unwrap();
        assert_eq!(chats.len(), 1);
        assert_eq!(chats[0].id, INBOX_ID);
        assert!(chats[0].is_inbox);
        assert_eq!(
            chats[0].title, "",
            "хранимое имя пустое, интерфейс показывает переведённое"
        );
        assert_eq!(chats[0].count, 0);
        raw(dir.path())
            .query_row(
                "SELECT value FROM sync_state WHERE key = 'device_id'",
                [],
                |r| r.get::<_, String>(0),
            )
            .unwrap()
    };
    for _ in 0..3 {
        let store = open_without_mirror(dir.path());
        assert_eq!(
            store.list_chats().unwrap().len(),
            1,
            "Inbox один после повторных стартов"
        );
    }
    let conn = raw(dir.path());
    let inboxes: i64 = conn
        .query_row("SELECT COUNT(*) FROM chat", [], |r| r.get(0))
        .unwrap();
    assert_eq!(inboxes, 1);
    let device_after: String = conn
        .query_row(
            "SELECT value FROM sync_state WHERE key = 'device_id'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(device_before, device_after, "device_id создаётся один раз");
    assert!(kinige_domain::hlc::is_uuid(&device_after));
}

#[test]
fn inbox_row_carries_all_six_groups_and_constant_core() {
    let dir = tempfile::tempdir().unwrap();
    drop(open_without_mirror(dir.path()));
    let (groups_raw, hlc_json, sv, created_at): (String, String, i64, i64) = raw(dir.path())
        .query_row(
            "SELECT groups_raw, hlc_json, schema_version, created_at FROM chat WHERE id = ?1",
            [INBOX_ID],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    let groups = parse_groups_raw(&groups_raw).unwrap();
    assert_eq!(
        groups.keys().map(String::as_str).collect::<Vec<_>>(),
        ["core", "deleted", "meta", "order", "privacy", "state"]
    );
    assert_eq!(groups["core"].get(), r#"{"created_at":0,"is_inbox":true}"#);
    let hlcs: Value = serde_json::from_str(&hlc_json).unwrap();
    assert_eq!(hlcs.as_object().unwrap().len(), 6);
    assert_eq!((sv, created_at), (1, 0));
}

#[test]
fn sent_message_appears_in_the_feed_with_counter_and_today_label() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let sent = store
        .send_message(
            INBOX_ID,
            "Купить **молоко**\n- [ ] позвонить маме\n\n",
            &moscow(T0),
        )
        .unwrap();
    assert_eq!(sent.body_md, "Купить **молоко**\n- [ ] позвонить маме");
    assert_eq!(sent.journal_date, "2026-10-21");
    assert_eq!(sent.created_at, T0);
    assert_eq!(sent.created_tz, "Europe/Moscow");
    assert!(kinige_domain::hlc::is_uuid(&sent.id));

    let page = store
        .list_messages(INBOX_ID, None, 50, &moscow(T0 + 5 * MINUTE))
        .unwrap();
    assert_eq!(page.messages, vec![sent.clone()]);
    assert_eq!(page.today, "2026-10-21");
    assert_eq!(page.older, None);
    assert_eq!(store.list_chats().unwrap()[0].count, 1);
}

#[test]
fn night_entries_belong_to_the_previous_day() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let sent = store
        .send_message(INBOX_ID, "не спится", &moscow(T_NIGHT))
        .unwrap();
    assert_eq!(
        sent.journal_date, "2026-10-20",
        "00:30 — ещё вчера (граница дня 04:00)"
    );
    let page = store
        .list_messages(INBOX_ID, None, 50, &moscow(T_NIGHT))
        .unwrap();
    assert_eq!(page.today, "2026-10-20");
}

#[test]
fn empty_and_oversized_entries_are_rejected_before_touching_the_database() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    assert!(matches!(
        store.send_message(INBOX_ID, " \n\t", &moscow(T0)),
        Err(StoreError::Empty)
    ));
    let huge = "я".repeat(kinige_domain::groups::MAX_BODY_BYTES);
    assert!(matches!(
        store.send_message(INBOX_ID, &huge, &moscow(T0)),
        Err(StoreError::TooLarge)
    ));
    assert!(matches!(
        store.send_message("нет-такого-чата", "текст", &moscow(T0)),
        Err(StoreError::ChatNotFound)
    ));
    assert_eq!(store.list_chats().unwrap()[0].count, 0);
}

#[test]
fn feed_order_is_journal_date_created_at_id_and_pages_are_keyset() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    // Отправка не по порядку времени: лента всё равно идёт по (journal_date, created_at, id).
    let times = [
        T0 + 3 * MINUTE,
        T0,
        T0 + MINUTE,
        T_NIGHT,
        T0 + 2 * MINUTE,
        T0 - 3 * 24 * 60 * MINUTE,
    ];
    let mut ids = Vec::new();
    for (i, ms) in times.iter().enumerate() {
        ids.push(
            store
                .send_message(INBOX_ID, &format!("запись {i}"), &moscow(*ms))
                .unwrap()
                .id,
        );
    }
    let now = moscow(T0 + 10 * MINUTE);
    let all = store.list_messages(INBOX_ID, None, 50, &now).unwrap();
    let order: Vec<&str> = all.messages.iter().map(|m| m.body_md.as_str()).collect();
    assert_eq!(
        order,
        [
            "запись 5",
            "запись 3",
            "запись 1",
            "запись 2",
            "запись 4",
            "запись 0"
        ][..],
        "по возрастанию"
    );
    assert_eq!(
        all.messages
            .iter()
            .map(|m| m.journal_date.as_str())
            .collect::<Vec<_>>(),
        [
            "2026-10-18",
            "2026-10-20",
            "2026-10-21",
            "2026-10-21",
            "2026-10-21",
            "2026-10-21"
        ]
    );

    // Страницами по два: последняя страница — самые новые, курсор ведёт к более старым, без дублей и пропусков.
    let mut seen = Vec::new();
    let mut cursor: Option<FeedCursor> = None;
    let mut pages = 0;
    loop {
        let page = store
            .list_messages(INBOX_ID, cursor.as_ref(), 2, &now)
            .unwrap();
        assert!(page.messages.len() <= 2);
        seen.splice(0..0, page.messages.iter().map(|m| m.id.clone()));
        pages += 1;
        match page.older {
            Some(next) => cursor = Some(next),
            None => break,
        }
    }
    assert_eq!(pages, 3);
    assert_eq!(
        seen,
        all.messages
            .iter()
            .map(|m| m.id.clone())
            .collect::<Vec<_>>()
    );
    assert_eq!(seen.len(), ids.len());
}

#[test]
fn entries_with_equal_created_at_are_ordered_by_id() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    for i in 0..5 {
        store
            .send_message(INBOX_ID, &format!("одно время {i}"), &moscow(T0))
            .unwrap();
    }
    let page = store
        .list_messages(INBOX_ID, None, 50, &moscow(T0))
        .unwrap();
    let ids: Vec<&str> = page.messages.iter().map(|m| m.id.as_str()).collect();
    let mut sorted = ids.clone();
    sorted.sort();
    assert_eq!(ids, sorted);
    // Курсор по записи с таким же created_at не теряет соседей.
    let second = &page.messages[1];
    let cursor = FeedCursor {
        journal_date: second.journal_date.clone(),
        created_at: second.created_at,
        id: second.id.clone(),
    };
    let older = store
        .list_messages(INBOX_ID, Some(&cursor), 50, &moscow(T0))
        .unwrap();
    assert_eq!(older.messages.len(), 1);
    assert_eq!(older.messages[0].id, page.messages[0].id);
}

#[test]
fn group_bytes_and_hlc_of_a_sent_message_are_complete() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let sent = store.send_message(INBOX_ID, "Привет", &moscow(T0)).unwrap();
    let (groups_raw, hlc_json, sv, server_known, origin): (String, String, i64, i64, String) = raw(dir.path())
        .query_row(
            "SELECT groups_raw, hlc_json, schema_version, server_known, origin FROM message WHERE id = ?1",
            [&sent.id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .unwrap();
    let groups = parse_groups_raw(&groups_raw).unwrap();
    assert_eq!(
        groups.keys().map(String::as_str).collect::<Vec<_>>(),
        ["body", "core", "deleted", "flags", "placement"]
    );
    assert_eq!(
        groups["body"].get(),
        r#"{"base":null,"body_md":"Привет","edited_at":null,"supersedes":[]}"#
    );
    let hlcs: Value = serde_json::from_str(&hlc_json).unwrap();
    for group in ["core", "body", "flags", "placement", "deleted"] {
        assert_eq!(hlcs[group]["sv"], 1, "{group}");
        let hlc = hlcs[group]["h"].as_str().unwrap();
        assert!(kinige_domain::hlc::parse(hlc).is_ok(), "{hlc}");
    }
    assert_eq!((sv, server_known, origin.as_str()), (1, 0, "user"));
}

#[test]
fn undo_moves_the_entry_to_the_trash_and_returns_its_text() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let first = store.send_message(INBOX_ID, "первая", &moscow(T0)).unwrap();
    let second = store
        .send_message(INBOX_ID, "вторая — поправить", &moscow(T0 + 1000))
        .unwrap();
    assert_eq!(store.list_chats().unwrap()[0].count, 2);

    let undone = store.undo_send(&second.id, &moscow(T0 + 2000)).unwrap();
    assert_eq!(undone.body_md, "вторая — поправить");
    let page = store
        .list_messages(INBOX_ID, None, 50, &moscow(T0 + 3000))
        .unwrap();
    assert_eq!(page.messages, vec![first], "запись пропала из ленты");
    assert_eq!(store.list_chats().unwrap()[0].count, 1, "счётчик снова 1");

    // Запись осталась в базе (корзина, слайс 3): группа deleted с новым HLC, остальные группы не тронуты.
    let (deleted_at, groups_raw, hlc_json): (i64, String, String) = raw(dir.path())
        .query_row(
            "SELECT deleted_at, groups_raw, hlc_json FROM message WHERE id = ?1",
            [&second.id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(deleted_at, T0 + 2000);
    let groups = parse_groups_raw(&groups_raw).unwrap();
    assert_eq!(
        groups["deleted"].get(),
        format!(r#"{{"deleted_at":{}}}"#, T0 + 2000)
    );
    assert!(groups["body"].get().contains("вторая — поправить"));
    let hlcs: Value = serde_json::from_str(&hlc_json).unwrap();
    assert!(
        hlcs["deleted"]["h"].as_str().unwrap() > hlcs["body"]["h"].as_str().unwrap(),
        "новый HLC у deleted"
    );

    assert!(
        matches!(
            store.undo_send(&second.id, &moscow(T0 + 4000)),
            Err(StoreError::MessageNotFound)
        ),
        "повторно нельзя"
    );
    assert!(matches!(
        store.undo_send("нет", &moscow(T0)),
        Err(StoreError::MessageNotFound)
    ));
}

#[test]
fn hlc_labels_are_strictly_increasing_across_events_and_restarts() {
    let dir = tempfile::tempdir().unwrap();
    let mut seen: Vec<String> = Vec::new();
    for round in 0..3 {
        let store = open_without_mirror(dir.path());
        for i in 0..4 {
            // Часы стоят на месте и даже идут назад: метки всё равно растут.
            let ms = if i % 2 == 0 { T0 } else { T0 - 5000 };
            let sent = store
                .send_message(INBOX_ID, &format!("раунд {round} запись {i}"), &moscow(ms))
                .unwrap();
            let hlc: String = raw(dir.path())
                .query_row(
                    "SELECT hlc_json FROM message WHERE id = ?1",
                    [&sent.id],
                    |r| r.get::<_, String>(0),
                )
                .map(|json| {
                    serde_json::from_str::<Value>(&json).unwrap()["core"]["h"]
                        .as_str()
                        .unwrap()
                        .to_string()
                })
                .unwrap();
            if let Some(last) = seen.last() {
                assert!(&hlc > last, "{hlc} должна быть больше {last}");
            }
            seen.push(hlc);
        }
    }
    assert_eq!(seen.len(), 12);
}

#[test]
fn write_failure_leaves_nothing_behind_and_returns_a_code() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    store
        .send_message(INBOX_ID, "до отказа", &moscow(T0))
        .unwrap();
    let clock_before = state(dir.path(), "hlc_c");
    let dirty_before: i64 = raw(dir.path())
        .query_row("SELECT COUNT(*) FROM mirror_dirty", [], |r| r.get(0))
        .unwrap();

    // Инъекция отказа: последний шаг транзакции (отметка копии) падает, как при нехватке места.
    let conn = raw(dir.path());
    conn.execute_batch("CREATE TRIGGER fail_dirty BEFORE INSERT ON mirror_dirty BEGIN SELECT RAISE(ABORT, 'нет места'); END;").unwrap();
    conn.execute_batch("CREATE TRIGGER fail_dirty_update BEFORE UPDATE ON mirror_dirty BEGIN SELECT RAISE(ABORT, 'нет места'); END;").unwrap();

    let error = store
        .send_message(INBOX_ID, "не запишется", &moscow(T0 + 1000))
        .unwrap_err();
    assert_eq!(error.code(), ErrorCode::StoreWriteFailed);
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM message", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1, "записи нет");
    assert_eq!(
        state(dir.path(), "hlc_c"),
        clock_before,
        "часы HLC откатились вместе с записью"
    );
    let dirty_after: i64 = conn
        .query_row("SELECT COUNT(*) FROM mirror_dirty", [], |r| r.get(0))
        .unwrap();
    assert_eq!(dirty_after, dirty_before);

    // «Отменить» при отказе тоже ничего не меняет.
    let id: String = conn
        .query_row("SELECT id FROM message", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        store.undo_send(&id, &moscow(T0 + 2000)).unwrap_err().code(),
        ErrorCode::StoreWriteFailed
    );
    let deleted: Option<i64> = conn
        .query_row("SELECT deleted_at FROM message", [], |r| r.get(0))
        .unwrap();
    assert_eq!(deleted, None);

    // Отказ прошёл: запись снова работает.
    conn.execute_batch("DROP TRIGGER fail_dirty; DROP TRIGGER fail_dirty_update;")
        .unwrap();
    assert!(
        store
            .send_message(INBOX_ID, "после отказа", &moscow(T0 + 3000))
            .is_ok()
    );
    assert_eq!(store.list_chats().unwrap()[0].count, 2);
}

fn state(dir: &std::path::Path, key: &str) -> String {
    raw(dir)
        .query_row("SELECT value FROM sync_state WHERE key = ?1", [key], |r| {
            r.get(0)
        })
        .unwrap()
}

#[test]
fn settings_are_stored_and_survive_restart() {
    let dir = tempfile::tempdir().unwrap();
    {
        let store = open_without_mirror(dir.path());
        let settings = store.get_settings().unwrap();
        assert_eq!(settings.text_size, TextSize::Normal);
        assert!(settings.mirror.is_default);
        assert_eq!(
            settings.mirror.folder.as_deref(),
            Some(mirror_dir(dir.path()).to_str().unwrap())
        );
        store.set_text_size(TextSize::Large).unwrap();
    }
    let store = open_without_mirror(dir.path());
    assert_eq!(store.get_settings().unwrap().text_size, TextSize::Large);
}

#[test]
fn a_database_from_a_newer_app_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    drop(open_without_mirror(dir.path()));
    raw(dir.path())
        .pragma_update(None, "user_version", LATEST_VERSION + 1)
        .unwrap();
    let error = kinige_store::Store::open_at(options(dir.path()), &Now::at(T0, "UTC"))
        .err()
        .expect("отказ");
    assert_eq!(error.code(), ErrorCode::Internal);
}
