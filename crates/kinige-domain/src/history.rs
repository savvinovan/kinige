//! `edit` revisions and snapshots for tombstones (slice 3, ADR-0001 §3.4, §3.5, appendix §A.0 and §C). No IO.
//!
//! An `edit` revision is a full snapshot of the entry **before** the edit: the verbatim bytes of all groups (known and
//! unknown) with their HLC and `sv`, `raw` as a string -- the same as for `conflict`, so that re-canonicalization on
//! another client does not change the bytes at an equal HLC.
//! The revision itself is an entry with the `core` group only (`message_id`, `snapshot_json`, `saved_at`, `reason`).

use crate::groups::{GroupError, SCHEMA_VERSION, assemble, parse_groups_raw};
use crate::jcs;
use crate::rewrite::corrupt;
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// The reason of a revision when the text is edited.
pub const REASON_EDIT: &str = "edit";
/// The trash period: after 30 days the entry is physically deleted (PRD §4.1).
pub const TRASH_DAYS: i64 = 30;
/// Milliseconds in a day.
pub const DAY_MS: i64 = 24 * 60 * 60 * 1000;

/// A whole row of the `message_revision` table.
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

/// The snapshot of an entry: `{"groups":[{"g","h","sv","raw"},…]}`, groups by name. A group without `hlc_json` means corruption.
pub fn edit_snapshot(groups_raw: &str, hlc_json: &str) -> Result<String, GroupError> {
    let groups = parse_groups_raw(groups_raw)?;
    let hlcs: BTreeMap<String, Value> = serde_json::from_str(hlc_json).map_err(corrupt)?;
    let mut entries = Vec::with_capacity(groups.len());
    for (name, raw) in &groups {
        let entry = hlcs
            .get(name)
            .ok_or_else(|| corrupt(format!("no HLC of group {name}")))?;
        let h = entry["h"]
            .as_str()
            .ok_or_else(|| corrupt(format!("no h in group {name}")))?;
        let sv = entry["sv"]
            .as_u64()
            .ok_or_else(|| corrupt(format!("no sv in group {name}")))?;
        entries.push(json!({"g": name, "h": h, "sv": sv, "raw": raw.get()}));
    }
    Ok(jcs::canonical_string(&json!({ "groups": entries }))?)
}

/// An `edit` revision with a snapshot of the entry before the edit. `hlc` is the stamp of the edit event (the same as for the new `body` version).
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

/// `purged_of` for a tombstone: a snapshot of the HLC of **all** groups of the entity as the device sees them, including
/// unknown ones (`{"<group>":"HLC"}`, ADR-0001 §3.5). A group that is not in the snapshot counts as newer than it.
pub fn purged_of(hlc_json: &str) -> Result<String, GroupError> {
    let hlcs: BTreeMap<String, Value> = serde_json::from_str(hlc_json).map_err(corrupt)?;
    if hlcs.is_empty() {
        return Err(corrupt("empty hlc_json"));
    }
    let mut snapshot = serde_json::Map::new();
    for (name, entry) in &hlcs {
        let h = entry["h"]
            .as_str()
            .ok_or_else(|| corrupt(format!("no h in group {name}")))?;
        snapshot.insert(name.clone(), json!(h));
    }
    Ok(jcs::canonical_string(&Value::Object(snapshot))?)
}

/// The same as [`purged_of`], but for an entry with a partly damaged `hlc_json`: the snapshot gets everything that could
/// be read (groups with a parsable stamp `h`). A group that is not in the snapshot counts as newer than it (ADR-0001
/// §3.5), so an incomplete snapshot only makes a future server purge more careful, while the entry can be deleted
/// locally. A non-object JSON and an empty result are an error: there is no snapshot at all.
pub fn purged_of_best_effort(hlc_json: &str) -> Result<String, GroupError> {
    let hlcs: BTreeMap<String, Value> = serde_json::from_str(hlc_json).map_err(corrupt)?;
    let mut snapshot = serde_json::Map::new();
    for (name, entry) in &hlcs {
        if let Some(h) = entry.get("h").and_then(Value::as_str) {
            snapshot.insert(name.clone(), json!(h));
        }
    }
    if snapshot.is_empty() {
        return Err(corrupt("hlc_json has no readable group"));
    }
    Ok(jcs::canonical_string(&Value::Object(snapshot))?)
}

/// The `deleted` group of the entry is in a supported `sv`: a purge (local and server) may only be created this way (ADR-0001 §4 item 4).
/// If there is no record of the group or it cannot be read, the answer is "allowed": there is nothing to decide that the
/// entry is newer with, and a person has the right to delete a damaged one.
pub fn deleted_group_supported(hlc_json: &str) -> bool {
    serde_json::from_str::<BTreeMap<String, Value>>(hlc_json)
        .ok()
        .and_then(|hlcs| hlcs.get("deleted")?.get("sv")?.as_u64())
        .is_none_or(|sv| sv <= u64::from(SCHEMA_VERSION))
}

/// In how many whole days an entry in the trash will be deleted by itself: from 30 on the day of deletion down to 0 on the last day; not below 0.
pub fn days_left(deleted_at_ms: i64, now_ms: i64) -> i64 {
    let expires = deleted_at_ms.saturating_add(TRASH_DAYS * DAY_MS);
    let left = expires.saturating_sub(now_ms);
    // A partial day counts as a day: "1 day left" until the very end of the period.
    ((left + DAY_MS - 1).div_euclid(DAY_MS)).max(0)
}

/// Whether it is time to delete an entry from the trash forever: 30 days have passed (`deleted_at <= now - 30 days`).
pub fn is_expired(deleted_at_ms: i64, now_ms: i64) -> bool {
    deleted_at_ms <= now_ms.saturating_sub(TRASH_DAYS * DAY_MS)
}
