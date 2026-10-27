//! Ревизии `edit` и снимки для tombstones (слайс 3, ADR-0001 §3.4, §3.5, приложение §A.0 и §C). Без IO.
//!
//! Ревизия `edit` — полный снимок записи **до** правки: дословные байты всех групп (известных и неизвестных) с их HLC и `sv`,
//! `raw` строкой — так же, как у `conflict`, чтобы переканонизация на другом клиенте не меняла байты при равном HLC.
//! Сама ревизия — запись только с группой `core` (`message_id`, `snapshot_json`, `saved_at`, `reason`).

use crate::groups::{GroupError, SCHEMA_VERSION, assemble, parse_groups_raw};
use crate::jcs;
use crate::rewrite::corrupt;
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// Причина ревизии при правке текста.
pub const REASON_EDIT: &str = "edit";
/// Срок корзины: через 30 дней запись удаляется физически (ТЗ §4.1).
pub const TRASH_DAYS: i64 = 30;
/// Миллисекунд в сутках.
pub const DAY_MS: i64 = 24 * 60 * 60 * 1000;

/// Строка таблицы `message_revision` целиком.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevisionRow {
    pub id: String,
    pub message_id: String,
    pub snapshot_json: String,
    pub saved_at: i64,
    pub reason: String,
    pub hlc_json: String,
    pub schema_version: u32,
    pub groups_raw: String,
}

/// Снимок записи: `{"groups":[{"g","h","sv","raw"},…]}`, группы по имени. Нет `hlc_json` у группы — повреждение.
pub fn edit_snapshot(groups_raw: &str, hlc_json: &str) -> Result<String, GroupError> {
    let groups = parse_groups_raw(groups_raw)?;
    let hlcs: BTreeMap<String, Value> = serde_json::from_str(hlc_json).map_err(corrupt)?;
    let mut entries = Vec::with_capacity(groups.len());
    for (name, raw) in &groups {
        let entry = hlcs
            .get(name)
            .ok_or_else(|| corrupt(format!("нет HLC группы {name}")))?;
        let h = entry["h"]
            .as_str()
            .ok_or_else(|| corrupt(format!("нет h у группы {name}")))?;
        let sv = entry["sv"]
            .as_u64()
            .ok_or_else(|| corrupt(format!("нет sv у группы {name}")))?;
        entries.push(json!({"g": name, "h": h, "sv": sv, "raw": raw.get()}));
    }
    let bytes = jcs::canonical_bytes(&json!({ "groups": entries }))?;
    Ok(String::from_utf8(bytes).expect("JCS выдаёт UTF-8"))
}

/// Ревизия `edit` со снимком записи до правки. `hlc` — метка события правки (та же, что у новой версии `body`).
pub fn edit_revision(
    id: &str,
    message_id: &str,
    groups_raw: &str,
    hlc_json: &str,
    saved_at: i64,
    hlc: &str,
) -> Result<RevisionRow, GroupError> {
    let snapshot_json = edit_snapshot(groups_raw, hlc_json)?;
    let snapshot: Value = serde_json::from_str(&snapshot_json).map_err(corrupt)?;
    let core = json!({
        "message_id": message_id,
        "reason": REASON_EDIT,
        "saved_at": saved_at,
        "snapshot_json": snapshot,
    });
    let (groups_raw, hlc_json) = assemble(&[("core", core)], hlc)?;
    Ok(RevisionRow {
        id: id.to_string(),
        message_id: message_id.to_string(),
        snapshot_json,
        saved_at,
        reason: REASON_EDIT.to_string(),
        hlc_json,
        schema_version: SCHEMA_VERSION,
        groups_raw,
    })
}

/// `purged_of` для tombstone: снимок HLC **всех** групп сущности, как их видит устройство, включая неизвестные
/// (`{"<группа>":"HLC"}`, ADR-0001 §3.5). Группа, которой нет в снимке, считается новее его.
pub fn purged_of(hlc_json: &str) -> Result<String, GroupError> {
    let hlcs: BTreeMap<String, Value> = serde_json::from_str(hlc_json).map_err(corrupt)?;
    if hlcs.is_empty() {
        return Err(corrupt("пустой hlc_json"));
    }
    let mut snapshot = serde_json::Map::new();
    for (name, entry) in &hlcs {
        let h = entry["h"]
            .as_str()
            .ok_or_else(|| corrupt(format!("нет h у группы {name}")))?;
        snapshot.insert(name.clone(), json!(h));
    }
    let bytes = jcs::canonical_bytes(&Value::Object(snapshot))?;
    Ok(String::from_utf8(bytes).expect("JCS выдаёт UTF-8"))
}

/// То же, что [`purged_of`], но для записи с частично повреждённым `hlc_json`: в снимок попадает всё, что удалось прочитать
/// (группы с разбираемой меткой `h`). Группа, которой нет в снимке, считается новее его (ADR-0001 §3.5), поэтому неполный
/// снимок только делает будущий серверный purge осторожнее, но запись можно удалить локально. Не объект JSON и пустой
/// результат — ошибка: снимка нет вовсе.
pub fn purged_of_best_effort(hlc_json: &str) -> Result<String, GroupError> {
    let hlcs: BTreeMap<String, Value> = serde_json::from_str(hlc_json).map_err(corrupt)?;
    let mut snapshot = serde_json::Map::new();
    for (name, entry) in &hlcs {
        if let Some(h) = entry.get("h").and_then(Value::as_str) {
            snapshot.insert(name.clone(), json!(h));
        }
    }
    if snapshot.is_empty() {
        return Err(corrupt("в hlc_json нет ни одной читаемой группы"));
    }
    let bytes = jcs::canonical_bytes(&Value::Object(snapshot))?;
    Ok(String::from_utf8(bytes).expect("JCS выдаёт UTF-8"))
}

/// Группа `deleted` записи в поддерживаемой `sv`: purge (локальный и серверный) создавать можно только так (ADR-0001 §4 п. 4).
/// Нет записи о группе или она не читается — «можно»: решить, что запись новее, нечем, а удалить повреждённую человек
/// вправе.
pub fn deleted_group_supported(hlc_json: &str) -> bool {
    serde_json::from_str::<BTreeMap<String, Value>>(hlc_json)
        .ok()
        .and_then(|hlcs| hlcs.get("deleted")?.get("sv")?.as_u64())
        .is_none_or(|sv| sv <= u64::from(SCHEMA_VERSION))
}

/// Через сколько целых дней запись из корзины удалится сама: от 30 в день удаления до 0 в последний день; не меньше 0.
pub fn days_left(deleted_at_ms: i64, now_ms: i64) -> i64 {
    let expires = deleted_at_ms.saturating_add(TRASH_DAYS * DAY_MS);
    let left = expires.saturating_sub(now_ms);
    // Неполный день считается как день: «осталось 1 дн.» до самого конца срока.
    ((left + DAY_MS - 1).div_euclid(DAY_MS)).max(0)
}

/// Пора ли удалять запись из корзины навсегда: прошло 30 суток (`deleted_at ≤ now − 30 дней`).
pub fn is_expired(deleted_at_ms: i64, now_ms: i64) -> bool {
    deleted_at_ms <= now_ms.saturating_sub(TRASH_DAYS * DAY_MS)
}
