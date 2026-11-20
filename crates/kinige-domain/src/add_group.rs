//! Adding a group that an older record does not have (ADR-0001 appendix §A.0 and §C.8, slice 7b; slice 5a reuses it). It is the one
//! exception to "a missing group is an error, not a re-creation" ([`crate::rewrite::rewrite_group`]): only the groups on the
//! allowlist below may be absent, and only an edit of that very group adds it. No code path writes a default-valued group to an
//! existing row: two devices that each added all of them would let one device's rename uncover the other's hidden photo.
//!
//! The rules:
//! - the two maps must name the same groups (any difference, in any group, is `Corrupt`); "absent" means absent from both;
//! - the new group's HLC is strictly greater than every HLC of the record (every entry parsed strictly);
//! - the other groups, unknown groups and the extra fields of the other `hlc_json` entries stay byte-identical.

use crate::groups::{GroupError, SCHEMA_VERSION, parse_groups_raw};
use crate::hlc;
use crate::jcs;
use crate::rewrite::{Groups, Rewritten, corrupt, max_schema_version};
use serde_json::{Value, json, value::RawValue};
use std::collections::BTreeMap;

/// The groups that may be absent from a record, per entity (slice 7b: an attachment's details; slice 5a adds the message's
/// `reactions`). Any other missing group stays an error: an entry without `body` is corrupt.
const MAY_BE_ABSENT: &[(&str, &[&str])] = &[("attachment", &["name", "description", "privacy"])];

/// Adds `group` with the canonical bytes of `value` under `new_hlc` and `sv = 1` to a record of `entity` that does not have it.
/// `max_bytes` limits the group's canonical bytes.
pub fn add_group(
    current: &Groups<'_>,
    entity: &str,
    group: &str,
    value: &Value,
    new_hlc: &str,
    max_bytes: Option<usize>,
) -> Result<Rewritten, GroupError> {
    let allowed = MAY_BE_ABSENT
        .iter()
        .any(|(name, groups)| *name == entity && groups.contains(&group));
    if !allowed {
        return Err(GroupError::Invalid("this group may not be added"));
    }
    let mut groups = parse_groups_raw(current.groups_raw)?;
    let mut hlcs: BTreeMap<String, Box<RawValue>> =
        serde_json::from_str(current.hlc_json).map_err(corrupt)?;
    // The two maps must name the same groups, whatever the group being added: a record that has one in only one of them could never
    // become an envelope (`Versioned::from_columns` refuses it), and adding to it would hide that behind a new group.
    if !groups.keys().eq(hlcs.keys()) {
        return Err(corrupt(
            "groups_raw and hlc_json name different groups".to_string(),
        ));
    }
    if groups.contains_key(group) {
        return Err(GroupError::Invalid("the group is already there"));
    }

    hlc::parse(new_hlc).map_err(|e| corrupt(format!("new HLC: {e}")))?;
    for (name, entry) in &hlcs {
        let entry: Value = serde_json::from_str(entry.get()).map_err(corrupt)?;
        let old = entry
            .get("h")
            .and_then(Value::as_str)
            .ok_or_else(|| corrupt(format!("group {name} has no h")))?;
        hlc::parse(old).map_err(|e| corrupt(format!("HLC of group {name}: {e}")))?;
        if new_hlc <= old {
            return Err(GroupError::HlcNotNewer);
        }
    }

    let text = jcs::canonical_string(value)?;
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
    Ok(Rewritten {
        groups_raw: serde_json::to_string(&groups).map_err(corrupt)?,
        hlc_json: serde_json::to_string(&hlcs).map_err(corrupt)?,
        schema_version: max_schema_version(&hlcs),
    })
}

#[cfg(test)]
mod tests;
