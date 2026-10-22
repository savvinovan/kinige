//! Первая перезапись группы `body`: неизвестные группы и поля дословно, повреждённый JSON, отказ записи, лимит, рост HLC.

mod common;

use common::capture::*;
use common::*;
use kinige_domain::{CaptureAction, ErrorCode};
use kinige_store::StoreError;
use serde_json::{Value, json, value::RawValue};

#[test]
fn supersedes_stays_at_64_through_the_store_and_the_oldest_is_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let first = store.capture_send("a", &moscow(T0)).unwrap().message;
    let h0 = body_hlc(&row(dir.path(), &first.id));
    // Запись с 63 прежними метками (HLC из прошлого, строго меньше текущей): после правки их ровно 64.
    let old: Vec<String> = (0..63)
        .map(|i| format!("{:013}-00000-{}", 1_000 + i, &h0[20..]))
        .collect();
    edit_raw_groups(dir.path(), &first.id, |groups, _| {
        let body = json!({"base": null, "body_md": "a", "edited_at": null, "supersedes": old});
        groups.insert(
            "body".into(),
            RawValue::from_string(
                String::from_utf8(kinige_domain::jcs::canonical_bytes(&body).unwrap()).unwrap(),
            )
            .unwrap(),
        );
    });
    store.capture_append("b", &moscow(T0 + MINUTE)).unwrap();
    let after = row(dir.path(), &first.id);
    let list: Vec<String> = serde_json::from_str(&after.body_supersedes).unwrap();
    assert_eq!(list.len(), 64);
    assert_eq!(list[0], old[0], "пока 64 — ничего не отброшено");
    assert_eq!(list.last(), Some(&h0));

    // Ещё одна правка: 65-я метка вытесняет самую старую, остаётся ровно 64.
    let h1 = body_hlc(&after);
    store.capture_append("c", &moscow(T0 + 2 * MINUTE)).unwrap();
    let again = row(dir.path(), &first.id);
    let list: Vec<String> = serde_json::from_str(&again.body_supersedes).unwrap();
    assert_eq!(list.len(), 64);
    assert_eq!(list[0], old[1], "старейшая отброшена");
    assert_eq!(list.last(), Some(&h1));
    let body: Value = serde_json::from_str(&group(&again, "body")).unwrap();
    assert_eq!(
        body["supersedes"],
        json!(list),
        "колонка и группа совпадают"
    );
}

#[test]
fn unknown_group_unknown_fields_and_extra_hlc_fields_survive_an_append_verbatim() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let first = store.capture_send("база", &moscow(T0)).unwrap().message;
    // Неизвестная группа (необязательная, в поддерживаемой sv) с пробелами в байтах; неизвестное поле в самой body;
    // лишнее поле в записи HLC чужой группы.
    edit_raw_groups(dir.path(), &first.id, |groups, hlcs| {
        groups.insert(
            "future".into(),
            RawValue::from_string(r#"{ "z": [1, 2],  "a": "ё" }"#.into()).unwrap(),
        );
        let h = serde_json::from_str::<Value>(hlcs["body"].get()).unwrap()["h"].clone();
        hlcs.insert(
            "future".into(),
            RawValue::from_string(format!(r#"{{ "h": {h}, "sv": 1, "x": true }}"#)).unwrap(),
        );
        let old: Value = serde_json::from_str(groups["body"].get()).unwrap();
        let mut body = old.as_object().unwrap().clone();
        body.insert("flair".into(), json!("x"));
        groups.insert(
            "body".into(),
            RawValue::from_string(
                String::from_utf8(
                    kinige_domain::jcs::canonical_bytes(&Value::Object(body)).unwrap(),
                )
                .unwrap(),
            )
            .unwrap(),
        );
        hlcs.insert(
            "flags".into(),
            RawValue::from_string(format!(r#"{{"h":{h},"sv":1,"note":"keep"}}"#)).unwrap(),
        );
    });
    let before = row(dir.path(), &first.id);

    let out = store
        .capture_append("дальше", &moscow(T0 + MINUTE))
        .unwrap();
    assert_eq!(out.action, CaptureAction::Appended);
    let after = row(dir.path(), &first.id);

    assert_eq!(group(&after, "future"), r#"{ "z": [1, 2],  "a": "ё" }"#);
    for name in ["core", "flags", "placement", "deleted"] {
        assert_eq!(group(&after, name), group(&before, name), "{name} дословно");
    }
    let body: Value = serde_json::from_str(&group(&after, "body")).unwrap();
    assert_eq!(body["flair"], "x", "неизвестное поле body сохранено");
    assert_eq!(body["body_md"], "база\n\nдальше");
    let hlcs_after: std::collections::BTreeMap<String, Box<RawValue>> =
        serde_json::from_str(&after.hlc_json).unwrap();
    let hlcs_before: std::collections::BTreeMap<String, Box<RawValue>> =
        serde_json::from_str(&before.hlc_json).unwrap();
    assert_eq!(hlcs_after["future"].get(), hlcs_before["future"].get());
    assert_eq!(hlcs_after["flags"].get(), hlcs_before["flags"].get());
    assert_ne!(hlcs_after["body"].get(), hlcs_before["body"].get());
    assert!(body_hlc(&after) > body_hlc(&before));
}

#[test]
fn corrupt_groups_fail_the_append_and_change_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let first = store.capture_send("цела", &moscow(T0)).unwrap().message;
    store
        .set_capture_draft("останется", &moscow(T0 + 1))
        .unwrap();
    for (column, bad) in [
        ("groups_raw", "{не json"),
        ("groups_raw", "{}"),
        ("hlc_json", "{"),
    ] {
        let conn = raw(dir.path());
        let good: String = conn
            .query_row(
                &format!("SELECT {column} FROM message WHERE id = ?1"),
                [&first.id],
                |r| r.get(0),
            )
            .unwrap();
        conn.execute(
            &format!("UPDATE message SET {column} = ?1 WHERE id = ?2"),
            (bad, &first.id),
        )
        .unwrap();
        let clock_before = (state(dir.path(), "hlc_l"), state(dir.path(), "hlc_c"));

        let error = store
            .capture_append("не запишется", &moscow(T0 + MINUTE))
            .unwrap_err();
        // Повреждённая запись — не повод молча создать другую: дописывание отказывает кодом.
        assert!(
            matches!(error, StoreError::Internal(_)),
            "{column} {bad}: {error}"
        );
        assert_eq!(error.code(), ErrorCode::Internal);
        assert_eq!(count(dir.path()), 1);
        assert_eq!(row(dir.path(), &first.id).body_md, "цела");
        assert_eq!(
            (state(dir.path(), "hlc_l"), state(dir.path(), "hlc_c")),
            clock_before,
            "часы HLC откатились"
        );
        assert_eq!(store.get_capture_draft().unwrap(), "останется");

        conn.execute(
            &format!("UPDATE message SET {column} = ?1 WHERE id = ?2"),
            (good, &first.id),
        )
        .unwrap();
    }
    assert_eq!(
        store
            .capture_append("после починки", &moscow(T0 + MINUTE))
            .unwrap()
            .action,
        CaptureAction::Appended
    );
}

#[test]
fn write_failure_during_append_leaves_the_entry_the_clock_and_the_draft_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let first = store
        .capture_send("до отказа", &moscow(T0))
        .unwrap()
        .message;
    store
        .set_capture_draft("черновик жив", &moscow(T0 + 1))
        .unwrap();
    let before = row(dir.path(), &first.id);
    let clock_before = (state(dir.path(), "hlc_l"), state(dir.path(), "hlc_c"));
    let dirty_before: i64 = raw(dir.path())
        .query_row("SELECT SUM(seq) FROM mirror_dirty", [], |r| r.get(0))
        .unwrap();

    // Отказ на последнем шаге транзакции: после UPDATE записи и HLC.
    let conn = raw(dir.path());
    conn.execute_batch(
        "CREATE TRIGGER fail_dirty_update BEFORE UPDATE ON mirror_dirty BEGIN SELECT RAISE(ABORT, 'нет места'); END;",
    )
    .unwrap();
    let error = store
        .capture_append("не запишется", &moscow(T0 + MINUTE))
        .unwrap_err();
    assert_eq!(error.code(), ErrorCode::StoreWriteFailed);

    let after = row(dir.path(), &first.id);
    assert_eq!(after.groups_raw, before.groups_raw);
    assert_eq!(after.hlc_json, before.hlc_json);
    assert_eq!(after.body_md, before.body_md);
    assert_eq!(after.edited_at, None);
    assert_eq!(
        (state(dir.path(), "hlc_l"), state(dir.path(), "hlc_c")),
        clock_before
    );
    assert_eq!(
        conn.query_row::<i64, _, _>("SELECT SUM(seq) FROM mirror_dirty", [], |r| r.get(0))
            .unwrap(),
        dirty_before
    );
    assert_eq!(store.get_capture_draft().unwrap(), "черновик жив");

    conn.execute_batch("DROP TRIGGER fail_dirty_update;")
        .unwrap();
    assert!(
        store
            .capture_append("после отказа", &moscow(T0 + 2 * MINUTE))
            .is_ok()
    );
}

#[test]
fn appending_beyond_the_group_limit_is_rejected_and_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    // 200 Ки знаков «я» — около 400 КиБ группы; второе столько же уже не помещается в 512 КиБ.
    let big = "я".repeat(200 * 1024);
    let first = store.capture_send(&big, &moscow(T0)).unwrap().message;
    let before = row(dir.path(), &first.id);
    store
        .set_capture_draft("черновик остаётся", &moscow(T0))
        .unwrap();
    let error = store
        .capture_append(&"я".repeat(100 * 1024), &moscow(T0 + MINUTE))
        .unwrap_err();
    assert!(matches!(error, StoreError::TooLarge), "{error}");
    assert_eq!(error.code(), ErrorCode::MessageTooLarge);
    assert_eq!(
        count(dir.path()),
        1,
        "новая запись не создана вместо отказа"
    );
    assert_eq!(row(dir.path(), &first.id).groups_raw, before.groups_raw);
    assert_eq!(store.get_capture_draft().unwrap(), "черновик остаётся");
}

#[test]
fn empty_text_is_rejected_by_both_capture_paths() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    store.capture_send("есть", &moscow(T0)).unwrap();
    for text in ["", "  \n\t "] {
        assert!(matches!(
            store.capture_send(text, &moscow(T0 + 1)),
            Err(StoreError::Empty)
        ));
        assert!(matches!(
            store.capture_append(text, &moscow(T0 + 1)),
            Err(StoreError::Empty)
        ));
    }
    assert_eq!(count(dir.path()), 1);
    assert_eq!(
        row(
            dir.path(),
            &store.capture_view(&moscow(T0 + 2)).unwrap().context[0].id
        )
        .body_md,
        "есть"
    );
}

#[test]
fn hlc_labels_keep_growing_across_appends_sends_and_restarts() {
    let dir = tempfile::tempdir().unwrap();
    let mut seen: Vec<String> = Vec::new();
    for round in 0..3 {
        let store = open_without_mirror(dir.path());
        // Часы стоят на месте и идут назад.
        let a = store
            .capture_send(&format!("r{round}"), &moscow(T0 + round * 20 * MINUTE))
            .unwrap();
        seen.push(body_hlc(&row(dir.path(), &a.message.id)));
        let b = store
            .capture_append("доп", &moscow(T0 + round * 20 * MINUTE - 5000))
            .unwrap();
        assert_eq!(b.action, CaptureAction::Appended);
        seen.push(body_hlc(&row(dir.path(), &b.message.id)));
    }
    assert_eq!(seen.len(), 6);
    assert!(seen.windows(2).all(|w| w[0] < w[1]), "{seen:?}");
}
