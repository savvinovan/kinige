//! Дописывание к последней записи Inbox: правило цели (граница 10 минут, `origin`, удалённая, вчерашняя, `sv` > 1), `base`/`supersedes` обеих веток §3.4.

mod common;

use common::capture::*;
use common::*;
use kinige_domain::CaptureAction;
use kinige_domain::hlc::parse as parse_hlc;
use serde_json::{Value, json};

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
fn a_target_shown_at_open_that_expired_before_cmd_enter_gives_a_new_entry() {
    // Окно показало подсказку (цель была), человек думал дольше десяти минут: правило решает Rust в момент отправки.
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let first = store.capture_send("начало", &moscow(T0)).unwrap().message;
    let view = store.capture_view(&moscow(T0 + 9 * MINUTE)).unwrap();
    assert_eq!(view.append_target.as_deref(), Some(first.id.as_str()));

    let out = store
        .capture_append("поздно", &moscow(T0 + 11 * MINUTE))
        .unwrap();
    assert_eq!(out.action, CaptureAction::Created);
    assert_eq!(out.message.body_md, "поздно");
    assert_eq!(count(dir.path()), 2);
    assert_eq!(
        row(dir.path(), &first.id).body_md,
        "начало",
        "первая запись не тронута"
    );
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
    store
        .delete_message(&second.id, &moscow(T0 + 2 * MINUTE))
        .unwrap();

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
    store2.delete_message(&only.id, &moscow(T0 + 1)).unwrap();
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
