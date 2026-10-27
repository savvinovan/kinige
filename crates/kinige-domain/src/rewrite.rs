//! Перезапись одной группы записи или чата (ADR-0001 §3.3, §3.4, §4). Без IO: записи и часов здесь нет, вызывающий
//! передаёт строки из базы и новый HLC.
//!
//! Правила, общие для всех групп:
//! - каноничные байты пишутся только для группы, которую правят, и только с новым HLC строго больше прежнего;
//! - остальные группы, неизвестные группы и неизвестные поля самой группы сохраняются дословно;
//! - запись в `hlc_json` самой группы пересобирается: `{"h": новый HLC, "sv": 1}`; записи других групп (с лишними полями)
//!   не трогаются;
//! - группа в `sv` выше поддерживаемой не переписывается ([`GroupError::UnsupportedVersion`]), повреждённые байты — ошибка.

use crate::groups::{GroupError, SCHEMA_VERSION, parse_groups_raw};
use crate::hlc;
use crate::jcs;
use serde_json::{Map, Value, json, value::RawValue};
use std::collections::BTreeMap;

/// Предел `supersedes` в группах `body` и `placement` (ADR-0001 §3.4): старейшие отбрасываются.
pub const MAX_SUPERSEDES: usize = 64;

/// Что нужно знать о записи или чате, чтобы переписать одну из их групп.
#[derive(Debug, Clone, Copy)]
pub struct Groups<'a> {
    pub groups_raw: &'a str,
    pub hlc_json: &'a str,
}

/// Результат перезаписи: новые `groups_raw`, `hlc_json` и `schema_version` (максимум `sv` по группам, ADR-0001 §4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rewritten {
    pub groups_raw: String,
    pub hlc_json: String,
    pub schema_version: u32,
}

pub(crate) fn corrupt(why: impl ToString) -> GroupError {
    GroupError::Corrupt(why.to_string())
}

/// `base` и `supersedes` для перезаписи `body` или `placement` по ADR-0001 §3.4.
///
/// Версия подтверждена (`confirmed`): `base = hlc(текущей)`, `supersedes = []`. Собственная неподтверждённая версия V:
/// `base = V.base`, `supersedes = V.supersedes ∪ {hlc(V)}`, не больше [`MAX_SUPERSEDES`] (старейшие отбрасываются).
pub fn chain_fields(
    object: &Map<String, Value>,
    old_hlc: &str,
    confirmed: bool,
) -> Result<(Option<String>, Vec<String>), GroupError> {
    if confirmed {
        return Ok((Some(old_hlc.to_string()), Vec::new()));
    }
    let base = match object.get("base") {
        Some(Value::String(h)) => Some(h.clone()),
        Some(Value::Null) | None => None,
        Some(_) => return Err(corrupt("base группы")),
    };
    let mut list: Vec<String> = match object.get("supersedes") {
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| item.as_str().map(str::to_string))
            .collect::<Option<_>>()
            .ok_or_else(|| corrupt("supersedes группы"))?,
        None => Vec::new(),
        Some(_) => return Err(corrupt("supersedes группы")),
    };
    if !list.iter().any(|h| h == old_hlc) {
        list.push(old_hlc.to_string());
    }
    if list.len() > MAX_SUPERSEDES {
        list.drain(..list.len() - MAX_SUPERSEDES);
    }
    Ok((base, list))
}

/// Записывает в объект группы `base` и `supersedes`.
pub fn set_chain(object: &mut Map<String, Value>, base: &Option<String>, supersedes: &[String]) {
    object.insert("base".into(), json!(base));
    object.insert("supersedes".into(), json!(supersedes));
}

/// Перезаписывает группу `group` с новым HLC. `edit` получает объект группы (с неизвестными полями) и HLC её прежней
/// версии; `max_bytes` — предел каноничных байтов группы.
pub fn rewrite_group(
    current: &Groups<'_>,
    group: &str,
    new_hlc: &str,
    max_bytes: Option<usize>,
    edit: impl FnOnce(&mut Map<String, Value>, &str) -> Result<(), GroupError>,
) -> Result<Rewritten, GroupError> {
    let mut groups = parse_groups_raw(current.groups_raw)?;
    let mut hlcs: BTreeMap<String, Box<RawValue>> =
        serde_json::from_str(current.hlc_json).map_err(corrupt)?;

    let old_group = groups
        .get(group)
        .ok_or_else(|| corrupt(format!("нет группы {group}")))?;
    let Value::Object(mut object) = serde_json::from_str(old_group.get()).map_err(corrupt)? else {
        return Err(corrupt(format!("группа {group} не объект")));
    };
    let old_entry: Value = serde_json::from_str(
        hlcs.get(group)
            .ok_or_else(|| corrupt(format!("нет HLC группы {group}")))?
            .get(),
    )
    .map_err(corrupt)?;
    let old_hlc = old_entry["h"]
        .as_str()
        .ok_or_else(|| corrupt(format!("нет h у группы {group}")))?
        .to_string();
    let old_sv = old_entry["sv"]
        .as_u64()
        .ok_or_else(|| corrupt(format!("нет sv у группы {group}")))?;
    if old_sv > u64::from(SCHEMA_VERSION) {
        return Err(GroupError::UnsupportedVersion);
    }
    // Метки сравниваются как строки только после строгого разбора: повреждённая метка — ошибка, а не «больше» или «меньше».
    hlc::parse(&old_hlc).map_err(|e| corrupt(format!("HLC группы {group}: {e}")))?;
    hlc::parse(new_hlc).map_err(|e| corrupt(format!("новый HLC: {e}")))?;
    if new_hlc <= old_hlc.as_str() {
        return Err(GroupError::HlcNotNewer);
    }

    edit(&mut object, &old_hlc)?;

    let bytes = jcs::canonical_bytes(&Value::Object(object))?;
    if max_bytes.is_some_and(|max| bytes.len() > max) {
        return Err(GroupError::TooLarge);
    }
    let text = String::from_utf8(bytes).expect("JCS выдаёт UTF-8");
    groups.insert(
        group.to_string(),
        RawValue::from_string(text).map_err(corrupt)?,
    );
    let entry =
        serde_json::to_string(&json!({"h": new_hlc, "sv": SCHEMA_VERSION})).map_err(corrupt)?;
    hlcs.insert(
        group.to_string(),
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

    Ok(Rewritten {
        groups_raw: serde_json::to_string(&groups).map_err(corrupt)?,
        hlc_json: serde_json::to_string(&hlcs).map_err(corrupt)?,
        schema_version,
    })
}

/// Запись только для чтения: хотя бы одна её группа в `sv` выше поддерживаемой (ADR-0001 §4 п. 4). Повреждённый
/// `hlc_json` (не разбирается, у группы нет `sv`) — ошибка, а не «можно».
pub fn is_read_only(hlc_json: &str) -> Result<bool, GroupError> {
    let entries = serde_json::from_str::<BTreeMap<String, Value>>(hlc_json).map_err(corrupt)?;
    let mut read_only = false;
    for entry in entries.values() {
        let sv = entry["sv"]
            .as_u64()
            .ok_or_else(|| corrupt("нет sv у группы"))?;
        read_only |= sv > u64::from(SCHEMA_VERSION);
    }
    Ok(read_only)
}

/// Значение группы как объект (по `groups_raw`).
pub fn group_object(groups_raw: &str, group: &str) -> Result<Map<String, Value>, GroupError> {
    let groups = parse_groups_raw(groups_raw)?;
    let raw = groups
        .get(group)
        .ok_or_else(|| corrupt(format!("нет группы {group}")))?;
    match serde_json::from_str(raw.get()).map_err(corrupt)? {
        Value::Object(object) => Ok(object),
        _ => Err(corrupt(format!("группа {group} не объект"))),
    }
}

/// Целое или `null` из поля группы.
pub fn opt_int(object: &Map<String, Value>, key: &str) -> Result<Option<i64>, GroupError> {
    match object.get(key) {
        Some(Value::Null) | None => Ok(None),
        Some(Value::Number(n)) => n
            .as_i64()
            .map(Some)
            .ok_or_else(|| corrupt(format!("поле {key}"))),
        Some(_) => Err(corrupt(format!("поле {key}"))),
    }
}

/// Строка или `null` из поля группы.
pub fn opt_str(object: &Map<String, Value>, key: &str) -> Result<Option<String>, GroupError> {
    match object.get(key) {
        Some(Value::Null) | None => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(corrupt(format!("поле {key}"))),
    }
}

/// Обязательная строка из поля группы.
pub fn req_str(object: &Map<String, Value>, key: &str) -> Result<String, GroupError> {
    opt_str(object, key)?.ok_or_else(|| corrupt(format!("нет поля {key}")))
}

/// Булево из поля группы; нет поля — `false`.
pub fn flag(object: &Map<String, Value>, key: &str) -> Result<bool, GroupError> {
    match object.get(key) {
        Some(Value::Bool(b)) => Ok(*b),
        Some(Value::Null) | None => Ok(false),
        Some(_) => Err(corrupt(format!("поле {key}"))),
    }
}
