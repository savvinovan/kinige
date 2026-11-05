//! A record as the sync rules see it: a set of field groups, each with its own HLC, `sv` and verbatim canonical bytes
//! (ADR-0001 §3.1, §3.3). The same data the app stores as `groups_raw` + `hlc_json`, parsed once.

use super::{SyncError, corrupt};
use crate::groups::parse_groups_raw;
use crate::hlc;
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use std::collections::BTreeMap;

/// A snapshot of HLCs, `{"<group>":"HLC"}`: the `purged_of` of a `purged` envelope and of a tombstone (ADR-0001 §3.5).
pub type Snapshot = BTreeMap<String, String>;

/// The newest `sv` this client writes and understands (ADR-0001 §4); a record with a group above it is read-only.
pub const SUPPORTED_SV: u32 = crate::groups::SCHEMA_VERSION;

/// One field group: its HLC, its `sv` and its exact bytes. Someone else's group is stored and resent verbatim (§3.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    /// A strict stamp (`hlc::parse` accepted it), so comparing the strings is comparing `(wall_ms, counter, device_id)`.
    pub hlc: String,
    pub sv: u32,
    /// Canonical JSON bytes (RFC 8785) of the group value, kept as they arrived.
    pub raw: String,
}

impl Group {
    /// A group from foreign parts. The stamp must be strict and the bytes must be one JSON value.
    pub fn new(hlc: &str, sv: u32, raw: &str) -> Result<Group, SyncError> {
        hlc::parse(hlc)?;
        serde_json::from_str::<&RawValue>(raw).map_err(|_| corrupt("group is not JSON"))?;
        Ok(Group {
            hlc: hlc.to_string(),
            sv,
            raw: raw.to_string(),
        })
    }
}

/// The groups of one record (a chat, a message, a revision), by group name. Unknown names are kept like known ones (§4 item 1).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Versioned {
    groups: BTreeMap<String, Group>,
}

#[derive(Serialize, Deserialize)]
struct HlcEntry {
    h: String,
    sv: u32,
}

impl Versioned {
    pub fn new() -> Versioned {
        Versioned::default()
    }

    /// From the columns the app writes. The two texts must name the same groups; every stamp must be strict.
    pub fn from_columns(groups_raw: &str, hlc_json: &str) -> Result<Versioned, SyncError> {
        let raw = parse_groups_raw(groups_raw).map_err(corrupt)?;
        let entries: BTreeMap<String, HlcEntry> =
            serde_json::from_str(hlc_json).map_err(corrupt)?;
        if raw.len() != entries.len() {
            return Err(corrupt("groups_raw and hlc_json name different groups"));
        }
        let mut record = Versioned::new();
        for (name, value) in &raw {
            let entry = entries
                .get(name)
                .ok_or_else(|| corrupt("group without HLC"))?;
            record.set(name, Group::new(&entry.h, entry.sv, value.get())?);
        }
        Ok(record)
    }

    /// `(groups_raw, hlc_json)` in the exact form of [`crate::groups::assemble`].
    pub fn to_columns(&self) -> Result<(String, String), SyncError> {
        let mut raw: BTreeMap<&str, &RawValue> = BTreeMap::new();
        let mut hlcs: BTreeMap<&str, HlcEntry> = BTreeMap::new();
        let mut owned = Vec::with_capacity(self.groups.len());
        for (name, group) in &self.groups {
            owned.push((
                name.as_str(),
                RawValue::from_string(group.raw.clone()).map_err(corrupt)?,
            ));
        }
        for (name, value) in &owned {
            raw.insert(name, value);
        }
        for (name, group) in &self.groups {
            hlcs.insert(
                name,
                HlcEntry {
                    h: group.hlc.clone(),
                    sv: group.sv,
                },
            );
        }
        Ok((
            serde_json::to_string(&raw).map_err(corrupt)?,
            serde_json::to_string(&hlcs).map_err(corrupt)?,
        ))
    }

    pub fn get(&self, name: &str) -> Option<&Group> {
        self.groups.get(name)
    }

    /// Writes (or replaces) a group. The caller has already decided that this version should be there.
    pub fn set(&mut self, name: &str, group: Group) {
        self.groups.insert(name.to_string(), group);
    }

    pub fn groups(&self) -> impl Iterator<Item = (&str, &Group)> {
        self.groups
            .iter()
            .map(|(name, group)| (name.as_str(), group))
    }

    pub fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }

    /// The HLCs of all groups, as the device sees them: what a `purged` envelope carries (§3.5).
    pub fn snapshot(&self) -> Snapshot {
        self.groups
            .iter()
            .map(|(name, group)| (name.clone(), group.hlc.clone()))
            .collect()
    }

    /// A record's `schema_version`: the maximum `sv` over its groups (§4).
    pub fn schema_version(&self) -> u32 {
        self.groups.values().map(|g| g.sv).max().unwrap_or(0)
    }

    /// A group with an `sv` above the supported one makes the record read-only: no `upsert` is created for it (§4 item 4).
    pub fn is_read_only(&self) -> bool {
        self.schema_version() > SUPPORTED_SV
    }
}
