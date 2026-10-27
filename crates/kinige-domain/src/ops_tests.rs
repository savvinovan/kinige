//! Тесты перезаписи групп слайса 3: байты каждой группы, `placement` в обеих ветках §3.4, снимок ревизии `edit`,
//! `purged_of` с неизвестной группой, новый чат со всеми группами, ключи порядка.

use crate::chat::{self, ChatInputError, NewChat, StateChange};
use crate::groups::{GroupError, INBOX_ID, MessageRow, NewMessage, new_message, parse_groups_raw};
use crate::history::{days_left, edit_revision, edit_snapshot, is_expired, purged_of};
use crate::message_ops::{self, FlagChange, message_columns};
use crate::rewrite::{Groups, is_read_only};
use serde_json::{Value, json, value::RawValue};
use std::collections::BTreeMap;

const DEVICE: &str = "5f0c3e2a-9b1d-4c7e-8a64-0d2f1b3c4e5f";
const NOW: i64 = 1_760_000_000_000;
const MSG: &str = "0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d";

fn hlc(l: u64) -> String {
    format!("{l:013}-00000-{DEVICE}")
}

fn message() -> MessageRow {
    new_message(
        &NewMessage {
            id: MSG,
            chat_id: INBOX_ID,
            body_md: "Идея #заметка",
            created_at: NOW,
            created_tz: "Europe/Moscow",
            journal_date: "2026-10-09",
        },
        &hlc(NOW as u64),
    )
    .unwrap()
}

fn groups(row: &MessageRow) -> Groups<'_> {
    Groups {
        groups_raw: &row.groups_raw,
        hlc_json: &row.hlc_json,
    }
}

fn group_text(groups_raw: &str, name: &str) -> String {
    parse_groups_raw(groups_raw).unwrap()[name]
        .get()
        .to_string()
}

fn hlcs(hlc_json: &str) -> BTreeMap<String, Value> {
    serde_json::from_str(hlc_json).unwrap()
}

/// Все группы, кроме `except`, дословно прежние; HLC всех остальных групп прежние.
fn assert_only_changed(before: &MessageRow, after_raw: &str, after_hlc: &str, except: &str) {
    let b = parse_groups_raw(&before.groups_raw).unwrap();
    let a = parse_groups_raw(after_raw).unwrap();
    assert_eq!(a.keys().collect::<Vec<_>>(), b.keys().collect::<Vec<_>>());
    for (name, raw) in &b {
        if name != except {
            assert_eq!(a[name].get(), raw.get(), "{name} дословно");
            assert_eq!(
                hlcs(after_hlc)[name],
                hlcs(&before.hlc_json)[name],
                "{name}: HLC"
            );
        }
    }
    assert_ne!(a[except].get(), b[except].get(), "{except} изменилась");
}

#[test]
fn deleted_group_bytes_for_delete_and_restore() {
    let row = message();
    let deleted =
        message_ops::set_deleted(&groups(&row), Some(NOW + 5), &hlc(NOW as u64 + 1000)).unwrap();
    assert_eq!(
        group_text(&deleted.groups_raw, "deleted"),
        format!(r#"{{"deleted_at":{}}}"#, NOW + 5)
    );
    assert_only_changed(&row, &deleted.groups_raw, &deleted.hlc_json, "deleted");
    assert_eq!(
        hlcs(&deleted.hlc_json)["deleted"],
        json!({"h": hlc(NOW as u64 + 1000), "sv": 1})
    );
    assert_eq!(
        message_columns(&deleted.groups_raw).unwrap().deleted_at,
        Some(NOW + 5)
    );

    let restored = message_ops::set_deleted(
        &Groups {
            groups_raw: &deleted.groups_raw,
            hlc_json: &deleted.hlc_json,
        },
        None,
        &hlc(NOW as u64 + 2000),
    )
    .unwrap();
    assert_eq!(
        group_text(&restored.groups_raw, "deleted"),
        r#"{"deleted_at":null}"#
    );
    assert_eq!(
        message_columns(&restored.groups_raw).unwrap().deleted_at,
        None
    );
}

#[test]
fn flags_group_bytes_and_rules() {
    let row = message();
    let task = message_ops::change_flag(
        &groups(&row),
        FlagChange::Task(true),
        NOW,
        &hlc(NOW as u64 + 1),
    )
    .unwrap();
    assert_eq!(
        group_text(&task.groups_raw, "flags"),
        r#"{"favorite_at":null,"is_task":true,"pinned_at":null,"task_done_at":null}"#
    );
    assert_only_changed(&row, &task.groups_raw, &task.hlc_json, "flags");

    let g = |r: &crate::rewrite::Rewritten| (r.groups_raw.clone(), r.hlc_json.clone());
    let (raw, hl) = g(&task);
    let done = message_ops::change_flag(
        &Groups {
            groups_raw: &raw,
            hlc_json: &hl,
        },
        FlagChange::TaskDone(true),
        NOW + 9,
        &hlc(NOW as u64 + 2),
    )
    .unwrap();
    assert_eq!(
        group_text(&done.groups_raw, "flags"),
        format!(
            r#"{{"favorite_at":null,"is_task":true,"pinned_at":null,"task_done_at":{}}}"#,
            NOW + 9
        )
    );

    // Снятие задачи обнуляет «выполнено»; избранное не трогается.
    let (raw, hl) = g(&done);
    let fav = message_ops::change_flag(
        &Groups {
            groups_raw: &raw,
            hlc_json: &hl,
        },
        FlagChange::Favorite(true),
        NOW + 20,
        &hlc(NOW as u64 + 3),
    )
    .unwrap();
    let (raw, hl) = g(&fav);
    let untask = message_ops::change_flag(
        &Groups {
            groups_raw: &raw,
            hlc_json: &hl,
        },
        FlagChange::Task(false),
        NOW,
        &hlc(NOW as u64 + 4),
    )
    .unwrap();
    let columns = message_columns(&untask.groups_raw).unwrap();
    assert!(!columns.is_task);
    assert_eq!(columns.task_done_at, None);
    assert_eq!(columns.favorite_at, Some(NOW + 20));

    // «Выполнено» у записи, которая не задача.
    assert_eq!(
        message_ops::change_flag(
            &groups(&row),
            FlagChange::TaskDone(true),
            NOW,
            &hlc(NOW as u64 + 1)
        ),
        Err(GroupError::Invalid(
            "отметка «выполнено» у записи, которая не задача"
        ))
    );
}

#[test]
fn placement_first_rewrite_covers_both_branches_of_the_chain() {
    let row = message();
    // Версия подтверждена сервером: base = её HLC, supersedes пуст.
    let confirmed =
        message_ops::set_journal_date(&groups(&row), "2026-10-08", true, &hlc(NOW as u64 + 1000))
            .unwrap();
    assert_eq!(
        group_text(&confirmed.groups_raw, "placement"),
        format!(
            r#"{{"base":"{}","chat_id":"{INBOX_ID}","journal_date":"2026-10-08","supersedes":[]}}"#,
            hlc(NOW as u64)
        )
    );
    assert_only_changed(
        &row,
        &confirmed.groups_raw,
        &confirmed.hlc_json,
        "placement",
    );

    // Собственная неподтверждённая версия: base остаётся (у первой — null), прежний HLC попадает в supersedes.
    let own =
        message_ops::set_journal_date(&groups(&row), "2026-10-08", false, &hlc(NOW as u64 + 1000))
            .unwrap();
    assert_eq!(
        group_text(&own.groups_raw, "placement"),
        format!(
            r#"{{"base":null,"chat_id":"{INBOX_ID}","journal_date":"2026-10-08","supersedes":["{}"]}}"#,
            hlc(NOW as u64)
        )
    );
    let columns = message_columns(&own.groups_raw).unwrap();
    assert_eq!(columns.journal_date, "2026-10-08");
    assert_eq!(columns.placement_base, None);
    assert_eq!(
        columns.placement_supersedes,
        format!(r#"["{}"]"#, hlc(NOW as u64))
    );

    // Вторая перезапись собственной версии накапливает supersedes.
    let again = message_ops::set_journal_date(
        &Groups {
            groups_raw: &own.groups_raw,
            hlc_json: &own.hlc_json,
        },
        "2026-10-07",
        false,
        &hlc(NOW as u64 + 2000),
    )
    .unwrap();
    let columns = message_columns(&again.groups_raw).unwrap();
    assert_eq!(
        columns.placement_supersedes,
        format!(r#"["{}","{}"]"#, hlc(NOW as u64), hlc(NOW as u64 + 1000))
    );
}

#[test]
fn dates_must_exist_but_the_future_is_allowed() {
    let row = message();
    for bad in [
        "2026-02-30",
        "2026-13-01",
        "2027-02-29",
        "abc",
        "",
        "0999-01-01",
    ] {
        assert!(
            matches!(
                message_ops::set_journal_date(&groups(&row), bad, true, &hlc(NOW as u64 + 1)),
                Err(GroupError::Invalid(_))
            ),
            "{bad}"
        );
    }
    for good in ["2028-02-29", "2099-12-31", "2026-10-09"] {
        assert!(
            message_ops::set_journal_date(&groups(&row), good, true, &hlc(NOW as u64 + 1)).is_ok(),
            "{good}"
        );
    }
}

#[test]
fn hlc_must_grow_strictly_and_groups_must_exist() {
    let row = message();
    assert_eq!(
        message_ops::set_deleted(&groups(&row), Some(1), &hlc(NOW as u64)),
        Err(GroupError::HlcNotNewer),
        "равный"
    );
    assert_eq!(
        message_ops::set_deleted(&groups(&row), Some(1), &hlc(NOW as u64 - 1)),
        Err(GroupError::HlcNotNewer),
        "меньший"
    );
    let without = {
        let mut map: BTreeMap<String, Box<RawValue>> = parse_groups_raw(&row.groups_raw).unwrap();
        map.remove("deleted");
        serde_json::to_string(&map).unwrap()
    };
    assert!(matches!(
        message_ops::set_deleted(
            &Groups {
                groups_raw: &without,
                hlc_json: &row.hlc_json
            },
            Some(1),
            &hlc(NOW as u64 + 1)
        ),
        Err(GroupError::Corrupt(_))
    ));
    assert!(matches!(
        message_ops::set_deleted(
            &Groups {
                groups_raw: "{",
                hlc_json: &row.hlc_json
            },
            Some(1),
            &hlc(NOW as u64 + 1)
        ),
        Err(GroupError::Corrupt(_))
    ));
    assert!(matches!(
        message_ops::set_deleted(
            &Groups {
                groups_raw: &row.groups_raw,
                hlc_json: "{}"
            },
            Some(1),
            &hlc(NOW as u64 + 1)
        ),
        Err(GroupError::Corrupt(_))
    ));
}

#[test]
fn read_only_when_any_group_is_newer() {
    let row = message();
    assert_eq!(is_read_only(&row.hlc_json), Ok(false));
    let newer = {
        let mut entries = hlcs(&row.hlc_json);
        entries.insert("flags".into(), json!({"h": hlc(NOW as u64), "sv": 2}));
        serde_json::to_string(&entries).unwrap()
    };
    assert_eq!(is_read_only(&newer), Ok(true));
    assert!(matches!(is_read_only("{"), Err(GroupError::Corrupt(_))));
    assert!(matches!(
        is_read_only(r#"{"core":{"h":"x"}}"#),
        Err(GroupError::Corrupt(_))
    ));
    // Группа, которую переписывают, в sv 2 — отказ; чужая группа в sv 2 не мешает перезаписи своей.
    let mut other = row.clone();
    other.hlc_json = row.hlc_json.replace(
        &format!(r#""flags":{{"h":"{}","sv":1}}"#, hlc(NOW as u64)),
        &format!(r#""flags":{{"h":"{}","sv":2}}"#, hlc(NOW as u64)),
    );
    assert_ne!(other.hlc_json, row.hlc_json);
    assert_eq!(
        message_ops::change_flag(
            &groups(&other),
            FlagChange::Favorite(true),
            NOW,
            &hlc(NOW as u64 + 1)
        ),
        Err(GroupError::UnsupportedVersion)
    );
    let ok = message_ops::set_deleted(&groups(&other), Some(1), &hlc(NOW as u64 + 1)).unwrap();
    assert_eq!(ok.schema_version, 2, "максимум sv по группам");
    assert_eq!(hlcs(&ok.hlc_json)["flags"], hlcs(&other.hlc_json)["flags"]);
}

#[test]
fn unknown_group_and_unknown_fields_survive_a_rewrite_of_another_group() {
    let row = message();
    let mut raw: BTreeMap<String, Box<RawValue>> = parse_groups_raw(&row.groups_raw).unwrap();
    raw.insert(
        "future".into(),
        RawValue::from_string(r#"{ "z": [1, 2],  "a": "ё" }"#.into()).unwrap(),
    );
    let mut hl: BTreeMap<String, Box<RawValue>> = serde_json::from_str(&row.hlc_json).unwrap();
    hl.insert(
        "future".into(),
        RawValue::from_string(format!(
            r#"{{ "h": "{}", "sv": 2, "x": true }}"#,
            hlc(NOW as u64)
        ))
        .unwrap(),
    );
    let (raw, hl) = (
        serde_json::to_string(&raw).unwrap(),
        serde_json::to_string(&hl).unwrap(),
    );
    let out = message_ops::set_deleted(
        &Groups {
            groups_raw: &raw,
            hlc_json: &hl,
        },
        Some(7),
        &hlc(NOW as u64 + 1),
    )
    .unwrap();
    assert_eq!(
        group_text(&out.groups_raw, "future"),
        r#"{ "z": [1, 2],  "a": "ё" }"#
    );
    let after: BTreeMap<String, Box<RawValue>> = serde_json::from_str(&out.hlc_json).unwrap();
    assert_eq!(
        after["future"].get(),
        format!(r#"{{ "h": "{}", "sv": 2, "x": true }}"#, hlc(NOW as u64))
    );
}

#[test]
fn edit_revision_is_a_verbatim_snapshot_of_every_group_before_the_edit() {
    let row = message();
    let mut raw: BTreeMap<String, Box<RawValue>> = parse_groups_raw(&row.groups_raw).unwrap();
    raw.insert(
        "future".into(),
        RawValue::from_string(r#"{ "z": 1 }"#.into()).unwrap(),
    );
    let mut hl: BTreeMap<String, Value> = hlcs(&row.hlc_json);
    hl.insert(
        "future".into(),
        json!({"h": hlc(NOW as u64), "sv": 3, "x": true}),
    );
    let (raw, hl) = (
        serde_json::to_string(&raw).unwrap(),
        serde_json::to_string(&hl).unwrap(),
    );

    let snapshot = edit_snapshot(&raw, &hl).unwrap();
    let value: Value = serde_json::from_str(&snapshot).unwrap();
    let entries = value["groups"].as_array().unwrap();
    let names: Vec<&str> = entries.iter().map(|e| e["g"].as_str().unwrap()).collect();
    assert_eq!(
        names,
        ["body", "core", "deleted", "flags", "future", "placement"]
    );
    let future = &entries[4];
    assert_eq!(future["raw"], r#"{ "z": 1 }"#, "байты дословно, строкой");
    assert_eq!(future["sv"], 3);
    assert_eq!(future["h"], hlc(NOW as u64));
    // Снимок каноничен.
    assert_eq!(
        crate::jcs::canonical_bytes(&value).unwrap(),
        snapshot.as_bytes()
    );

    let revision = edit_revision("rev-1", MSG, &raw, &hl, NOW + 5, &hlc(NOW as u64 + 9)).unwrap();
    assert_eq!(revision.reason, "edit");
    assert_eq!(revision.snapshot_json, snapshot);
    assert_eq!(revision.schema_version, 1);
    assert_eq!(
        revision.hlc_json,
        format!(r#"{{"core":{{"h":"{}","sv":1}}}}"#, hlc(NOW as u64 + 9))
    );
    let core: Value = serde_json::from_str(&group_text(&revision.groups_raw, "core")).unwrap();
    assert_eq!(core["message_id"], MSG);
    assert_eq!(core["saved_at"], NOW + 5);
    assert_eq!(core["reason"], "edit");
    assert_eq!(core["snapshot_json"], value);

    // Нет HLC у группы — повреждение, а не пропуск.
    assert!(matches!(
        edit_snapshot(&raw, &row.hlc_json),
        Err(GroupError::Corrupt(_))
    ));
}

#[test]
fn purged_of_covers_every_group_including_unknown_ones() {
    let row = message();
    let mut hl = hlcs(&row.hlc_json);
    hl.insert(
        "future".into(),
        json!({"h": hlc(NOW as u64 + 77), "sv": 9, "x": 1}),
    );
    let text = purged_of(&serde_json::to_string(&hl).unwrap()).unwrap();
    assert_eq!(
        text,
        format!(
            r#"{{"body":"{0}","core":"{0}","deleted":"{0}","flags":"{0}","future":"{1}","placement":"{0}"}}"#,
            hlc(NOW as u64),
            hlc(NOW as u64 + 77)
        )
    );
    assert!(matches!(purged_of("{}"), Err(GroupError::Corrupt(_))));
    assert!(matches!(purged_of("["), Err(GroupError::Corrupt(_))));
    assert!(matches!(
        purged_of(r#"{"core":{"sv":1}}"#),
        Err(GroupError::Corrupt(_))
    ));
}

#[test]
fn trash_timing() {
    const DAY: i64 = 86_400_000;
    assert_eq!(days_left(0, 0), 30);
    assert_eq!(days_left(0, 1), 30, "неполный день — день");
    assert_eq!(days_left(0, DAY), 29);
    assert_eq!(days_left(0, 29 * DAY), 1);
    assert_eq!(days_left(0, 30 * DAY - 1), 1);
    assert_eq!(days_left(0, 30 * DAY), 0);
    assert_eq!(days_left(0, 99 * DAY), 0);
    assert!(!is_expired(0, 30 * DAY - 1));
    assert!(is_expired(0, 30 * DAY));
    assert!(!is_expired(10 * DAY, 39 * DAY));
}

#[test]
fn new_chat_has_all_six_groups_under_one_hlc() {
    let chat = chat::new_chat(
        &NewChat {
            id: "c1",
            title: "Книги",
            icon: "📚",
            sort_key: "a000001",
            created_at: NOW,
        },
        &hlc(NOW as u64),
    )
    .unwrap();
    let groups = parse_groups_raw(&chat.groups_raw).unwrap();
    assert_eq!(
        groups.keys().map(String::as_str).collect::<Vec<_>>(),
        ["core", "deleted", "meta", "order", "privacy", "state"]
    );
    assert_eq!(
        groups["core"].get(),
        format!(r#"{{"created_at":{NOW},"is_inbox":false}}"#)
    );
    assert_eq!(
        groups["meta"].get(),
        r#"{"color":null,"icon":"📚","title":"Книги"}"#
    );
    assert_eq!(groups["order"].get(), r#"{"sort_key":"a000001"}"#);
    assert_eq!(
        groups["state"].get(),
        r#"{"archived_at":null,"pinned_at":null}"#
    );
    assert_eq!(groups["privacy"].get(), r#"{"ai_excluded":false}"#);
    assert_eq!(groups["deleted"].get(), r#"{"deleted_at":null}"#);
    for entry in hlcs(&chat.hlc_json).values() {
        assert_eq!(*entry, json!({"h": hlc(NOW as u64), "sv": 1}));
    }
    assert_eq!(chat::chat_columns(&chat.groups_raw).unwrap().title, "Книги");
}

#[test]
fn chat_meta_and_state_rewrites_keep_the_rest_verbatim() {
    let chat = chat::new_chat(
        &NewChat {
            id: "c1",
            title: "Книги",
            icon: "📚",
            sort_key: "a000001",
            created_at: NOW,
        },
        &hlc(NOW as u64),
    )
    .unwrap();
    let cur = Groups {
        groups_raw: &chat.groups_raw,
        hlc_json: &chat.hlc_json,
    };
    let renamed = chat::set_meta(&cur, Some("Фильмы"), None, &hlc(NOW as u64 + 1)).unwrap();
    assert_eq!(
        group_text(&renamed.groups_raw, "meta"),
        r#"{"color":null,"icon":"📚","title":"Фильмы"}"#
    );
    let iconed = chat::set_meta(&cur, None, Some("🎬"), &hlc(NOW as u64 + 1)).unwrap();
    assert_eq!(
        group_text(&iconed.groups_raw, "meta"),
        r#"{"color":null,"icon":"🎬","title":"Книги"}"#
    );
    for other in ["core", "order", "state", "privacy", "deleted"] {
        assert_eq!(
            group_text(&renamed.groups_raw, other),
            group_text(&chat.groups_raw, other),
            "{other}"
        );
    }

    let pinned = chat::set_state(
        &cur,
        StateChange::Pinned(Some(NOW + 3)),
        &hlc(NOW as u64 + 2),
    )
    .unwrap();
    assert_eq!(
        group_text(&pinned.groups_raw, "state"),
        format!(r#"{{"archived_at":null,"pinned_at":{}}}"#, NOW + 3)
    );
    let archived = chat::set_state(
        &Groups {
            groups_raw: &pinned.groups_raw,
            hlc_json: &pinned.hlc_json,
        },
        StateChange::Archived(Some(NOW + 4)),
        &hlc(NOW as u64 + 3),
    )
    .unwrap();
    let columns = chat::chat_columns(&archived.groups_raw).unwrap();
    assert_eq!(
        (columns.pinned_at, columns.archived_at),
        (Some(NOW + 3), Some(NOW + 4))
    );
    assert_eq!(
        chat::set_state(&cur, StateChange::Archived(None), &hlc(NOW as u64)),
        Err(GroupError::HlcNotNewer)
    );
}

#[test]
fn chat_input_rules() {
    assert_eq!(chat::normalize_title("  Книги  "), Ok("Книги".to_string()));
    assert_eq!(chat::normalize_title(""), Err(ChatInputError::TitleEmpty));
    assert_eq!(
        chat::normalize_title(" \t "),
        Err(ChatInputError::TitleEmpty)
    );
    assert_eq!(
        chat::normalize_title("a\nb"),
        Err(ChatInputError::TitleEmpty)
    );
    assert!(chat::normalize_title(&"я".repeat(128)).is_ok());
    assert_eq!(
        chat::normalize_title(&"я".repeat(129)),
        Err(ChatInputError::TitleTooLong)
    );
    assert_eq!(chat::normalize_icon(None), Ok("💬".to_string()));
    assert_eq!(chat::normalize_icon(Some("  ")), Ok("💬".to_string()));
    assert_eq!(chat::normalize_icon(Some("📚")), Ok("📚".to_string()));
    assert_eq!(chat::normalize_icon(Some("👨‍👩‍👧‍👦")), Ok("👨‍👩‍👧‍👦".to_string()));
    assert_eq!(
        chat::normalize_icon(Some("a b")),
        Err(ChatInputError::IconInvalid)
    );
    assert_eq!(
        chat::normalize_icon(Some(&"😀".repeat(9))),
        Err(ChatInputError::IconInvalid)
    );
}

#[test]
fn order_keys_grow_and_sort_after_everything_before() {
    let first = chat::key_after(None);
    assert_eq!(first, "a000001");
    assert!(first.as_str() > "a0", "после Inbox");
    assert_eq!(chat::key_after(Some("a0")), "a000001");
    let mut key = first;
    for _ in 0..2000 {
        let next = chat::key_after(Some(&key));
        assert!(next > key, "{next} > {key}");
        key = next;
    }
    assert_eq!(
        chat::key_after(Some("a00000z")),
        "a000010",
        "перенос разряда"
    );
    assert_eq!(
        chat::key_after(Some("azzzzzz")),
        "azzzzzz1",
        "переполнение — приставка"
    );
    assert!(chat::key_after(Some("azzzzzz")).as_str() > "azzzzzz");
    // Чужой формат: строго больше максимума.
    let foreign = chat::key_after(Some("m5"));
    assert!(foreign.as_str() > "m5", "{foreign}");
    assert_eq!(chat::key_after(Some("A")), "a000001");
}
