//! Дописывание к последней записи (ТЗ §5.1, §9.3, слайс 2): правило «куда дописывать» и первая перезапись группы
//! `body` (ADR-0001 §3.3, §3.4, §4). Без IO: записи и часов здесь нет, вызывающий передаёт строки из базы и новый HLC.

use crate::groups::{
    GroupError, MAX_BODY_GROUP_BYTES, ORIGIN_USER, SCHEMA_VERSION, parse_groups_raw,
};
use crate::jcs;
use serde_json::{Map, Value, json, value::RawValue};
use std::collections::BTreeMap;

/// Окно дописывания: запись не старше десяти минут (ТЗ §2.1, §9.3).
pub const APPEND_WINDOW_MS: i64 = 10 * 60 * 1000;
/// Предел `supersedes` в группе `body` (ADR-0001 §3.4): старейшие отбрасываются.
pub const MAX_SUPERSEDES: usize = 64;

/// Что нужно знать о последней неудалённой записи Inbox, чтобы решить, можно ли дописывать.
#[derive(Debug, Clone, Copy)]
pub struct AppendCandidate<'a> {
    pub origin: &'a str,
    pub created_at: i64,
    pub journal_date: &'a str,
    /// `hlc_json` записи: по нему проверяется `sv` каждой группы.
    pub hlc_json: &'a str,
}

/// Можно ли дописать к записи: её создал пользователь (`origin = 'user'`, записи агента и импорта не дописываются, §9.3),
/// она создана не больше десяти минут назад, относится к сегодняшнему дню журнала, а все её группы — в поддерживаемой
/// `sv` (ADR-0001 §4 п. 4: запись с более новой группой только для чтения). Повреждённый `hlc_json` (не разбирается, нет
/// группы `body`, у группы нет `sv`) — ошибка, а не «нет»: запись тихо не пропускается, повреждение видно.
pub fn can_append(
    candidate: &AppendCandidate<'_>,
    now_ms: i64,
    today: &str,
) -> Result<bool, GroupError> {
    let entries = serde_json::from_str::<BTreeMap<String, Value>>(candidate.hlc_json)
        .map_err(|e| GroupError::Corrupt(e.to_string()))?;
    if !entries.contains_key("body") {
        return Err(GroupError::Corrupt("нет HLC группы body".into()));
    }
    let mut supported = true;
    for entry in entries.values() {
        let sv = entry["sv"]
            .as_u64()
            .ok_or_else(|| GroupError::Corrupt("нет sv у группы".into()))?;
        supported &= sv <= u64::from(SCHEMA_VERSION);
    }
    Ok(supported
        && candidate.origin == ORIGIN_USER
        && candidate.journal_date == today
        && now_ms.saturating_sub(candidate.created_at) <= APPEND_WINDOW_MS)
}

/// Текст записи после дописывания: новый абзац (`\n\n`), потому что переводы строк в ленте склеивают строки (`breaks: true`).
pub fn joined_body(old_body_md: &str, addition: &str) -> String {
    format!("{old_body_md}\n\n{addition}")
}

/// Текущее состояние записи, нужное для перезаписи `body`.
#[derive(Debug, Clone, Copy)]
pub struct CurrentBody<'a> {
    pub groups_raw: &'a str,
    pub hlc_json: &'a str,
    /// Версия подтверждена сервером (`server_known = 1`) или пришла извне.
    pub server_known: bool,
}

/// Запись после перезаписи `body`: значения для колонок и новые `groups_raw` / `hlc_json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RewrittenBody {
    pub body_md: String,
    pub edited_at: i64,
    pub body_base: Option<String>,
    /// JSON-массив HLC для колонки `body_supersedes`.
    pub body_supersedes: String,
    pub groups_raw: String,
    pub hlc_json: String,
    /// Максимум `sv` по группам (ADR-0001 §4).
    pub schema_version: u32,
}

fn corrupt(why: impl ToString) -> GroupError {
    GroupError::Corrupt(why.to_string())
}

/// Перезаписывает группу `body` с новым HLC (ADR-0001 §3.3: клиент пишет каноничные байты только для группы, которую сам
/// правит). Остальные группы — известные и неизвестные — и неизвестные поля самой `body` сохраняются дословно.
///
/// `base` и `supersedes` — по §3.4:
/// - версию подтвердил сервер или она пришла извне: `base = hlc(текущей)`, `supersedes = []`;
/// - собственная неподтверждённая версия V: `base = V.base`, `supersedes = V.supersedes ∪ {hlc(V)}`, не больше 64 (старейшие
///   отбрасываются).
///
/// Ошибки: повреждённые байты, группа `body` с `sv` выше поддерживаемой, новый HLC не больше прежнего, группа длиннее
/// [`MAX_BODY_GROUP_BYTES`] после склейки.
pub fn rewrite_body(
    current: &CurrentBody<'_>,
    body_md: &str,
    edited_at: i64,
    new_hlc: &str,
) -> Result<RewrittenBody, GroupError> {
    let mut groups = parse_groups_raw(current.groups_raw)?;
    let mut hlcs: BTreeMap<String, Box<RawValue>> =
        serde_json::from_str(current.hlc_json).map_err(corrupt)?;

    let old_group = groups
        .get("body")
        .ok_or_else(|| corrupt("нет группы body"))?;
    let Value::Object(mut body) = serde_json::from_str(old_group.get()).map_err(corrupt)? else {
        return Err(corrupt("группа body не объект"));
    };
    let old_entry: Value = serde_json::from_str(
        hlcs.get("body")
            .ok_or_else(|| corrupt("нет HLC группы body"))?
            .get(),
    )
    .map_err(corrupt)?;
    let old_hlc = old_entry["h"]
        .as_str()
        .ok_or_else(|| corrupt("нет h у группы body"))?
        .to_string();
    let old_sv = old_entry["sv"]
        .as_u64()
        .ok_or_else(|| corrupt("нет sv у группы body"))?;
    if old_sv > u64::from(SCHEMA_VERSION) {
        return Err(GroupError::UnsupportedVersion);
    }
    if new_hlc <= old_hlc.as_str() {
        return Err(GroupError::HlcNotNewer);
    }

    let (base, supersedes) = if current.server_known {
        (Some(old_hlc), Vec::new())
    } else {
        let base = match body.get("base") {
            Some(Value::String(h)) => Some(h.clone()),
            Some(Value::Null) | None => None,
            Some(_) => return Err(corrupt("base группы body")),
        };
        let mut list: Vec<String> = match body.get("supersedes") {
            Some(Value::Array(items)) => items
                .iter()
                .map(|item| item.as_str().map(str::to_string))
                .collect::<Option<_>>()
                .ok_or_else(|| corrupt("supersedes группы body"))?,
            None => Vec::new(),
            Some(_) => return Err(corrupt("supersedes группы body")),
        };
        if !list.contains(&old_hlc) {
            list.push(old_hlc);
        }
        if list.len() > MAX_SUPERSEDES {
            list.drain(..list.len() - MAX_SUPERSEDES);
        }
        (base, list)
    };

    body.insert("body_md".into(), json!(body_md));
    body.insert("edited_at".into(), json!(edited_at));
    body.insert("base".into(), json!(base));
    body.insert("supersedes".into(), json!(supersedes));
    let bytes = jcs::canonical_bytes(&Value::Object(body))?;
    if bytes.len() > MAX_BODY_GROUP_BYTES {
        return Err(GroupError::TooLarge);
    }
    let text = String::from_utf8(bytes).expect("JCS выдаёт UTF-8");
    groups.insert("body".into(), RawValue::from_string(text).map_err(corrupt)?);
    let entry =
        serde_json::to_string(&json!({"h": new_hlc, "sv": SCHEMA_VERSION})).map_err(corrupt)?;
    hlcs.insert(
        "body".into(),
        RawValue::from_string(entry).map_err(corrupt)?,
    );

    let schema_version = hlcs
        .values()
        .filter_map(|entry| {
            serde_json::from_str::<Map<String, Value>>(entry.get())
                .ok()?
                .get("sv")?
                .as_u64()
        })
        .max()
        .map_or(SCHEMA_VERSION, |max| {
            u32::try_from(max).unwrap_or(u32::MAX).max(SCHEMA_VERSION)
        });

    Ok(RewrittenBody {
        body_md: body_md.to_string(),
        edited_at,
        body_supersedes: serde_json::to_string(&supersedes).map_err(corrupt)?,
        body_base: base,
        groups_raw: serde_json::to_string(&groups).map_err(corrupt)?,
        hlc_json: serde_json::to_string(&hlcs).map_err(corrupt)?,
        schema_version,
    })
}

#[cfg(test)]
mod tests {
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

    fn current<'a>(row: &'a crate::groups::MessageRow, server_known: bool) -> CurrentBody<'a> {
        CurrentBody {
            groups_raw: &row.groups_raw,
            hlc_json: &row.hlc_json,
            server_known,
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
        let can =
            |c: &AppendCandidate<'_>, now: i64, today: &str| can_append(c, now, today).unwrap();
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
                server_known: false,
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
                    server_known: false,
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
            &serde_json::from_str::<Value>(&body_group(&groups_raw)).unwrap()["supersedes"]
                .to_string(),
        )
        .unwrap();
        assert_eq!(list.len(), MAX_SUPERSEDES);
        assert_eq!(list.last(), Some(&hlc(1_760_000_000_000 + 69)));
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
                r#"{"base":null,"body_md":"Идея","edited_at":null,"flair":"x","supersedes":[]}"#
                    .into(),
            )
            .unwrap(),
        );
        let groups_raw = serde_json::to_string(&groups).unwrap();
        let mut hlcs: BTreeMap<String, Box<RawValue>> =
            serde_json::from_str(&row.hlc_json).unwrap();
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
                server_known: false,
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
        let hlc_after: BTreeMap<String, Box<RawValue>> =
            serde_json::from_str(&out.hlc_json).unwrap();
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
                    server_known: false,
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
                    server_known: false
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
}
