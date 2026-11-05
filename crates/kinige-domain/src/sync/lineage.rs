//! The `vv` field of a `body` / `placement` group (ADR-0001 §3.4, S4a 2026-11-05): for each device, the newest HLC of that device's
//! versions in the ancestry of this version. It is computed from the version being edited alone, so it needs no extra local state,
//! and it is additive (no `sv` bump). A reader asks one question of it: "has the author of this version seen P?" -- yes exactly
//! when `vv` holds an entry of P's device that is not below `hlc(P)`.

use super::conflict::{Lineage, device_of};
use super::record::Group;
use super::{SyncError, corrupt};
use crate::jcs;
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// At most this many devices are remembered; the ones with the oldest versions drop out first, and a dropped device's versions look
/// concurrent, which is an extra revision, never a lost version.
pub const MAX_VV: usize = 16;

/// The `vv` of a new version `new_hlc` written by editing `parent` (the version this device has now).
///
/// An old-format parent has no `vv`: it is rebuilt from what its bytes say (its own HLC, `base`, `supersedes`), which says less and so
/// can only make later losers look concurrent. A parent with an unusable `vv` is treated the same way.
pub fn next_vv(parent: Option<&Group>, new_hlc: &str) -> Vec<String> {
    let mut newest: BTreeMap<&str, &str> = BTreeMap::new();
    let lineage = parent
        .map(|group| Lineage::parse(&group.raw))
        .unwrap_or_default();
    let known: Vec<&str> = match (&lineage.vv, parent) {
        (Some(vv), _) => vv.iter().map(String::as_str).collect(),
        (None, Some(group)) => std::iter::once(group.hlc.as_str())
            .chain(lineage.base.as_deref())
            .chain(lineage.supersedes.iter().map(String::as_str))
            .collect(),
        (None, None) => Vec::new(),
    };
    for stamp in known.into_iter().chain(std::iter::once(new_hlc)) {
        let Some(device) = device_of(stamp) else {
            continue;
        };
        let slot = newest.entry(device).or_insert(stamp);
        if stamp > *slot {
            *slot = stamp;
        }
    }
    let mut list: Vec<String> = newest.into_values().map(str::to_string).collect();
    if list.len() > MAX_VV {
        // Never drop the version itself: keep the newest ones.
        list.sort();
        list.drain(..list.len() - MAX_VV);
    }
    list.sort();
    list
}

/// `raw` (a group object, canonical JSON) with `vv` set, re-canonicalized. Everything else in the group, including fields this
/// client does not know, is kept.
pub fn with_vv(raw: &str, vv: &[String]) -> Result<String, SyncError> {
    let Value::Object(mut object) = serde_json::from_str::<Value>(raw).map_err(corrupt)? else {
        return Err(corrupt("group is not an object"));
    };
    object.insert("vv".to_string(), json!(vv));
    jcs::canonical_string(&Value::Object(object)).map_err(corrupt)
}
