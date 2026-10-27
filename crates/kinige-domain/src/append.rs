//! Дописывание к последней записи (ТЗ §5.1, §9.3, слайс 2): правило «куда дописывать» и перезапись группы `body`
//! (ADR-0001 §3.3, §3.4, §4). Без IO: записи и часов здесь нет, вызывающий передаёт строки из базы и новый HLC.
//! Перезапись `body` используют и дописывание, и правка из ленты (слайс 3); общий движок — `rewrite`.

use crate::groups::{GroupError, MAX_BODY_GROUP_BYTES, ORIGIN_USER, SCHEMA_VERSION};
pub use crate::rewrite::MAX_SUPERSEDES;
use crate::rewrite::{Groups, chain_fields, rewrite_group, set_chain};
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// Окно дописывания: запись не старше десяти минут (ТЗ §2.1, §9.3).
pub const APPEND_WINDOW_MS: i64 = 10 * 60 * 1000;

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
    let mut chain = (None, Vec::new());
    let rewritten = rewrite_group(
        &Groups {
            groups_raw: current.groups_raw,
            hlc_json: current.hlc_json,
        },
        "body",
        new_hlc,
        Some(MAX_BODY_GROUP_BYTES),
        |body, old_hlc| {
            chain = chain_fields(body, old_hlc, current.body_confirmed)?;
            body.insert("body_md".into(), json!(body_md));
            body.insert("edited_at".into(), json!(edited_at));
            set_chain(body, &chain.0, &chain.1);
            Ok(())
        },
    )?;
    Ok(RewrittenBody {
        body_md: body_md.to_string(),
        edited_at,
        body_supersedes: serde_json::to_string(&chain.1)
            .map_err(|e| GroupError::Corrupt(e.to_string()))?,
        body_base: chain.0,
        groups_raw: rewritten.groups_raw,
        hlc_json: rewritten.hlc_json,
        schema_version: rewritten.schema_version,
    })
}

#[cfg(test)]
mod tests;
