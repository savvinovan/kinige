//! Тесты дописывания: правило цели и перезапись группы `body` (ADR-0001 §3.3, §3.4, §4).

use super::*;
use crate::groups::{INBOX_ID, NewMessage, new_message, parse_groups_raw};
use crate::jcs;
use serde_json::{Value, json, value::RawValue};
use std::collections::BTreeMap;

const DEVICE: &str = "5f0c3e2a-9b1d-4c7e-8a64-0d2f1b3c4e5f";
const NOW: i64 = 1_760_000_000_000;

fn hlc(l: u64) -> String {
    format!("{l:013}-00000-{DEVICE}")
}

fn message(body: &str) -> crate::groups::MessageRow {
    new_message(
        &NewMessage {
            id: "0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d",
            chat_id: INBOX_ID,
            body_md: body,
            created_at: NOW,
            created_tz: "Europe/Moscow",
            journal_date: "2026-10-09",
        },
        &hlc(1_760_000_000_000),
    )
    .unwrap()
}

fn current<'a>(row: &'a crate::groups::MessageRow, body_confirmed: bool) -> CurrentBody<'a> {
    CurrentBody {
        groups_raw: &row.groups_raw,
        hlc_json: &row.hlc_json,
        body_confirmed,
    }
}

fn body_group(groups_raw: &str) -> String {
    parse_groups_raw(groups_raw).unwrap()["body"]
        .get()
        .to_string()
}

fn candidate(row: &crate::groups::MessageRow) -> AppendCandidate<'_> {
    AppendCandidate {
        origin: &row.origin,
        created_at: row.created_at,
        journal_date: &row.journal_date,
        hlc_json: &row.hlc_json,
    }
}

#[test]
fn joined_body_starts_a_new_paragraph() {
    assert_eq!(joined_body("Идея", "и ещё"), "Идея\n\nи ещё");
}

#[test]
fn append_target_rule() {
    let row = message("Идея");
    let today = "2026-10-09";
    let can = |c: &AppendCandidate<'_>, now: i64, today: &str| can_append(c, now, today).unwrap();
    assert!(can(&candidate(&row), NOW, today));
    assert!(
        can(&candidate(&row), NOW + APPEND_WINDOW_MS, today),
        "ровно десять минут — ещё можно"
    );
    assert!(
        !can(&candidate(&row), NOW + APPEND_WINDOW_MS + 1, today),
        "десять минут и миллисекунда — уже нет"
    );
    assert!(
        can(&candidate(&row), NOW - 5_000, today),
        "часы чуть ушли назад: запись «из будущего» дописывается"
    );
    assert!(
        !can(&candidate(&row), NOW, "2026-10-10"),
        "вчерашняя запись"
    );
    let mut other = candidate(&row);
    for origin in ["agent", "import_telegram", ""] {
        other.origin = origin;
        assert!(!can(&other, NOW, today), "origin {origin:?}");
    }
}

#[test]
fn records_with_newer_groups_are_not_appended_and_broken_hlc_is_an_error() {
    let row = message("Идея");
    let today = "2026-10-09";
    let newer = row.hlc_json.replace(
        &format!(r#""deleted":{{"h":"{}","sv":1}}"#, hlc(1_760_000_000_000)),
        &format!(r#""deleted":{{"h":"{}","sv":2}}"#, hlc(1_760_000_000_000)),
    );
    assert_ne!(newer, row.hlc_json);
    let mut c = candidate(&row);
    c.hlc_json = &newer;
    assert_eq!(
        can_append(&c, NOW, today),
        Ok(false),
        "sv группы выше поддерживаемой"
    );
    for broken in [
        "",
        "{",
        "[]",
        r#"{"body":{"h":"x"}}"#,
        r#"{"core":{"h":"x","sv":1}}"#,
    ] {
        c.hlc_json = broken;
        assert!(
            matches!(can_append(&c, NOW, today), Err(GroupError::Corrupt(_))),
            "{broken:?} — повреждение, а не «нельзя»"
        );
    }
}

mod rewrite;
mod supersedes;
