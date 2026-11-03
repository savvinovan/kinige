//! Rewriting one group of an entry or a chat (ADR-0001 §3.3, §3.4, §4). No IO: there is no database or clock here, the caller
//! passes the rows from the database and the new HLC.
//!
//! Rules common to all groups:
//! - canonical bytes are written only for the group being edited, and only with a new HLC strictly greater than the previous one;
//! - the other groups, unknown groups and unknown fields of the group itself are kept verbatim;
//! - the `hlc_json` record of the group itself is rebuilt: `{"h": new HLC, "sv": 1}`; the records of other groups (with extra fields)
//!   are not touched;
//! - a group whose `sv` is higher than supported is not rewritten ([`GroupError::UnsupportedVersion`]); corrupt bytes are an error.

use crate::groups::{GroupError, SCHEMA_VERSION, parse_groups_raw};
use crate::hlc;
use crate::jcs;
use serde_json::{Map, Value, json, value::RawValue};
use std::collections::BTreeMap;

/// The limit of `supersedes` in the `body` and `placement` groups (ADR-0001 §3.4): the oldest are dropped.
pub const MAX_SUPERSEDES: usize = 64;

/// What you need to know about an entry or a chat to rewrite one of its groups.
#[derive(Debug, Clone, Copy)]
pub struct Groups<'a> {
    pub groups_raw: &'a str,
    pub hlc_json: &'a str,
}

/// The result of a rewrite: the new `groups_raw`, `hlc_json` and `schema_version` (the maximum `sv` over the groups, ADR-0001 §4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rewritten {
    pub groups_raw: String,
    pub hlc_json: String,
    pub schema_version: u32,
}

pub(crate) fn corrupt(why: impl ToString) -> GroupError {
    GroupError::Corrupt(why.to_string())
}

/// `base` and `supersedes` for rewriting `body` or `placement` per ADR-0001 §3.4.
///
/// The version is confirmed (`confirmed`): `base = hlc(current)`, `supersedes = []`. Own unconfirmed version V:
/// `base = V.base`, `supersedes = V.supersedes ∪ {hlc(V)}`, at most [`MAX_SUPERSEDES`] (the oldest are dropped).
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
        Some(_) => return Err(corrupt("group base")),
    };
    let mut list: Vec<String> = match object.get("supersedes") {
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| item.as_str().map(str::to_string))
            .collect::<Option<_>>()
            .ok_or_else(|| corrupt("group supersedes"))?,
        None => Vec::new(),
        Some(_) => return Err(corrupt("group supersedes")),
    };
    if !list.iter().any(|h| h == old_hlc) {
        list.push(old_hlc.to_string());
    }
    if list.len() > MAX_SUPERSEDES {
        // The smallest HLCs (the oldest versions) are dropped, not the first by position: a foreign write may have stored the list
        // out of order. Fixed-width stamps are compared as strings (ADR-0001 §2).
        list.sort();
        list.drain(..list.len() - MAX_SUPERSEDES);
    }
    Ok((base, list))
}

/// Writes `base` and `supersedes` into the group object.
pub fn set_chain(object: &mut Map<String, Value>, base: &Option<String>, supersedes: &[String]) {
    object.insert("base".into(), json!(base));
    object.insert("supersedes".into(), json!(supersedes));
}

/// Rewrites the group `group` with a new HLC. `edit` receives the group object (with unknown fields) and the HLC of its previous
/// version; `max_bytes` is the limit of the group's canonical bytes.
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
        .ok_or_else(|| corrupt(format!("no group {group}")))?;
    let Value::Object(mut object) = serde_json::from_str(old_group.get()).map_err(corrupt)? else {
        return Err(corrupt(format!("group {group} is not an object")));
    };
    let old_entry: Value = serde_json::from_str(
        hlcs.get(group)
            .ok_or_else(|| corrupt(format!("no HLC for group {group}")))?
            .get(),
    )
    .map_err(corrupt)?;
    let old_hlc = old_entry
        .get("h")
        .and_then(Value::as_str)
        .ok_or_else(|| corrupt(format!("group {group} has no h")))?
        .to_string();
    let old_sv = old_entry
        .get("sv")
        .and_then(Value::as_u64)
        .ok_or_else(|| corrupt(format!("group {group} has no sv")))?;
    if old_sv > u64::from(SCHEMA_VERSION) {
        return Err(GroupError::UnsupportedVersion);
    }
    // Stamps are compared as strings only after strict parsing: a corrupt stamp is an error, not "greater" or "smaller".
    hlc::parse(&old_hlc).map_err(|e| corrupt(format!("HLC of group {group}: {e}")))?;
    hlc::parse(new_hlc).map_err(|e| corrupt(format!("new HLC: {e}")))?;
    if new_hlc <= old_hlc.as_str() {
        return Err(GroupError::HlcNotNewer);
    }

    edit(&mut object, &old_hlc)?;

    let text = jcs::canonical_string(&Value::Object(object))?;
    if max_bytes.is_some_and(|max| text.len() > max) {
        return Err(GroupError::TooLarge);
    }
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

/// A read-only entry: at least one of its groups has an `sv` higher than supported (ADR-0001 §4 item 4). A corrupt
/// `hlc_json` (does not parse, a group has no `sv`) is an error, not "allowed".
pub fn is_read_only(hlc_json: &str) -> Result<bool, GroupError> {
    let entries = serde_json::from_str::<BTreeMap<String, Value>>(hlc_json).map_err(corrupt)?;
    let mut read_only = false;
    for entry in entries.values() {
        let sv = entry["sv"]
            .as_u64()
            .ok_or_else(|| corrupt("group has no sv"))?;
        read_only |= sv > u64::from(SCHEMA_VERSION);
    }
    Ok(read_only)
}

/// The value of a group as an object (from `groups_raw`).
pub fn group_object(groups_raw: &str, group: &str) -> Result<Map<String, Value>, GroupError> {
    let groups = parse_groups_raw(groups_raw)?;
    let raw = groups
        .get(group)
        .ok_or_else(|| corrupt(format!("no group {group}")))?;
    match serde_json::from_str(raw.get()).map_err(corrupt)? {
        Value::Object(object) => Ok(object),
        _ => Err(corrupt(format!("group {group} is not an object"))),
    }
}

/// An integer or `null` from a group field.
pub fn opt_int(object: &Map<String, Value>, key: &str) -> Result<Option<i64>, GroupError> {
    match object.get(key) {
        Some(Value::Null) | None => Ok(None),
        Some(Value::Number(n)) => n
            .as_i64()
            .map(Some)
            .ok_or_else(|| corrupt(format!("field {key}"))),
        Some(_) => Err(corrupt(format!("field {key}"))),
    }
}

/// A string or `null` from a group field.
pub fn opt_str(object: &Map<String, Value>, key: &str) -> Result<Option<String>, GroupError> {
    match object.get(key) {
        Some(Value::Null) | None => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(corrupt(format!("field {key}"))),
    }
}

/// A required string from a group field.
pub fn req_str(object: &Map<String, Value>, key: &str) -> Result<String, GroupError> {
    opt_str(object, key)?.ok_or_else(|| corrupt(format!("no field {key}")))
}

/// A boolean from a group field; no field means `false`.
pub fn flag(object: &Map<String, Value>, key: &str) -> Result<bool, GroupError> {
    match object.get(key) {
        Some(Value::Bool(b)) => Ok(*b),
        Some(Value::Null) | None => Ok(false),
        Some(_) => Err(corrupt(format!("field {key}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hlc(l: u64) -> String {
        format!("{l:013}-00000-5f0c3e2a-9b1d-4c7e-8a64-0d2f1b3c4e5f")
    }

    #[test]
    fn overflowing_supersedes_drops_the_smallest_hlc_not_the_first_by_position() {
        // A list written out of order by a foreign client: smaller stamps in the middle and at the end.
        let mut list: Vec<String> = (0..MAX_SUPERSEDES as u64)
            .map(|i| hlc(10_000 - i))
            .collect();
        list.push(hlc(5)); // the oldest is at the end
        let object: Map<String, Value> = serde_json::from_value(json!({
            "base": null,
            "supersedes": list,
        }))
        .unwrap();
        let (base, chain) = chain_fields(&object, &hlc(20_000), false).unwrap();
        assert_eq!(base, None);
        assert_eq!(chain.len(), MAX_SUPERSEDES);
        assert!(!chain.contains(&hlc(5)), "the smallest stamp is dropped");
        assert!(
            !chain.contains(&hlc(10_000 - 63)),
            "and the next one by age"
        );
        assert!(chain.contains(&hlc(20_000)), "the new stamp is in place");
        assert!(
            chain.contains(&hlc(10_000)),
            "the newest of the old ones stayed"
        );
    }

    #[test]
    fn a_short_chain_keeps_its_order() {
        let object: Map<String, Value> =
            serde_json::from_value(json!({"base": null, "supersedes": [hlc(9), hlc(3)]})).unwrap();
        let (_, chain) = chain_fields(&object, &hlc(20), false).unwrap();
        assert_eq!(chain, [hlc(9), hlc(3), hlc(20)]);
    }
}
