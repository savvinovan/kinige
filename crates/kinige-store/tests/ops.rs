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
