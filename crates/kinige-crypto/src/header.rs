//! The open header and the encrypted payload of an envelope (ADR-0001 §5.2, §5.3). The envelope layer authenticates the header's
//! bytes as they are, so the format of the JSON is checked only after the AEAD has passed (§A.4 step 5): everything here is a
//! parse of authentic bytes from a possibly faulty client, and a failure is `Malformed`, which the caller quarantines.

use crate::error::CryptoError;
use crate::strict_json::check_no_duplicate_keys;
use kinige_domain::hlc;
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use std::collections::BTreeMap;

/// The longest name of an entity or a group; both are short ASCII identifiers.
const MAX_NAME: usize = 32;

/// What the server sees of an operation (§5.2). Unknown fields of the JSON are ignored: the bytes are authenticated anyway, and a
/// new open field is additive (§4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Header {
    pub entity: String,
    pub id: String,
    pub op: String,
    pub kind: Kind,
    /// `group -> {h, sv}` of an `upsert`; absent for `purged`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub groups: BTreeMap<String, GroupStamp>,
    /// The snapshot of HLCs of a `purged` (§3.5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub purged_of: Option<BTreeMap<String, String>>,
    /// `remote_blob_id` of a purged attachment (§5.2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blob: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Upsert,
    Purged,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupStamp {
    pub h: String,
    pub sv: u32,
}

impl Header {
    /// The header's bytes for [`crate::envelope::seal`]: JSON without whitespace, keys in struct order. A header that does not pass
    /// [`Header::validate`] is not written.
    pub fn to_bytes(&self) -> Result<Vec<u8>, CryptoError> {
        self.validate().map_err(CryptoError::Malformed)?;
        serde_json::to_vec(self).map_err(|_| CryptoError::Input("header does not serialize"))
    }

    /// Parses authentic header bytes: no duplicate keys, the right types, and the rules of [`Header::validate`].
    pub fn parse(bytes: &[u8]) -> Result<Header, CryptoError> {
        check_no_duplicate_keys(bytes).map_err(CryptoError::Malformed)?;
        let header: Header =
            serde_json::from_slice(bytes).map_err(|_| CryptoError::Malformed("header fields"))?;
        header.validate().map_err(CryptoError::Malformed)?;
        Ok(header)
    }

    /// `entity` and group names are short lower-case identifiers, `id` and `op` lower-case UUIDs, every HLC a strict stamp, every
    /// `sv` at least 1. An `upsert` has groups and no snapshot; a `purged` has a snapshot and no groups; `blob` belongs only to a
    /// purged attachment.
    pub fn validate(&self) -> Result<(), &'static str> {
        if !is_name(&self.entity) {
            return Err("entity");
        }
        if !hlc::is_uuid(&self.id) || !hlc::is_uuid(&self.op) {
            return Err("id or op");
        }
        for (name, stamp) in &self.groups {
            if !is_name(name) || stamp.sv == 0 || hlc::parse(&stamp.h).is_err() {
                return Err("group stamp");
            }
        }
        match self.kind {
            Kind::Upsert if self.groups.is_empty() => return Err("upsert without groups"),
            Kind::Upsert if self.purged_of.is_some() || self.blob.is_some() => {
                return Err("upsert with purge fields");
            }
            Kind::Purged if !self.groups.is_empty() => return Err("purged with groups"),
            Kind::Purged => {
                let snapshot = self
                    .purged_of
                    .as_ref()
                    .filter(|snapshot| !snapshot.is_empty());
                let snapshot = snapshot.ok_or("purged without a snapshot")?;
                if snapshot
                    .iter()
                    .any(|(name, stamp)| !is_name(name) || hlc::parse(stamp).is_err())
                {
                    return Err("snapshot stamp");
                }
            }
            Kind::Upsert => {}
        }
        match &self.blob {
            Some(blob) if self.entity != "attachment" || !hlc::is_uuid(blob) => Err("blob"),
            _ => Ok(()),
        }
    }
}

/// The payload of an `upsert` (§5.3): `{"groups":{"<g>":<the group's bytes>}}`. Each value is inserted as it is (§3.3), which
/// must be a JSON object without duplicate keys; the groups must be exactly those of the header. A `purged` has no payload bytes.
pub fn build_payload(
    header: &Header,
    groups: &BTreeMap<String, String>,
) -> Result<Vec<u8>, CryptoError> {
    if header.kind == Kind::Purged {
        return if groups.is_empty() {
            Ok(Vec::new())
        } else {
            Err(CryptoError::Input("purged has no payload"))
        };
    }
    if !groups.keys().eq(header.groups.keys()) {
        return Err(CryptoError::Input("payload groups differ from the header"));
    }
    let mut out = String::from("{\"groups\":{");
    for (index, (name, raw)) in groups.iter().enumerate() {
        if !is_object(raw.as_bytes()) {
            return Err(CryptoError::Input("a group is not a json object"));
        }
        if index > 0 {
            out.push(',');
        }
        out.push('"');
        out.push_str(name);
        out.push_str("\":");
        out.push_str(raw);
    }
    out.push_str("}}");
    Ok(out.into_bytes())
}

/// Parses the decrypted payload against its header (§A.4 step 5): duplicate keys are refused, only `groups` is allowed at the top,
/// the set of groups equals `header.groups`, and a `purged` has none. Returns the groups' exact bytes.
pub fn parse_payload(
    payload: &[u8],
    header: &Header,
) -> Result<BTreeMap<String, String>, CryptoError> {
    if header.kind == Kind::Purged {
        return if payload.is_empty() {
            Ok(BTreeMap::new())
        } else {
            Err(CryptoError::Malformed("purged with a payload"))
        };
    }
    check_no_duplicate_keys(payload).map_err(CryptoError::Malformed)?;
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Payload {
        groups: BTreeMap<String, Box<RawValue>>,
    }
    let parsed: Payload =
        serde_json::from_slice(payload).map_err(|_| CryptoError::Malformed("payload shape"))?;
    if !parsed.groups.keys().eq(header.groups.keys()) {
        return Err(CryptoError::Malformed(
            "payload groups differ from the header",
        ));
    }
    let mut out = BTreeMap::new();
    for (name, raw) in parsed.groups {
        if !is_object(raw.get().as_bytes()) {
            return Err(CryptoError::Malformed("a group is not a json object"));
        }
        out.insert(name, raw.get().to_string());
    }
    Ok(out)
}

fn is_name(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= MAX_NAME
        && text
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// A group's bytes are a JSON object without repeated keys (the canonical JSON of §A.0).
fn is_object(raw: &[u8]) -> bool {
    raw.first() == Some(&b'{') && check_no_duplicate_keys(raw).is_ok()
}
