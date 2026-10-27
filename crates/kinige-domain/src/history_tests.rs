//! Тесты истории слайса 3 (продолжение `ops_tests.rs`): снимок ревизии `edit`, `purged_of` с неизвестной группой, сроки
//! корзины.

use crate::groups::{GroupError, INBOX_ID, MessageRow, NewMessage, new_message, parse_groups_raw};
use crate::history::{days_left, edit_revision, edit_snapshot, is_expired, purged_of};
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

fn group_text(groups_raw: &str, name: &str) -> String {
    parse_groups_raw(groups_raw).unwrap()[name]
        .get()
        .to_string()
}

fn hlcs(hlc_json: &str) -> BTreeMap<String, Value> {
    serde_json::from_str(hlc_json).unwrap()
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
