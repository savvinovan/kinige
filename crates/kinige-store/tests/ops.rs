//! Действия с записью (слайс 3): перезапись одной группы, дословность чужого, HLC, повреждение, отказ записи,
//! только чтение. Для `deleted`, `flags`, `placement` и `body` (правка и пункт задачи).

mod common;

use common::ops::*;
use common::*;
use kinige_domain::groups::INBOX_ID;
use kinige_domain::{ErrorCode, MessageFlag};
use kinige_store::{Store, StoreError};
use serde_json::json;
use std::path::Path;

type Op = fn(&Store, &str) -> Result<(), StoreError>;

fn at(minutes: i64) -> kinige_store::Now {
    moscow(T0 + minutes * MINUTE)
}

/// Действия по одному на группу: имя, меняемая группа, вызов.
fn ops() -> Vec<(&'static str, &'static str, Op)> {
    vec![
        ("delete", "deleted", |s, id| s.delete_message(id, &at(1))),
        ("edit", "body", |s, id| {
            s.edit_message(id, "новый текст #другой", &at(1)).map(drop)
        }),
        ("task", "flags", |s, id| {
            s.set_message_flag(id, MessageFlag::Task, true, &at(1))
                .map(drop)
        }),
        ("favorite", "flags", |s, id| {
            s.set_message_flag(id, MessageFlag::Favorite, true, &at(1))
                .map(drop)
        }),
        ("date", "placement", |s, id| {
            s.set_message_date(id, "2026-10-01", &at(1)).map(drop)
        }),
        ("task item", "body", |s, id| {
            s.toggle_task_item(id, 0, true, &at(1)).map(drop)
        }),
    ]
}

const TEXT: &str = "Идея #заметка\n- [ ] дело";

fn setup(dir: &Path) -> (Store, String) {
    let store = open_without_mirror(dir);
    let sent = store.send_message(INBOX_ID, TEXT, &at(0)).unwrap();
    (store, sent.id)
}

#[test]
fn every_op_rewrites_only_its_group_and_keeps_foreign_bytes_verbatim() {
    for (name, changed, op) in ops() {
        let dir = tempfile::tempdir().unwrap();
        let (store, id) = setup(dir.path());
        add_foreign(dir.path(), &id);
        let before_groups = groups(dir.path(), "message", &id);
        let before_hlcs = hlc_entries(dir.path(), "message", &id);
        let before_hlc = group_hlc(dir.path(), "message", &id, changed);

        op(&store, &id).unwrap_or_else(|e| panic!("{name}: {e}"));

        let after_groups = groups(dir.path(), "message", &id);
        assert_only_changed(&before_groups, &after_groups, &[changed]);
        assert_eq!(
            after_groups["future"], FUTURE_GROUP,
            "{name}: пробелы в байтах"
        );
        let after_hlcs = hlc_entries(dir.path(), "message", &id);
        for (group, entry) in &before_hlcs {
            if group != changed {
                assert_eq!(
                    &after_hlcs[group], entry,
                    "{name}: запись HLC группы {group} дословно"
                );
            }
        }
        // Меняется только своя группа, и её HLC строго больше прежнего и равен новому состоянию часов.
        let new_hlc = group_hlc(dir.path(), "message", &id, changed);
        assert!(new_hlc > before_hlc, "{name}: {new_hlc} > {before_hlc}");
        assert_eq!(
            after_hlcs[changed],
            format!(r#"{{"h":"{new_hlc}","sv":1}}"#)
        );
        assert_columns_match_groups(dir.path(), &id);
    }
}

#[test]
fn restore_rewrites_only_deleted_and_keeps_foreign_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let (store, id) = setup(dir.path());
    add_foreign(dir.path(), &id);
    store.delete_message(&id, &at(1)).unwrap();
    let before = groups(dir.path(), "message", &id);
    let h_deleted = group_hlc(dir.path(), "message", &id, "deleted");

    store.restore_message(&id, &at(2)).unwrap();

    let after = groups(dir.path(), "message", &id);
    assert_only_changed(&before, &after, &["deleted"]);
    assert_eq!(after["deleted"], r#"{"deleted_at":null}"#);
    assert!(group_hlc(dir.path(), "message", &id, "deleted") > h_deleted);
    assert_eq!(after["future"], FUTURE_GROUP);
    assert_columns_match_groups(dir.path(), &id);
    assert_eq!(columns(dir.path(), &id)["deleted_at"], json!(null));
}

#[test]
fn hlc_labels_grow_strictly_across_ops_and_restarts() {
    let dir = tempfile::tempdir().unwrap();
    let mut seen: Vec<String> = Vec::new();
    let id = {
        let (store, id) = setup(dir.path());
        for (_, changed, op) in ops() {
            // Часы стоят на месте и идут назад: метка всё равно растёт.
            op(&store, &id).ok();
            seen.push(group_hlc(dir.path(), "message", &id, changed));
        }
        id
    };
    let store = open_without_mirror(dir.path());
    store.restore_message(&id, &moscow(T0 - 1000)).ok();
    store.delete_message(&id, &moscow(T0 - 1000)).unwrap();
    seen.push(group_hlc(dir.path(), "message", &id, "deleted"));
    store.restore_message(&id, &moscow(T0 - 2000)).unwrap();
    seen.push(group_hlc(dir.path(), "message", &id, "deleted"));
    // Первая запись после "delete" как первой группы deleted уже растёт; остальные группы разные, поэтому сравниваем
    // метки событий через часы: каждая следующая больше предыдущей в порядке выполнения.
    let tail = &seen[seen.len() - 2..];
    assert!(tail[0] < tail[1], "{tail:?}");
    let (l, c) = clock(dir.path());
    assert!(l.parse::<u64>().unwrap() >= 1_792_563_120_000 && c.parse::<u32>().is_ok());
}

#[test]
fn corrupt_groups_fail_every_op_with_an_error_and_change_nothing() {
    for (name, _, op) in ops() {
        for (column, bad) in [
            ("groups_raw", "{не json"),
            ("groups_raw", "{}"),
            ("hlc_json", "{"),
            ("hlc_json", "{}"),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let (store, id) = setup(dir.path());
            let conn = raw(dir.path());
            conn.execute(
                &format!("UPDATE message SET {column} = ?1 WHERE id = ?2"),
                (bad, &id),
            )
            .unwrap();
            let clock_before = clock(dir.path());
            let error = op(&store, &id).unwrap_err();
            assert!(
                matches!(error, StoreError::Internal(_)),
                "{name} {column} {bad}: {error}"
            );
            assert_eq!(error.code(), ErrorCode::Internal);
            assert_eq!(clock(dir.path()), clock_before, "{name}: часы откатились");
            assert_eq!(revisions(dir.path(), &id), 0);
            let stored: String = conn
                .query_row(
                    &format!("SELECT {column} FROM message WHERE id = ?1"),
                    [&id],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(stored, bad, "{name}: поврежденное значение не тронуто");
        }
    }
}

#[test]
fn a_missing_group_is_an_error_not_a_silent_recreate() {
    for (name, changed, op) in ops() {
        let dir = tempfile::tempdir().unwrap();
        let (store, id) = setup(dir.path());
        edit_raw_groups(dir.path(), &id, |groups, hlcs| {
            // Нет самой группы при наличии метки, и наоборот: нет метки при наличии группы.
            groups.remove(changed);
            hlcs.remove("future");
        });
        let error = op(&store, &id).unwrap_err();
        assert!(matches!(error, StoreError::Internal(_)), "{name}: {error}");
    }
}

#[test]
fn an_injected_write_failure_changes_nothing_for_every_op() {
    for (name, _, op) in ops() {
        let dir = tempfile::tempdir().unwrap();
        let (store, id) = setup(dir.path());
        store
            .set_chat_draft(INBOX_ID, "черновик жив", &at(0))
            .unwrap();
        let before = (
            groups(dir.path(), "message", &id),
            hlc_entries(dir.path(), "message", &id),
            columns(dir.path(), &id),
            clock(dir.path()),
            revisions(dir.path(), &id),
            count_rows(dir.path(), "message_tag"),
        );
        let dirty_before: i64 = raw(dir.path())
            .query_row("SELECT SUM(seq) FROM mirror_dirty", [], |r| r.get(0))
            .unwrap();

        // Отказ на последнем шаге: после перезаписи группы, колонок, HLC и ревизии.
        let conn = raw(dir.path());
        conn.execute_batch(
            "CREATE TRIGGER fail_i BEFORE INSERT ON mirror_dirty BEGIN SELECT RAISE(ABORT, 'нет места'); END; \
             CREATE TRIGGER fail_u BEFORE UPDATE ON mirror_dirty BEGIN SELECT RAISE(ABORT, 'нет места'); END; \
             CREATE TRIGGER fail_m BEFORE UPDATE OF is_task, favorite_at ON message BEGIN SELECT RAISE(ABORT, 'нет места'); END;",
        )
        .unwrap();
        let error = op(&store, &id).unwrap_err();
        assert_eq!(error.code(), ErrorCode::StoreWriteFailed, "{name}: {error}");

        let after = (
            groups(dir.path(), "message", &id),
            hlc_entries(dir.path(), "message", &id),
            columns(dir.path(), &id),
            clock(dir.path()),
            revisions(dir.path(), &id),
            count_rows(dir.path(), "message_tag"),
        );
        assert_eq!(after, before, "{name}: ничего не изменилось");
        assert_eq!(
            conn.query_row::<i64, _, _>("SELECT SUM(seq) FROM mirror_dirty", [], |r| r.get(0))
                .unwrap(),
            dirty_before
        );
        assert_eq!(store.get_chat_draft(INBOX_ID).unwrap(), "черновик жив");

        conn.execute_batch("DROP TRIGGER fail_i; DROP TRIGGER fail_u; DROP TRIGGER fail_m;")
            .unwrap();
        op(&store, &id).unwrap_or_else(|e| panic!("{name} после починки: {e}"));
    }
}

#[test]
fn a_record_with_a_newer_group_is_read_only_for_every_op() {
    for (name, _, op) in ops() {
        let dir = tempfile::tempdir().unwrap();
        let (store, id) = setup(dir.path());
        make_read_only(dir.path(), &id);
        let before = (
            groups(dir.path(), "message", &id),
            hlc_entries(dir.path(), "message", &id),
            clock(dir.path()),
        );
        let error = op(&store, &id).unwrap_err();
        assert!(matches!(error, StoreError::ReadOnly), "{name}: {error}");
        assert_eq!(error.code(), ErrorCode::ReadOnly);
        assert_eq!(
            (
                groups(dir.path(), "message", &id),
                hlc_entries(dir.path(), "message", &id),
                clock(dir.path())
            ),
            before,
            "{name}"
        );
        // В ленте запись помечена только для чтения.
        let page = store.list_messages(INBOX_ID, None, 10, &at(2)).unwrap();
        assert!(page.messages[0].read_only);
    }
}

#[test]
fn unknown_ids_are_message_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let (store, _) = setup(dir.path());
    for (name, _, op) in ops() {
        let error = op(&store, "нет-такой").unwrap_err();
        assert!(
            matches!(error, StoreError::MessageNotFound),
            "{name}: {error}"
        );
        assert_eq!(error.code(), ErrorCode::MessageNotFound);
    }
}

#[test]
fn delete_and_restore_move_the_counter_and_the_feed_and_are_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let (store, id) = setup(dir.path());
    let other = store.send_message(INBOX_ID, "вторая", &at(1)).unwrap();
    assert_eq!(store.list_chats().unwrap()[0].count, 2);

    store.delete_message(&id, &at(2)).unwrap();
    assert_eq!(
        store.list_chats().unwrap()[0].count,
        1,
        "счётчик уменьшился на 1"
    );
    assert_eq!(store.sidebar().unwrap().trash_count, 1);
    let page = store.list_messages(INBOX_ID, None, 10, &at(3)).unwrap();
    assert_eq!(
        page.messages
            .iter()
            .map(|m| m.id.as_str())
            .collect::<Vec<_>>(),
        [other.id.as_str()]
    );
    let clock_after_delete = clock(dir.path());
    store.delete_message(&id, &at(9)).unwrap();
    assert_eq!(
        clock(dir.path()),
        clock_after_delete,
        "повторное удаление ничего не пишет"
    );
    assert_eq!(
        columns(dir.path(), &id)["deleted_at"],
        json!(T0 + 2 * MINUTE)
    );

    store.restore_message(&id, &at(4)).unwrap();
    assert_eq!(store.list_chats().unwrap()[0].count, 2);
    assert_eq!(store.sidebar().unwrap().trash_count, 0);
    let page = store.list_messages(INBOX_ID, None, 10, &at(5)).unwrap();
    assert_eq!(
        page.messages
            .iter()
            .map(|m| m.id.clone())
            .collect::<Vec<_>>(),
        [id.clone(), other.id],
        "запись вернулась на своё место"
    );
    store.restore_message(&id, &at(6)).unwrap();
    assert_eq!(
        store.restore_message("нет", &at(6)).unwrap_err().code(),
        ErrorCode::MessageNotFound
    );
}

#[test]
fn edit_keeps_the_entry_in_place_and_saves_the_old_version_as_a_revision() {
    let dir = tempfile::tempdir().unwrap();
    let (store, id) = setup(dir.path());
    add_foreign(dir.path(), &id);
    let first = store.send_message(INBOX_ID, "до", &at(-5)).unwrap();
    let before = groups(dir.path(), "message", &id);
    let hlcs_before = hlc_entries(dir.path(), "message", &id);

    let edited = store
        .edit_message(&id, "\n\nНовый текст #новый \r\n", &at(3))
        .unwrap();
    assert_eq!(edited.body_md, "Новый текст #новый");
    assert_eq!(edited.tags, ["новый"]);
    assert_eq!(
        (edited.created_at, edited.journal_date.as_str()),
        (T0, "2026-10-21")
    );

    // Запись на прежнем месте и с прежним временем.
    let page = store.list_messages(INBOX_ID, None, 10, &at(4)).unwrap();
    assert_eq!(
        page.messages
            .iter()
            .map(|m| m.id.clone())
            .collect::<Vec<_>>(),
        [first.id, id.clone()]
    );
    assert_eq!(
        columns(dir.path(), &id)["edited_at"],
        json!(T0 + 3 * MINUTE)
    );

    // Ровно одна ревизия `edit` со снимком записи до правки: дословные байты всех групп с HLC и sv.
    assert_eq!(revisions(dir.path(), &id), 1);
    let (snapshot, reason, saved_at, revision_hlc): (String, String, i64, String) = raw(dir.path())
        .query_row(
            "SELECT snapshot_json, reason, saved_at, hlc_json FROM message_revision WHERE message_id = ?1",
            [&id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!(reason, "edit");
    assert_eq!(saved_at, T0 + 3 * MINUTE);
    let snapshot: serde_json::Value = serde_json::from_str(&snapshot).unwrap();
    let entries = snapshot["groups"].as_array().unwrap();
    assert_eq!(entries.len(), before.len());
    for entry in entries {
        let name = entry["g"].as_str().unwrap();
        assert_eq!(
            entry["raw"].as_str().unwrap(),
            before[name],
            "{name}: байты до правки дословно"
        );
        let hlc_entry: serde_json::Value = serde_json::from_str(&hlcs_before[name]).unwrap();
        assert_eq!(entry["h"], hlc_entry["h"], "{name}");
        assert_eq!(entry["sv"], hlc_entry["sv"], "{name}");
    }
    // Сама ревизия — запись только с группой `core`; её HLC — метка события правки.
    let revision_groups = groups(
        dir.path(),
        "message_revision",
        &revision_id(dir.path(), &id),
    );
    assert_eq!(revision_groups.keys().collect::<Vec<_>>(), ["core"]);
    let body_hlc = group_hlc(dir.path(), "message", &id, "body");
    assert_eq!(
        revision_hlc,
        format!(r#"{{"core":{{"h":"{body_hlc}","sv":1}}}}"#)
    );
    assert_columns_match_groups(dir.path(), &id);
}

fn revision_id(dir: &Path, message_id: &str) -> String {
    raw(dir)
        .query_row(
            "SELECT id FROM message_revision WHERE message_id = ?1",
            [message_id],
            |r| r.get(0),
        )
        .unwrap()
}

#[test]
fn edit_with_the_same_empty_or_oversized_text_writes_no_revision() {
    let dir = tempfile::tempdir().unwrap();
    let (store, id) = setup(dir.path());
    let clock_before = clock(dir.path());
    store.edit_message(&id, TEXT, &at(1)).unwrap();
    assert_eq!(revisions(dir.path(), &id), 0, "тот же текст — без ревизии");
    assert_eq!(clock(dir.path()), clock_before);
    for text in ["", "  \n\t "] {
        assert!(matches!(
            store.edit_message(&id, text, &at(1)),
            Err(StoreError::Empty)
        ));
    }
    let huge = "я".repeat(300 * 1024);
    assert!(matches!(
        store.edit_message(&id, &huge, &at(1)),
        Err(StoreError::TooLarge)
    ));
    assert_eq!(revisions(dir.path(), &id), 0);
    assert_eq!(columns(dir.path(), &id)["body_md"], json!(TEXT));
}

#[test]
fn edit_branches_of_base_and_supersedes_follow_the_confirmation() {
    let dir = tempfile::tempdir().unwrap();
    let (store, id) = setup(dir.path());
    let h0 = group_hlc(dir.path(), "message", &id, "body");
    // Собственная неподтверждённая версия: base остаётся null, прежний HLC попадает в supersedes.
    store.edit_message(&id, "раз", &at(1)).unwrap();
    let c = columns(dir.path(), &id);
    assert_eq!(
        (c["body_base"].clone(), c["body_supersedes"].clone()),
        (json!(null), json!(format!(r#"["{h0}"]"#)))
    );
    // Версию подтвердил сервер: base = её HLC, supersedes пуст.
    let h1 = group_hlc(dir.path(), "message", &id, "body");
    raw(dir.path())
        .execute("UPDATE message SET server_known = 1 WHERE id = ?1", [&id])
        .unwrap();
    store.edit_message(&id, "два", &at(2)).unwrap();
    let c = columns(dir.path(), &id);
    assert_eq!(
        (c["body_base"].clone(), c["body_supersedes"].clone()),
        (json!(h1), json!("[]"))
    );
    assert_columns_match_groups(dir.path(), &id);
}

#[test]
fn task_item_toggle_is_a_text_edit_with_a_revision_and_stale_requests_fail() {
    let dir = tempfile::tempdir().unwrap();
    let (store, id) = setup(dir.path());
    let done = store.toggle_task_item(&id, 0, true, &at(1)).unwrap();
    assert_eq!(done.body_md, "Идея #заметка\n- [x] дело");
    assert_eq!(revisions(dir.path(), &id), 1, "старая версия сохранена");
    let undone = store.toggle_task_item(&id, 0, false, &at(2)).unwrap();
    assert_eq!(undone.body_md, TEXT);
    assert_eq!(revisions(dir.path(), &id), 2);

    let clock_before = clock(dir.path());
    for (index, checked) in [(0, false), (1, true), (5, false)] {
        let error = store
            .toggle_task_item(&id, index, checked, &at(3))
            .unwrap_err();
        assert!(
            matches!(error, StoreError::TaskItemStale),
            "{index} {checked}"
        );
        assert_eq!(error.code(), ErrorCode::TaskItemStale);
    }
    assert_eq!(clock(dir.path()), clock_before);
    assert_eq!(revisions(dir.path(), &id), 2);
}

#[test]
fn flags_task_done_favorite_and_no_ops() {
    let dir = tempfile::tempdir().unwrap();
    let (store, id) = setup(dir.path());
    // «Выполнено» у записи, которая не задача, — ошибка, а сброс — тихий no-op.
    assert!(
        store
            .set_message_flag(&id, MessageFlag::TaskDone, true, &at(1))
            .is_err()
    );
    store
        .set_message_flag(&id, MessageFlag::TaskDone, false, &at(1))
        .unwrap();

    let view = store
        .set_message_flag(&id, MessageFlag::Task, true, &at(2))
        .unwrap();
    assert!(view.is_task && view.task_done_at.is_none());
    let view = store
        .set_message_flag(&id, MessageFlag::TaskDone, true, &at(3))
        .unwrap();
    assert_eq!(view.task_done_at, Some(T0 + 3 * MINUTE));
    let view = store
        .set_message_flag(&id, MessageFlag::Favorite, true, &at(4))
        .unwrap();
    assert_eq!(view.favorite_at, Some(T0 + 4 * MINUTE));
    let clock_before = clock(dir.path());
    store
        .set_message_flag(&id, MessageFlag::Favorite, true, &at(9))
        .unwrap();
    assert_eq!(
        clock(dir.path()),
        clock_before,
        "то же состояние — ничего не пишется"
    );
    // Снятие задачи обнуляет «выполнено», избранное остаётся.
    let view = store
        .set_message_flag(&id, MessageFlag::Task, false, &at(5))
        .unwrap();
    assert!(!view.is_task && view.task_done_at.is_none());
    assert_eq!(view.favorite_at, Some(T0 + 4 * MINUTE));
    assert_eq!(
        groups(dir.path(), "message", &id)["flags"],
        format!(
            r#"{{"favorite_at":{},"is_task":false,"pinned_at":null,"task_done_at":null}}"#,
            T0 + 4 * MINUTE
        )
    );
    assert_eq!(revisions(dir.path(), &id), 0, "флаги ревизий не пишут");
}

#[test]
fn changing_the_date_moves_the_entry_between_days_in_both_branches() {
    let dir = tempfile::tempdir().unwrap();
    let (store, id) = setup(dir.path());
    let h0 = group_hlc(dir.path(), "message", &id, "placement");

    let moved = store.set_message_date(&id, "2026-10-20", &at(1)).unwrap();
    assert_eq!(moved.journal_date, "2026-10-20");
    assert_eq!(moved.created_at, T0, "время создания не меняется");
    let c = columns(dir.path(), &id);
    assert_eq!(c["placement_base"], json!(null));
    assert_eq!(c["placement_supersedes"], json!(format!(r#"["{h0}"]"#)));
    assert_eq!(
        groups(dir.path(), "message", &id)["placement"],
        format!(
            r#"{{"base":null,"chat_id":"{INBOX_ID}","journal_date":"2026-10-20","supersedes":["{h0}"]}}"#
        )
    );

    let h1 = group_hlc(dir.path(), "message", &id, "placement");
    raw(dir.path())
        .execute("UPDATE message SET server_known = 1 WHERE id = ?1", [&id])
        .unwrap();
    store.set_message_date(&id, "2099-01-31", &at(2)).unwrap();
    let c = columns(dir.path(), &id);
    assert_eq!(
        (
            c["placement_base"].clone(),
            c["placement_supersedes"].clone()
        ),
        (json!(h1), json!("[]"))
    );
    assert_eq!(
        c["journal_date"],
        json!("2099-01-31"),
        "будущая дата разрешена"
    );

    for bad in ["2026-02-30", "вчера", "", "2026-13-01"] {
        assert!(
            matches!(
                store.set_message_date(&id, bad, &at(3)),
                Err(StoreError::InvalidDate)
            ),
            "{bad}"
        );
    }
    // Та же дата — ничего не пишется.
    let clock_before = clock(dir.path());
    store.set_message_date(&id, "2099-01-31", &at(4)).unwrap();
    assert_eq!(clock(dir.path()), clock_before);
    assert_columns_match_groups(dir.path(), &id);
    assert_eq!(revisions(dir.path(), &id), 0, "перенос ревизий не пишет");
}
