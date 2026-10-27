//! Дописывание к последней записи (ТЗ §5.1, §9.3, слайс 2): правило «куда дописывать» и первая перезапись группы
//! `body` (ADR-0001 §3.3, §3.4, §4). Без IO: записи и часов здесь нет, вызывающий передаёт строки из базы и новый HLC.

use crate::groups::{
    GroupError, MAX_BODY_GROUP_BYTES, ORIGIN_USER, SCHEMA_VERSION, parse_groups_raw,
};
use crate::hlc;
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
    /// Текущая версия группы `body` **подтверждена сервером или пришла извне** (ADR-0001 §3.4: «правится версия, которую
    /// сервер подтвердил»). Неподтверждённая — это «строка outbox без `acked_seq` или `server_known = 0`»: **вызывающий
    /// обязан учесть оба признака** (в слайсах 1–10 outbox нет, и хватает колонки `server_known`; при появлении outbox
    /// (слайс 11) версия с неподтверждённой строкой outbox — неподтверждённая, даже если `server_known = 1`).
    pub body_confirmed: bool,
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
/// - версия подтверждена (`body_confirmed`, см. [`CurrentBody`]): `base = hlc(текущей)`, `supersedes = []`;
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
    // Метки сравниваются как строки только после строгого разбора: повреждённая метка — ошибка, а не «больше» или «меньше».
    hlc::parse(&old_hlc).map_err(|e| corrupt(format!("HLC группы body: {e}")))?;
    hlc::parse(new_hlc).map_err(|e| corrupt(format!("новый HLC: {e}")))?;
    if new_hlc <= old_hlc.as_str() {
        return Err(GroupError::HlcNotNewer);
    }

    let (base, supersedes) = if current.body_confirmed {
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
mod tests;
