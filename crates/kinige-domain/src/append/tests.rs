//! Тесты дописывания: правило цели и перезапись группы `body` (ADR-0001 §3.3, §3.4, §4).

use super::*;
use crate::groups::{INBOX_ID, NewMessage, new_message};

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

#[test]
fn unconfirmed_version_gets_base_from_itself_and_supersedes_the_old_hlc() {
    let row = message("Идея");
    let new_hlc = hlc(1_760_000_100_000);
    let out = rewrite_body(
        &current(&row, false),
        "Идея\n\nи ещё",
        NOW + 100_000,
        &new_hlc,
    )
    .unwrap();
    assert_eq!(
        body_group(&out.groups_raw),
        format!(
            r#"{{"base":null,"body_md":"Идея\n\nи ещё","edited_at":1760000100000,"supersedes":["{}"]}}"#,
            hlc(1_760_000_000_000)
        )
    );
    assert_eq!(out.body_base, None);
    assert_eq!(
        out.body_supersedes,
        format!(r#"["{}"]"#, hlc(1_760_000_000_000))
    );
    assert_eq!(out.body_md, "Идея\n\nи ещё");
    assert_eq!(out.edited_at, NOW + 100_000);
    assert_eq!(out.schema_version, 1);
    let hlcs: BTreeMap<String, Value> = serde_json::from_str(&out.hlc_json).unwrap();
    assert_eq!(hlcs["body"], json!({"h": new_hlc, "sv": 1}));
    for other in ["core", "flags", "placement", "deleted"] {
        assert_eq!(
            hlcs[other],
            json!({"h": hlc(1_760_000_000_000), "sv": 1}),
            "{other} не тронута"
        );
    }
}

#[test]
fn second_rewrite_of_an_unconfirmed_version_accumulates_supersedes() {
    let row = message("Идея");
    let first = rewrite_body(
        &current(&row, false),
        "a\n\nb",
        NOW + 1,
        &hlc(1_760_000_100_000),
    )
    .unwrap();
    let second = rewrite_body(
        &CurrentBody {
            groups_raw: &first.groups_raw,
            hlc_json: &first.hlc_json,
            body_confirmed: false,
        },
        "a\n\nb\n\nc",
        NOW + 2,
        &hlc(1_760_000_200_000),
    )
    .unwrap();
    assert_eq!(
        second.body_supersedes,
        format!(
            r#"["{}","{}"]"#,
            hlc(1_760_000_000_000),
            hlc(1_760_000_100_000)
        )
    );
    assert_eq!(second.body_base, None, "base первой версии остаётся");
}

#[test]
fn confirmed_version_becomes_base_and_supersedes_is_empty() {
    let row = message("Идея");
    let out = rewrite_body(
        &current(&row, true),
        "Идея\n\nи ещё",
        NOW + 5,
        &hlc(1_760_000_100_000),
    )
    .unwrap();
    assert_eq!(
        body_group(&out.groups_raw),
        format!(
            r#"{{"base":"{}","body_md":"Идея\n\nи ещё","edited_at":1760000000005,"supersedes":[]}}"#,
            hlc(1_760_000_000_000)
        )
    );
    assert_eq!(out.body_base, Some(hlc(1_760_000_000_000)));
    assert_eq!(out.body_supersedes, "[]");
}

#[test]
fn supersedes_keeps_the_newest_64() {
    let row = message("Идея");
    let mut groups_raw = row.groups_raw.clone();
    let mut hlc_json = row.hlc_json.clone();
    let mut all = vec![hlc(1_760_000_000_000)];
    for step in 1..=70u64 {
        let new_hlc = hlc(1_760_000_000_000 + step);
        let out = rewrite_body(
            &CurrentBody {
                groups_raw: &groups_raw,
                hlc_json: &hlc_json,
                body_confirmed: false,
            },
            "текст",
            NOW,
            &new_hlc,
        )
        .unwrap();
        groups_raw = out.groups_raw;
        hlc_json = out.hlc_json;
        all.push(new_hlc);
        let list: Vec<String> = serde_json::from_str(&out.body_supersedes).unwrap();
        assert!(list.len() <= MAX_SUPERSEDES, "шаг {step}");
        // Всегда последние версии, кроме текущей.
        let expected: Vec<&String> = all[..all.len() - 1]
            .iter()
            .rev()
            .take(MAX_SUPERSEDES)
            .rev()
            .collect();
        assert_eq!(list.iter().collect::<Vec<_>>(), expected, "шаг {step}");
    }
    let list: Vec<String> = serde_json::from_str(
        &serde_json::from_str::<Value>(&body_group(&groups_raw)).unwrap()["supersedes"].to_string(),
    )
    .unwrap();
    assert_eq!(list.len(), MAX_SUPERSEDES);
    assert_eq!(list.last(), Some(&hlc(1_760_000_000_000 + 69)));
}

/// Запись, у которой в `supersedes` уже `count` меток (старейшая — `l = 1_000 + 0`), текущая версия — `l = 1_000 + count`.
fn with_supersedes(count: u64) -> (String, String, Vec<String>) {
    let row = message("Идея");
    let old: Vec<String> = (0..count).map(|i| hlc(1_000 + i)).collect();
    let mut groups = parse_groups_raw(&row.groups_raw).unwrap();
    groups.insert(
        "body".into(),
        RawValue::from_string(
            String::from_utf8(
                jcs::canonical_bytes(&json!({
                    "base": null, "body_md": "Идея", "edited_at": null, "supersedes": old
                }))
                .unwrap(),
            )
            .unwrap(),
        )
        .unwrap(),
    );
    let mut hlcs: BTreeMap<String, Box<RawValue>> = serde_json::from_str(&row.hlc_json).unwrap();
    hlcs.insert(
        "body".into(),
        RawValue::from_string(json!({"h": hlc(1_000 + count), "sv": 1}).to_string()).unwrap(),
    );
    (
        serde_json::to_string(&groups).unwrap(),
        serde_json::to_string(&hlcs).unwrap(),
        old,
    )
}

#[test]
fn supersedes_is_exactly_64_at_the_boundary_and_drops_only_the_oldest() {
    // 62 прежних + текущая версия → после правки 63 метки: ничего не отброшено.
    // 63 прежних + текущая → ровно 64: ничего не отброшено.
    // 64 прежних + текущая → было бы 65: отбрасывается ровно одна, самая старая.
    for (count, dropped) in [(62u64, 0usize), (63, 0), (64, 1), (70, 7)] {
        let (groups_raw, hlc_json, old) = with_supersedes(count);
        let out = rewrite_body(
            &CurrentBody {
                groups_raw: &groups_raw,
                hlc_json: &hlc_json,
                body_confirmed: false,
            },
            "x",
            NOW,
            &hlc(1_000_000),
        )
        .unwrap();
        let list: Vec<String> = serde_json::from_str(&out.body_supersedes).unwrap();
        let mut expected: Vec<String> = old.clone();
        expected.push(hlc(1_000 + count));
        let expected: Vec<String> = expected.split_off(dropped);
        assert_eq!(
            list.len(),
            expected.len().min(MAX_SUPERSEDES),
            "count={count}"
        );
        assert_eq!(list, expected, "count={count}");
        assert!(list.len() <= 64);
        let group: Value = serde_json::from_str(&body_group(&out.groups_raw)).unwrap();
        assert_eq!(
            group["supersedes"],
            json!(list),
            "группа и колонка совпадают"
        );
        assert_eq!(
            list.last(),
            Some(&hlc(1_000 + count)),
            "прежняя версия в списке"
        );
        if dropped > 0 {
            assert!(!list.contains(&old[0]), "старейшая отброшена");
        }
    }
    let (groups_raw, hlc_json, _) = with_supersedes(63);
    let out = rewrite_body(
        &CurrentBody {
            groups_raw: &groups_raw,
            hlc_json: &hlc_json,
            body_confirmed: false,
        },
        "x",
        NOW,
        &hlc(1_000_000),
    )
    .unwrap();
    let list: Vec<String> = serde_json::from_str(&out.body_supersedes).unwrap();
    assert_eq!(list.len(), 64, "ровно 64, не 63 и не 65");
}

#[test]
fn malformed_hlc_labels_are_corrupt_not_ordered_by_string_comparison() {
    let row = message("Идея");
    // Новая метка повреждена, но «больше» как строка.
    for bad in ["zzz", "9999999999999-00000-not-a-uuid", ""] {
        assert!(
            matches!(
                rewrite_body(&current(&row, false), "x", NOW, bad),
                Err(GroupError::Corrupt(_))
            ),
            "{bad:?}"
        );
    }
    // Прежняя метка повреждена: ошибка, а не HlcNotNewer и не успех.
    let hlc_json = row.hlc_json.replace(
        &format!(r#""body":{{"h":"{}","sv":1}}"#, hlc(1_760_000_000_000)),
        r#""body":{"h":"0","sv":1}"#,
    );
    assert_ne!(hlc_json, row.hlc_json);
    assert!(matches!(
        rewrite_body(
            &CurrentBody {
                groups_raw: &row.groups_raw,
                hlc_json: &hlc_json,
                body_confirmed: false
            },
            "x",
            NOW,
            &hlc(1_760_000_100_000)
        ),
        Err(GroupError::Corrupt(_))
    ));
}

#[test]
fn unknown_groups_unknown_body_fields_and_extra_hlc_fields_survive_byte_for_byte() {
    let row = message("Идея");
    // Группа новой версии с пробелами в байтах и своим sv, лишнее поле в записи HLC чужой группы, неизвестное поле в body.
    let future = r#"{ "z": [1, 2],  "a": "ё" }"#;
    let mut groups = parse_groups_raw(&row.groups_raw).unwrap();
    groups.insert(
        "future".into(),
        RawValue::from_string(future.into()).unwrap(),
    );
    groups.insert(
        "body".into(),
        RawValue::from_string(
            r#"{"base":null,"body_md":"Идея","edited_at":null,"flair":"x","supersedes":[]}"#.into(),
        )
        .unwrap(),
    );
    let groups_raw = serde_json::to_string(&groups).unwrap();
    let mut hlcs: BTreeMap<String, Box<RawValue>> = serde_json::from_str(&row.hlc_json).unwrap();
    hlcs.insert(
        "future".into(),
        RawValue::from_string(format!(
            r#"{{ "h":"{}", "sv":2, "x":true }}"#,
            hlc(1_760_000_000_001)
        ))
        .unwrap(),
    );
    hlcs.insert(
        "flags".into(),
        RawValue::from_string(format!(
            r#"{{"h":"{}","sv":1,"note":"keep"}}"#,
            hlc(1_760_000_000_000)
        ))
        .unwrap(),
    );
    let hlc_json = serde_json::to_string(&hlcs).unwrap();

    let out = rewrite_body(
        &CurrentBody {
            groups_raw: &groups_raw,
            hlc_json: &hlc_json,
            body_confirmed: false,
        },
        "Идея\n\nи ещё",
        NOW,
        &hlc(1_760_000_100_000),
    )
    .unwrap();

    let after = parse_groups_raw(&out.groups_raw).unwrap();
    assert_eq!(after["future"].get(), future, "неизвестная группа дословно");
    for name in ["core", "flags", "placement", "deleted"] {
        assert_eq!(after[name].get(), groups[name].get(), "{name} дословно");
    }
    let body: Value = serde_json::from_str(after["body"].get()).unwrap();
    assert_eq!(body["flair"], "x", "неизвестное поле body сохранено");
    assert_eq!(
        after["body"].get(),
        String::from_utf8(jcs::canonical_bytes(&body).unwrap()).unwrap(),
        "перезаписанная группа каноничная"
    );
    let hlc_after: BTreeMap<String, Box<RawValue>> = serde_json::from_str(&out.hlc_json).unwrap();
    assert_eq!(hlc_after["future"].get(), hlcs["future"].get());
    assert_eq!(hlc_after["flags"].get(), hlcs["flags"].get());
    assert_eq!(out.schema_version, 2, "максимум по группам");
}

#[test]
fn corrupt_input_is_an_error_and_nothing_is_returned() {
    let row = message("Идея");
    let newer = hlc(1_760_000_100_000);
    let ok = |groups_raw: &str, hlc_json: &str| {
        rewrite_body(
            &CurrentBody {
                groups_raw,
                hlc_json,
                body_confirmed: false,
            },
            "x",
            NOW,
            &newer,
        )
    };
    assert!(ok(&row.groups_raw, &row.hlc_json).is_ok());
    for groups_raw in ["", "{", "[]", "{}", r#"{"body":5}"#, r#"{"body":[]}"#] {
        assert!(
            matches!(ok(groups_raw, &row.hlc_json), Err(GroupError::Corrupt(_))),
            "{groups_raw:?}"
        );
    }
    for hlc_json in [
        "",
        "{",
        "{}",
        r#"{"body":{"h":"a"}}"#,
        r#"{"body":{"sv":1}}"#,
    ] {
        assert!(
            matches!(ok(&row.groups_raw, hlc_json), Err(GroupError::Corrupt(_))),
            "{hlc_json:?}"
        );
    }
    let bad_base = row.groups_raw.replace(r#""base":null"#, r#""base":5"#);
    assert_ne!(bad_base, row.groups_raw);
    assert!(matches!(
        ok(&bad_base, &row.hlc_json),
        Err(GroupError::Corrupt(_))
    ));
}

#[test]
fn body_group_of_a_newer_version_is_not_rewritten() {
    let row = message("Идея");
    let hlc_json = row.hlc_json.replace(
        &format!(r#""body":{{"h":"{}","sv":1}}"#, hlc(1_760_000_000_000)),
        &format!(r#""body":{{"h":"{}","sv":2}}"#, hlc(1_760_000_000_000)),
    );
    assert_ne!(hlc_json, row.hlc_json);
    assert_eq!(
        rewrite_body(
            &CurrentBody {
                groups_raw: &row.groups_raw,
                hlc_json: &hlc_json,
                body_confirmed: false
            },
            "x",
            NOW,
            &hlc(1_760_000_100_000)
        ),
        Err(GroupError::UnsupportedVersion)
    );
}

#[test]
fn new_hlc_must_be_strictly_greater() {
    let row = message("Идея");
    for bad in [hlc(1_760_000_000_000), hlc(1_759_999_999_999)] {
        assert_eq!(
            rewrite_body(&current(&row, false), "x", NOW, &bad),
            Err(GroupError::HlcNotNewer),
            "{bad}"
        );
    }
    // Тот же момент, но счётчик больше.
    let next = format!("{:013}-00001-{DEVICE}", 1_760_000_000_000u64);
    assert!(rewrite_body(&current(&row, false), "x", NOW, &next).is_ok());
}

#[test]
fn group_limit_counts_canonical_bytes_after_joining() {
    let row = message("Идея");
    let newer = hlc(1_760_000_100_000);
    // Кавычки и переводы строк удваиваются в JCS.
    let escaped = "\"\n".repeat(256 * 1024 - 8);
    assert_eq!(
        rewrite_body(&current(&row, false), &escaped, NOW, &newer),
        Err(GroupError::TooLarge)
    );
    let plain = "я".repeat(200 * 1024);
    assert!(rewrite_body(&current(&row, false), &plain, NOW, &newer).is_ok());
    let too_long = "я".repeat(300 * 1024);
    assert_eq!(
        rewrite_body(&current(&row, false), &too_long, NOW, &newer),
        Err(GroupError::TooLarge)
    );
}
