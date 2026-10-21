//! Быстрый захват на настоящем SQLite (слайс 2): контекст окна, новая запись из окна, дописывание к последней записи
//! (первая перезапись группы `body`), черновики, миграция 1 → 2. Тесты, отложенные из слайса 1: неизвестная группа и
//! лишние поля дословно, `base`/`supersedes`, откат при отказе записи.

mod common;

use common::*;
use kinige_domain::groups::{INBOX_ID, parse_groups_raw};
use kinige_domain::hlc::parse as parse_hlc;
use kinige_domain::{CaptureAction, ErrorCode, ShortcutKind};
use kinige_store::{LATEST_VERSION, Store, StoreError};
use serde_json::{Value, json, value::RawValue};
use std::path::Path;
use std::process::Command;

const MIN10: i64 = 10 * MINUTE;

struct Row {
    body_md: String,
    edited_at: Option<i64>,
    created_at: i64,
    body_base: Option<String>,
    body_supersedes: String,
    hlc_json: String,
    groups_raw: String,
    schema_version: i64,
    origin: String,
    server_known: i64,
}

fn row(dir: &Path, id: &str) -> Row {
    raw(dir)
        .query_row(
            "SELECT body_md, edited_at, created_at, body_base, body_supersedes, hlc_json, groups_raw, schema_version, \
             origin, server_known FROM message WHERE id = ?1",
            [id],
            |r| {
                Ok(Row {
                    body_md: r.get(0)?,
                    edited_at: r.get(1)?,
                    created_at: r.get(2)?,
                    body_base: r.get(3)?,
                    body_supersedes: r.get(4)?,
                    hlc_json: r.get(5)?,
                    groups_raw: r.get(6)?,
                    schema_version: r.get(7)?,
                    origin: r.get(8)?,
                    server_known: r.get(9)?,
                })
            },
        )
        .unwrap()
}

fn count(dir: &Path) -> i64 {
    raw(dir)
        .query_row("SELECT COUNT(*) FROM message", [], |r| r.get(0))
        .unwrap()
}

fn state(dir: &Path, key: &str) -> String {
    raw(dir)
        .query_row("SELECT value FROM sync_state WHERE key = ?1", [key], |r| {
            r.get(0)
        })
        .unwrap()
}

fn body_hlc(row: &Row) -> String {
    serde_json::from_str::<Value>(&row.hlc_json).unwrap()["body"]["h"]
        .as_str()
        .unwrap()
        .to_string()
}

fn group(row: &Row, name: &str) -> String {
    parse_groups_raw(&row.groups_raw).unwrap()[name]
        .get()
        .to_string()
}

fn set_group(dir: &Path, id: &str, name: &'static str, text: &'static str, sv: u64) {
    edit_raw_groups(dir, id, |groups, hlcs| {
        groups.insert(name.into(), RawValue::from_string(text.into()).unwrap());
        let h = hlcs
            .get("body")
            .map(|b| serde_json::from_str::<Value>(b.get()).unwrap()["h"].clone())
            .unwrap();
        hlcs.insert(
            name.into(),
            RawValue::from_string(json!({"h": h, "sv": sv}).to_string()).unwrap(),
        );
    });
}

// ---------- окно захвата: контекст и новая запись ----------

#[test]
fn context_is_up_to_three_latest_undeleted_entries_of_today_oldest_first() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());

    let empty = store.capture_view(&moscow(T0)).unwrap();
    assert_eq!(empty.today, "2026-10-21");
    assert!(empty.context.is_empty() && empty.append_target.is_none() && empty.draft.is_empty());

    store
        .send_message(INBOX_ID, "вчера", &moscow(T0 - 24 * 60 * MINUTE))
        .unwrap();
    assert!(
        store.capture_view(&moscow(T0)).unwrap().context.is_empty(),
        "вчерашняя запись не в контексте"
    );

    let ids: Vec<String> = (0..5)
        .map(|i| {
            store
                .send_message(INBOX_ID, &format!("запись {i}"), &moscow(T0 + i * MINUTE))
                .unwrap()
                .id
        })
        .collect();
    mark_deleted(dir.path(), &ids[4], T0 + 6 * MINUTE);

    let view = store.capture_view(&moscow(T0 + 7 * MINUTE)).unwrap();
    let texts: Vec<&str> = view.context.iter().map(|m| m.body_md.as_str()).collect();
    assert_eq!(
        texts,
        ["запись 1", "запись 2", "запись 3"],
        "три последних неудалённых, самая новая последней"
    );
    assert_eq!(view.append_target.as_deref(), Some(ids[3].as_str()));
}

#[test]
fn capture_send_creates_a_user_entry_in_inbox_and_clears_the_capture_draft() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    store
        .set_capture_draft("недописанное", &moscow(T0))
        .unwrap();
    store
        .set_chat_draft(INBOX_ID, "черновик чата", &moscow(T0))
        .unwrap();

    let outcome = store
        .capture_send("Идея: **попробовать**", &moscow(T0 + MINUTE))
        .unwrap();
    assert_eq!(outcome.action, CaptureAction::Created);
    assert_eq!(outcome.message.chat_id, INBOX_ID);
    assert_eq!(outcome.message.body_md, "Идея: **попробовать**");
    assert_eq!(row(dir.path(), &outcome.message.id).origin, "user");
    assert_eq!(store.list_chats().unwrap()[0].count, 1);
    assert_eq!(store.get_capture_draft().unwrap(), "");
    assert_eq!(
        store.get_chat_draft(INBOX_ID).unwrap(),
        "черновик чата",
        "черновик главного окна не тронут"
    );
}

// ---------- дописывание ----------

#[test]
fn append_within_ten_minutes_extends_the_same_entry_as_a_new_paragraph() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path());
    let first = store
        .capture_send("Идея: попробовать", &moscow(T0))
        .unwrap()
        .message;
    let before = row(dir.path(), &first.id);
    store
        .set_capture_draft("и ещё", &moscow(T0 + 3 * MINUTE))
        .unwrap();

    let view = store.capture_view(&moscow(T0 + 3 * MINUTE)).unwrap();
    assert_eq!(view.append_target.as_deref(), Some(first.id.as_str()));

    let outcome = store
        .capture_append("и ещё", &moscow(T0 + 3 * MINUTE))
        .unwrap();
    assert_eq!(outcome.action, CaptureAction::Appended);
    assert_eq!(outcome.message.id, first.id);
    assert_eq!(outcome.message.body_md, "Идея: попробовать\n\nи ещё");
    assert_eq!(outcome.message.created_at, first.created_at, "то же время");
    assert_eq!(count(dir.path()), 1, "новой записи нет");
    assert_eq!(store.list_chats().unwrap()[0].count, 1);
    assert_eq!(store.get_capture_draft().unwrap(), "");

    let after = row(dir.path(), &first.id);
    assert_eq!(after.body_md, "Идея: попробовать\n\nи ещё");
    assert_eq!(after.created_at, before.created_at);
    assert_eq!(after.edited_at, Some(T0 + 3 * MINUTE));
    assert_eq!(after.body_base, None);
    assert_eq!(
        after.body_supersedes,
        format!(r#"["{}"]"#, body_hlc(&before))
    );
    assert!(body_hlc(&after) > body_hlc(&before), "HLC строго больше");
    assert!(parse_hlc(&body_hlc(&after)).is_ok());
    assert_eq!(
        group(&after, "body"),
        format!(
            r#"{{"base":null,"body_md":"Идея: попробовать\n\nи ещё","edited_at":{},"supersedes":["{}"]}}"#,
            T0 + 3 * MINUTE,
            body_hlc(&before)
        ),
        "байты группы body: каноничные, предыдущая версия в supersedes"
    );
    // Остальные группы и их HLC нетронуты, записей ревизий и других таблиц нет.
    for name in ["core", "flags", "placement", "deleted"] {
        assert_eq!(group(&after, name), group(&before, name), "{name}");
    }
    let hlcs: Value = serde_json::from_str(&after.hlc_json).unwrap();
    let hlcs_before: Value = serde_json::from_str(&before.hlc_json).unwrap();
    for name in ["core", "flags", "placement", "deleted"] {
        assert_eq!(hlcs[name], hlcs_before[name], "HLC группы {name}");
    }
    assert_eq!(hlcs["body"]["sv"], 1);
    assert_eq!(after.schema_version, 1);
    // Колонки — разобранное представление байтов группы (инвариант ADR-0001 §B).
    let body: Value = serde_json::from_str(&group(&after, "body")).unwrap();
    assert_eq!(body["body_md"], json!(after.body_md));
    assert_eq!(body["edited_at"], json!(after.edited_at));

    // Markdown-копия показывает дописанный текст под тем же временем.
    let file = mirror_dir(dir.path())
        .join("Inbox")
        .join("2026")
        .join("2026-10-21.md");
    wait_until("копия с дописанным текстом", || {
        std::fs::read_to_string(&file).is_ok_and(|text| text.contains("Идея: попробовать\n\nи ещё"))
    });
    let text = std::fs::read_to_string(&file).unwrap();
    assert_eq!(text.matches("## 09:12").count(), 1);
    assert_eq!(text.matches("## ").count(), 1);
}

#[test]
fn second_append_keeps_base_and_collects_both_previous_versions() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let first = store.capture_send("a", &moscow(T0)).unwrap().message;
    let h0 = body_hlc(&row(dir.path(), &first.id));
    store.capture_append("b", &moscow(T0 + MINUTE)).unwrap();
    let h1 = body_hlc(&row(dir.path(), &first.id));
    let out = store.capture_append("c", &moscow(T0 + 2 * MINUTE)).unwrap();
    assert_eq!(out.message.body_md, "a\n\nb\n\nc");
    let after = row(dir.path(), &first.id);
    assert_eq!(after.body_base, None);
    assert_eq!(after.body_supersedes, format!(r#"["{h0}","{h1}"]"#));
    assert!(body_hlc(&after) > h1 && h1 > h0);
}

#[test]
fn append_to_a_server_confirmed_version_sets_base_and_empties_supersedes() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let first = store.capture_send("a", &moscow(T0)).unwrap().message;
    let h0 = body_hlc(&row(dir.path(), &first.id));
    raw(dir.path())
        .execute("UPDATE message SET server_known = 1", [])
        .unwrap();
    store.capture_append("b", &moscow(T0 + MINUTE)).unwrap();
    let after = row(dir.path(), &first.id);
    assert_eq!(after.server_known, 1);
    assert_eq!(after.body_base.as_deref(), Some(h0.as_str()));
    assert_eq!(after.body_supersedes, "[]");
    assert_eq!(
        group(&after, "body"),
        format!(
            r#"{{"base":"{h0}","body_md":"a\n\nb","edited_at":{},"supersedes":[]}}"#,
            T0 + MINUTE
        )
    );
}

#[test]
fn append_boundary_is_ten_minutes_inclusive() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    store.capture_send("a", &moscow(T0)).unwrap();
    assert_eq!(
        store
            .capture_append("b", &moscow(T0 + MIN10))
            .unwrap()
            .action,
        CaptureAction::Appended
    );
    let store2 = open_without_mirror(tempfile::tempdir().unwrap().path());
    store2.capture_send("a", &moscow(T0)).unwrap();
    let late = store2.capture_append("b", &moscow(T0 + MIN10 + 1)).unwrap();
    assert_eq!(late.action, CaptureAction::Created);
    assert_eq!(late.message.body_md, "b");
}

#[test]
fn append_without_a_target_creates_a_new_entry() {
    // Пустой Inbox за сегодня.
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let out = store.capture_append("первая", &moscow(T0)).unwrap();
    assert_eq!(out.action, CaptureAction::Created);
    assert_eq!(count(dir.path()), 1);
    assert_eq!(row(dir.path(), &out.message.id).edited_at, None);

    // Последняя запись старше десяти минут.
    let out2 = store
        .capture_append("через пятнадцать минут", &moscow(T0 + 15 * MINUTE))
        .unwrap();
    assert_eq!(out2.action, CaptureAction::Created);
    assert_ne!(out2.message.id, out.message.id);
    assert_eq!(count(dir.path()), 2);
}

#[test]
fn a_foreign_origin_is_never_appended_to() {
    for origin in ["agent", "import_telegram"] {
        let dir = tempfile::tempdir().unwrap();
        let store = open_without_mirror(dir.path());
        let first = store.capture_send("чужая", &moscow(T0)).unwrap().message;
        raw(dir.path())
            .execute("UPDATE message SET origin = ?1", [origin])
            .unwrap();
        assert_eq!(row(dir.path(), &first.id).origin, origin);
        let before = row(dir.path(), &first.id);
        let out = store.capture_append("моя", &moscow(T0 + MINUTE)).unwrap();
        assert_eq!(out.action, CaptureAction::Created, "{origin}");
        let after = row(dir.path(), &first.id);
        assert_eq!(
            after.groups_raw, before.groups_raw,
            "чужая запись нетронута"
        );
        assert_eq!(after.body_md, "чужая");
        // Теперь последняя запись — моя, и дописывать можно к ней, а не к чужой.
        assert_eq!(
            store
                .capture_view(&moscow(T0 + MINUTE))
                .unwrap()
                .append_target
                .as_deref(),
            Some(out.message.id.as_str())
        );
    }
}

#[test]
fn a_deleted_last_entry_is_skipped_and_the_previous_one_is_the_target() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let first = store.capture_send("первая", &moscow(T0)).unwrap().message;
    let second = store
        .capture_send("вторая", &moscow(T0 + MINUTE))
        .unwrap()
        .message;
    mark_deleted(dir.path(), &second.id, T0 + 2 * MINUTE);

    let view = store.capture_view(&moscow(T0 + 3 * MINUTE)).unwrap();
    assert_eq!(view.append_target.as_deref(), Some(first.id.as_str()));
    let out = store
        .capture_append("дописано", &moscow(T0 + 3 * MINUTE))
        .unwrap();
    assert_eq!(out.message.id, first.id);
    assert_eq!(out.message.body_md, "первая\n\nдописано");
    let deleted = row(dir.path(), &second.id);
    assert_eq!(deleted.body_md, "вторая", "удалённая запись не менялась");

    // Если удалена единственная запись — цели нет.
    let dir2 = tempfile::tempdir().unwrap();
    let store2 = open_without_mirror(dir2.path());
    let only = store2.capture_send("одна", &moscow(T0)).unwrap().message;
    mark_deleted(dir2.path(), &only.id, T0 + 1);
    assert_eq!(
        store2
            .capture_append("новая", &moscow(T0 + 2))
            .unwrap()
            .action,
        CaptureAction::Created
    );
}

#[test]
fn an_entry_from_yesterday_is_not_appended_even_within_ten_minutes() {
    // Граница дня 04:00: запись в 03:55 принадлежит вчерашнему дню журнала, в 04:02 уже сегодня.
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let t_0355 = T_NIGHT + (3 * 60 + 55 - 30) * MINUTE; // 00:30 + 3 ч 25 мин
    let first = store
        .capture_send("поздняя", &moscow(t_0355))
        .unwrap()
        .message;
    assert_eq!(first.journal_date, "2026-10-20");
    let view = store.capture_view(&moscow(t_0355 + 7 * MINUTE)).unwrap();
    assert_eq!(view.today, "2026-10-21");
    assert!(view.context.is_empty() && view.append_target.is_none());
    let out = store
        .capture_append("ранняя", &moscow(t_0355 + 7 * MINUTE))
        .unwrap();
    assert_eq!(out.action, CaptureAction::Created);
    assert_eq!(out.message.journal_date, "2026-10-21");
}

#[test]
fn an_entry_with_a_group_of_a_newer_version_is_read_only() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let first = store.capture_send("новее", &moscow(T0)).unwrap().message;
    set_group(
        dir.path(),
        &first.id,
        "future",
        r#"{ "z": 1,  "a": "ё" }"#,
        2,
    );
    raw(dir.path())
        .execute("UPDATE message SET schema_version = 2", [])
        .unwrap();
    let before = row(dir.path(), &first.id);
    assert_eq!(
        store
            .capture_view(&moscow(T0 + MINUTE))
            .unwrap()
            .append_target,
        None
    );
    let out = store.capture_append("новая", &moscow(T0 + MINUTE)).unwrap();
    assert_eq!(out.action, CaptureAction::Created);
    let after = row(dir.path(), &first.id);
    assert_eq!(after.groups_raw, before.groups_raw, "байт в байт");
    assert_eq!(after.hlc_json, before.hlc_json);
    assert_eq!(after.body_md, "новее");
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

// ---------- черновики ----------

#[test]
fn drafts_are_stored_replaced_removed_and_survive_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    {
        let store = open_without_mirror(dir.path());
        assert_eq!(store.get_capture_draft().unwrap(), "");
        assert_eq!(store.get_chat_draft(INBOX_ID).unwrap(), "");
        store.set_capture_draft("раз", &moscow(T0)).unwrap();
        store.set_capture_draft("раз два", &moscow(T0 + 1)).unwrap();
        store
            .set_chat_draft(INBOX_ID, "в главном\nокне", &moscow(T0 + 2))
            .unwrap();
    }
    let store = open_without_mirror(dir.path());
    assert_eq!(store.get_capture_draft().unwrap(), "раз два");
    assert_eq!(store.get_chat_draft(INBOX_ID).unwrap(), "в главном\nокне");
    let rows: i64 = raw(dir.path())
        .query_row("SELECT COUNT(*) FROM draft", [], |r| r.get(0))
        .unwrap();
    assert_eq!(rows, 2, "по строке на ключ");

    store.set_capture_draft("", &moscow(T0 + 3)).unwrap();
    assert_eq!(store.get_capture_draft().unwrap(), "");
    let rows: i64 = raw(dir.path())
        .query_row("SELECT COUNT(*) FROM draft", [], |r| r.get(0))
        .unwrap();
    assert_eq!(rows, 1, "пустой текст удаляет строку");
}

#[test]
fn a_draft_for_a_missing_chat_is_refused_and_an_oversized_one_too() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    assert!(matches!(
        store.set_chat_draft("нет-такого-чата", "x", &moscow(T0)),
        Err(StoreError::ChatNotFound)
    ));
    let rows: i64 = raw(dir.path())
        .query_row("SELECT COUNT(*) FROM draft", [], |r| r.get(0))
        .unwrap();
    assert_eq!(rows, 0);
    let huge = "x".repeat(kinige_store::MAX_DRAFT_BYTES + 1);
    assert!(matches!(
        store.set_capture_draft(&huge, &moscow(T0)),
        Err(StoreError::TooLarge)
    ));
    assert_eq!(store.get_capture_draft().unwrap(), "");
}

#[test]
fn sending_from_the_main_window_clears_only_that_chat_draft_in_the_same_transaction() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    store
        .set_chat_draft(INBOX_ID, "готово к отправке", &moscow(T0))
        .unwrap();
    store.set_capture_draft("чужой", &moscow(T0)).unwrap();
    store
        .send_message(INBOX_ID, "готово к отправке", &moscow(T0 + 1))
        .unwrap();
    assert_eq!(store.get_chat_draft(INBOX_ID).unwrap(), "");
    assert_eq!(store.get_capture_draft().unwrap(), "чужой");

    // Отказ записи: черновик остаётся вместе с текстом.
    store
        .set_chat_draft(INBOX_ID, "не отправится", &moscow(T0 + 2))
        .unwrap();
    let conn = raw(dir.path());
    conn.execute_batch(
        "CREATE TRIGGER fail_dirty BEFORE UPDATE ON mirror_dirty BEGIN SELECT RAISE(ABORT, 'нет места'); END;",
    )
    .unwrap();
    assert!(
        store
            .send_message(INBOX_ID, "не отправится", &moscow(T0 + 3))
            .is_err()
    );
    assert_eq!(store.get_chat_draft(INBOX_ID).unwrap(), "не отправится");
}

#[test]
fn drafts_are_not_part_of_the_markdown_copy_or_the_message_tables() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path());
    store
        .set_capture_draft("СЕКРЕТНЫЙ ЧЕРНОВИК", &moscow(T0))
        .unwrap();
    store
        .send_message(INBOX_ID, "обычная", &moscow(T0))
        .unwrap();
    let file = mirror_dir(dir.path())
        .join("Inbox")
        .join("2026")
        .join("2026-10-21.md");
    wait_until("копия", || file.exists());
    assert!(
        !std::fs::read_to_string(&file)
            .unwrap()
            .contains("СЕКРЕТНЫЙ")
    );
    let in_messages: i64 = raw(dir.path())
        .query_row(
            "SELECT COUNT(*) FROM message WHERE body_md LIKE '%СЕКРЕТНЫЙ%' OR groups_raw LIKE '%СЕКРЕТНЫЙ%'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(in_messages, 0);
}

// ---------- миграция 1 → 2 ----------

#[test]
fn migration_two_adds_only_the_draft_table_and_keeps_every_existing_row() {
    let dir = tempfile::tempdir().unwrap();
    let sent = {
        let store = open_without_mirror(dir.path());
        store
            .send_message(INBOX_ID, "до миграции", &moscow(T0))
            .unwrap()
    };
    let before = row(dir.path(), &sent.id);
    // Возвращаем базу в состояние слайса 1: без таблицы черновиков и с `user_version = 1`.
    {
        let conn = raw(dir.path());
        conn.execute_batch("DROP TABLE draft;").unwrap();
        conn.pragma_update(None, "user_version", 1).unwrap();
    }
    let store = open_without_mirror(dir.path());
    let conn = raw(dir.path());
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, LATEST_VERSION);
    assert_eq!(LATEST_VERSION, 2);
    let columns: Vec<(String, String)> = conn
        .prepare("SELECT name, type FROM pragma_table_info('draft') ORDER BY cid")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        columns,
        [
            ("key".to_string(), "TEXT".to_string()),
            ("body".to_string(), "TEXT".to_string()),
            ("updated_at".to_string(), "INTEGER".to_string())
        ]
    );
    let after = row(dir.path(), &sent.id);
    assert_eq!(after.groups_raw, before.groups_raw);
    assert_eq!(after.hlc_json, before.hlc_json);
    assert_eq!(after.body_md, "до миграции");
    assert_eq!(store.list_chats().unwrap()[0].count, 1);
    // Повторное открытие ничего не меняет.
    drop(store);
    drop(open_without_mirror(dir.path()));
}

// ---------- прерывание процесса ----------

const CHILD_ENV: &str = "KINIGE_CAPTURE_ABORT_DIR";
const CHILD_MODE: &str = "KINIGE_CAPTURE_ABORT_MODE";

fn run_child(test: &str, dir: &Path, mode: &str) {
    let status = Command::new(std::env::current_exe().unwrap())
        .args([test, "--exact", "--nocapture", "--test-threads=1"])
        .env(CHILD_ENV, dir)
        .env(CHILD_MODE, mode)
        .status()
        .unwrap();
    assert!(!status.success(), "дочерний процесс должен был упасть");
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(status.signal(), Some(6), "SIGABRT");
    }
}

#[test]
fn abort_right_after_an_append_commit_keeps_the_rewritten_entry() {
    if let Ok(dir) = std::env::var(CHILD_ENV) {
        let store: Store = open_without_mirror(dir.as_ref());
        store.capture_send("основа", &moscow(T0)).unwrap();
        store
            .set_capture_draft("не отправлено", &moscow(T0))
            .unwrap();
        store
            .capture_append("дописано", &moscow(T0 + MINUTE))
            .unwrap();
        std::process::abort();
    }
    let dir = tempfile::tempdir().unwrap();
    run_child(
        "abort_right_after_an_append_commit_keeps_the_rewritten_entry",
        dir.path(),
        "append",
    );

    let store = open_without_mirror(dir.path());
    let view = store.capture_view(&moscow(T0 + 2 * MINUTE)).unwrap();
    assert_eq!(view.context.len(), 1);
    assert_eq!(view.context[0].body_md, "основа\n\nдописано");
    assert_eq!(view.draft, "", "черновик очищен той же транзакцией");
    let after = row(dir.path(), &view.context[0].id);
    assert_eq!(after.edited_at, Some(T0 + MINUTE));
    assert!(after.body_supersedes.starts_with("[\""));
    let dirty: i64 = raw(dir.path())
        .query_row("SELECT COUNT(*) FROM mirror_dirty", [], |r| r.get(0))
        .unwrap();
    assert_eq!(dirty, 1, "отметка копии пережила прерывание");
}

#[test]
fn abort_right_after_saving_a_draft_keeps_the_draft() {
    if let Ok(dir) = std::env::var(CHILD_ENV) {
        let store: Store = open_without_mirror(dir.as_ref());
        store
            .set_capture_draft("писал, писал…\nи вдруг", &moscow(T0))
            .unwrap();
        store
            .set_chat_draft(INBOX_ID, "и в главном", &moscow(T0))
            .unwrap();
        std::process::abort();
    }
    let dir = tempfile::tempdir().unwrap();
    run_child(
        "abort_right_after_saving_a_draft_keeps_the_draft",
        dir.path(),
        "draft",
    );
    let store = open_without_mirror(dir.path());
    assert_eq!(store.get_capture_draft().unwrap(), "писал, писал…\nи вдруг");
    assert_eq!(store.get_chat_draft(INBOX_ID).unwrap(), "и в главном");
}

// ---------- сочетания клавиш ----------

#[test]
fn shortcuts_default_to_the_decided_combinations_and_can_be_changed_and_reset() {
    let dir = tempfile::tempdir().unwrap();
    {
        let store = open_without_mirror(dir.path());
        let settings = store.get_settings().unwrap();
        let pairs: Vec<(ShortcutKind, &str, bool)> = settings
            .shortcuts
            .iter()
            .map(|s| (s.kind, s.accelerator.as_str(), s.is_default))
            .collect();
        assert_eq!(
            pairs,
            [
                (ShortcutKind::Capture, "Alt+Space", true),
                (ShortcutKind::Clipboard, "Alt+Shift+KeyV", true)
            ]
        );

        assert_eq!(
            store
                .check_shortcut(ShortcutKind::Capture, "Cmd+Alt+KeyN")
                .unwrap(),
            "Alt+Cmd+KeyN"
        );
        store
            .save_shortcut(ShortcutKind::Capture, "Cmd+Alt+KeyN")
            .unwrap();
        assert_eq!(
            store.shortcut_accelerator(ShortcutKind::Capture).unwrap(),
            "Alt+Cmd+KeyN"
        );
    }
    let store = open_without_mirror(dir.path());
    let settings = store.get_settings().unwrap();
    assert_eq!(settings.shortcuts[0].accelerator, "Alt+Cmd+KeyN");
    assert!(!settings.shortcuts[0].is_default);
    assert!(settings.shortcuts[1].is_default);

    store.reset_shortcut(ShortcutKind::Capture).unwrap();
    let settings = store.get_settings().unwrap();
    assert_eq!(settings.shortcuts[0].accelerator, "Alt+Space");
    assert!(settings.shortcuts[0].is_default);
}

#[test]
fn invalid_reserved_and_duplicate_shortcuts_are_refused_before_anything_is_saved() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    for bad in [
        "",
        "Space",
        "Shift+KeyV",
        "Alt+Escape",
        "Cmd+KeyQ",
        "Alt+Alt+KeyV",
        "nonsense",
    ] {
        assert!(
            matches!(
                store.check_shortcut(ShortcutKind::Capture, bad),
                Err(StoreError::ShortcutInvalid)
            ),
            "{bad:?}"
        );
        assert!(store.save_shortcut(ShortcutKind::Capture, bad).is_err());
    }
    // Совпадение с сочетанием другого действия — в любой записи.
    for same in ["Alt+Shift+KeyV", "Shift+Alt+KeyV", "Option+Shift+KeyV"] {
        assert!(
            matches!(
                store.check_shortcut(ShortcutKind::Capture, same),
                Err(StoreError::ShortcutDuplicate)
            ),
            "{same}"
        );
    }
    // Своё же текущее сочетание допустимо.
    assert_eq!(
        store
            .check_shortcut(ShortcutKind::Capture, "Alt+Space")
            .unwrap(),
        "Alt+Space"
    );
    let stored: i64 = raw(dir.path())
        .query_row(
            "SELECT COUNT(*) FROM app_setting WHERE key LIKE 'shortcut_%'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(stored, 0);
    // Повреждённое значение в настройках → умолчание.
    raw(dir.path())
        .execute(
            "INSERT INTO app_setting(key, value) VALUES ('shortcut_capture', 'мусор')",
            [],
        )
        .unwrap();
    assert_eq!(
        store.shortcut_accelerator(ShortcutKind::Capture).unwrap(),
        "Alt+Space"
    );
}
