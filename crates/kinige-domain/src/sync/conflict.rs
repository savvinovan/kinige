//! Text and move conflicts (ADR-0001 §3.4): when the losing version of a message's `body` or `placement` must be kept as a
//! `conflict` revision, and what that revision is. A revision's id and content are identical on every device, so creating it twice,
//! on two devices, is idempotent.

use super::record::{Group, SUPPORTED_SV};
use super::{SyncError, corrupt};
use crate::hlc;
use crate::jcs;
use serde_json::{Value, json};
use std::fmt::Write as _;

pub const BODY: &str = "body";
pub const PLACEMENT: &str = "placement";
/// The revision's `reason` for a lost concurrent version (an `edit` revision is local and not a sync rule).
pub const REASON_CONFLICT: &str = "conflict";

/// `1` for `body`, `2` for `placement`: the byte in the input of the revision id (§1). Other groups have no revisions: `flags` is
/// last-writer-wins without a trace (decision 9).
pub fn group_code(group: &str) -> Option<u8> {
    match group {
        BODY => Some(1),
        PLACEMENT => Some(2),
        _ => None,
    }
}

/// `base`, `supersedes` and `vv` of a `body` / `placement` group (§3.4).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Lineage {
    /// The HLC of the version the edit was made from; `None` for the first version, treated as -infinity.
    pub base: Option<String>,
    pub supersedes: Vec<String>,
    /// S4a, 2026-11-05: for each device, the newest HLC of that device's versions in this version's ancestry, this version included
    /// (one entry per device, see [`super::lineage::next_vv`]). `None` for a version written before `vv` existed.
    pub vv: Option<Vec<String>>,
}

impl Lineage {
    /// Reads the fields out of a group's bytes. A group that does not parse, or lacks them, gets the empty lineage, which
    /// makes a loser look concurrent: an extra revision is the safe side, a lost version is not. A `vv` that is not a list of strings
    /// is ignored the same way.
    pub fn parse(raw: &str) -> Lineage {
        let Ok(Value::Object(object)) = serde_json::from_str::<Value>(raw) else {
            return Lineage::default();
        };
        let base = object
            .get("base")
            .and_then(Value::as_str)
            .map(str::to_string);
        let strings = |value: &Value| -> Option<Vec<String>> {
            match value {
                Value::Array(items) => items
                    .iter()
                    .map(|item| item.as_str().map(str::to_string))
                    .collect(),
                _ => None,
            }
        };
        // Items that are not strings are dropped from `supersedes` (the old behaviour); a `vv` with one is not trusted at all.
        let supersedes = match object.get("supersedes") {
            Some(Value::Array(items)) => items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect(),
            _ => Vec::new(),
        };
        let vv = object.get("vv").and_then(strings);
        Lineage {
            base,
            supersedes,
            vv,
        }
    }
}

/// The `conflict` revision of the losing version P of `group` of message `message_id` (§3.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictRevision {
    /// `message_id[16] || group[1] || hlc(P)[56]`: the input of `crypto_generichash(K_rev, ...)` whose first 16 bytes, with the
    /// version and variant bits of a v4 UUID ([`uuid_from_bytes`]), are the revision id (§1). The hash is S4b's: this crate has
    /// no keys.
    pub id_input: Vec<u8>,
    /// The revision's `core` group: HLC = `hlc(P)`, so two devices that create it write the same bytes with the same stamp.
    pub core: Group,
}

/// Whether the loser P must become a `conflict` revision of the winner W (§3.4). A client that does not support the `sv` of either
/// version makes none (another device will, §3.4); it cannot read what it would write.
///
/// P and W are concurrent when their contents differ and P is not in W's history. Three tests, in this order:
/// 1. `hlc(P)` is in `W.supersedes`, or is `W.base`: W was made from P (also when the ack for P was lost and P still counts as
///    unconfirmed): no revision.
/// 2. P was written by another device than W and W has a `vv` (S4a): P is in W's history exactly when W's `vv` has an entry of P's
///    device that is not below `hlc(P)`. Exact; it does not depend on which intermediate envelopes the server still has.
/// 3. Otherwise (the same device wrote both: a profile clone, or an old-format W without `vv`): `hlc(P) > W.base` (no base counts as
///    -infinity), and **the device's own unconfirmed version loses -- always a revision**, for any relation to `W.base`.
pub fn needs_conflict_revision(
    group: &str,
    winner: &Group,
    loser: &Group,
    loser_is_own_unconfirmed: bool,
) -> bool {
    if group_code(group).is_none()
        || winner.sv > SUPPORTED_SV
        || loser.sv > SUPPORTED_SV
        || !content_differs(group, &winner.raw, &loser.raw)
    {
        return false;
    }
    let lineage = Lineage::parse(&winner.raw);
    if lineage.supersedes.contains(&loser.hlc)
        || lineage.base.as_deref() == Some(loser.hlc.as_str())
    {
        return false;
    }
    if let Some(vv) = &lineage.vv {
        let (Some(author_p), Some(author_w)) = (device_of(&loser.hlc), device_of(&winner.hlc))
        else {
            return true;
        };
        if author_p != author_w {
            let seen = vv.iter().filter(|h| device_of(h) == Some(author_p)).max();
            return seen.is_none_or(|seen| *seen < loser.hlc);
        }
    }
    loser_is_own_unconfirmed || lineage.base.as_ref().is_none_or(|base| loser.hlc > *base)
}

/// The device id of a strict HLC, `None` for anything else.
pub(crate) fn device_of(stamp: &str) -> Option<&str> {
    hlc::parse(stamp).ok().map(|(_, device)| device)
}

/// Whether two versions of `body` / `placement` say different things. `base`, `supersedes` and `edited_at` are service fields and
/// do not count. Bytes that do not parse count as different.
pub fn content_differs(group: &str, a: &str, b: &str) -> bool {
    let fields: &[&str] = match group {
        BODY => &["body_md"],
        PLACEMENT => &["chat_id", "journal_date"],
        _ => return a != b,
    };
    let (Ok(Value::Object(a)), Ok(Value::Object(b))) = (
        serde_json::from_str::<Value>(a),
        serde_json::from_str::<Value>(b),
    ) else {
        return true;
    };
    fields.iter().any(|field| a.get(*field) != b.get(*field))
}

/// Makes the revision of the losing version P of `group` of message `message_id`. A version with an `sv` this client does not
/// support cannot be written into a revision (ADR-0001 §3.4: "if it supports the `sv` of both versions"): another device will.
pub fn conflict_revision(
    message_id: &str,
    group: &str,
    loser: &Group,
) -> Result<ConflictRevision, SyncError> {
    if loser.sv > SUPPORTED_SV {
        return Err(SyncError::UnsupportedVersion);
    }
    let code = group_code(group).ok_or_else(|| corrupt("no revisions for this group"))?;
    let mut id_input = uuid_bytes(message_id)
        .ok_or_else(|| corrupt("message id is not a UUID"))?
        .to_vec();
    id_input.push(code);
    id_input.extend_from_slice(loser.hlc.as_bytes());

    let (clock, _) = hlc::parse(&loser.hlc)?;
    // `raw` is a JSON string with the exact bytes of the group, not a nested object: re-canonicalizing by another client would
    // give other bytes for an equal HLC (§3.4).
    let snapshot = jcs::canonical_string(
        &json!({"g": group, "h": loser.hlc, "raw": loser.raw, "sv": loser.sv}),
    )
    .map_err(corrupt)?;
    let core = jcs::canonical_string(&json!({
        "message_id": message_id,
        "reason": REASON_CONFLICT,
        "saved_at": clock.l,
        "snapshot_json": snapshot,
    }))
    .map_err(corrupt)?;
    Ok(ConflictRevision {
        id_input,
        core: Group::new(&loser.hlc, SUPPORTED_SV, &core)?,
    })
}

/// A lower-case UUID as 16 bytes.
pub fn uuid_bytes(text: &str) -> Option<[u8; 16]> {
    if !hlc::is_uuid(text) {
        return None;
    }
    let digits: Vec<u8> = text
        .bytes()
        .filter(|b| *b != b'-')
        .map(|b| {
            if b.is_ascii_digit() {
                b - b'0'
            } else {
                b - b'a' + 10
            }
        })
        .collect();
    let mut out = [0u8; 16];
    for (byte, pair) in out.iter_mut().zip(digits.chunks(2)) {
        *byte = pair.iter().fold(0, |acc, d| (acc << 4) | d);
    }
    Some(out)
}

/// 16 bytes as a lower-case UUID with the version (4) and variant bits of a v4 UUID set, as ADR-0001 §1 says for the revision id.
pub fn uuid_from_bytes(mut bytes: [u8; 16]) -> String {
    if let Some(byte) = bytes.get_mut(6) {
        *byte = (*byte & 0x0f) | 0x40;
    }
    if let Some(byte) = bytes.get_mut(8) {
        *byte = (*byte & 0x3f) | 0x80;
    }
    let mut out = String::with_capacity(36);
    for (i, byte) in bytes.iter().enumerate() {
        if matches!(i, 4 | 6 | 8 | 10) {
            out.push('-');
        }
        // Writing to a String cannot fail.
        let _ = write!(out, "{byte:02x}");
    }
    out
}
