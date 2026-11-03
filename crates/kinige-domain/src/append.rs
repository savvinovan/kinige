//! Appending to the last entry (PRD §5.1, §9.3, slice 2): the "where to append" rule and rewriting the `body` group
//! (ADR-0001 §3.3, §3.4, §4). No IO: there is no database or clock here, the caller passes the rows from the database and the new HLC.
//! Both appending and editing from the feed (slice 3) use the `body` rewrite; the shared engine is `rewrite`.

use crate::groups::{GroupError, MAX_BODY_GROUP_BYTES, ORIGIN_USER, SCHEMA_VERSION};
pub use crate::rewrite::MAX_SUPERSEDES;
use crate::rewrite::{Groups, chain_fields, rewrite_group, set_chain};
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// The append window: the entry is not older than ten minutes (PRD §2.1, §9.3).
pub const APPEND_WINDOW_MS: i64 = 10 * 60 * 1000;

/// What you need to know about the last non-deleted Inbox entry to decide whether it can be appended to.
#[derive(Debug, Clone, Copy)]
pub struct AppendCandidate<'a> {
    pub origin: &'a str,
    pub created_at: i64,
    pub journal_date: &'a str,
    /// The entry's `hlc_json`: the `sv` of every group is checked against it.
    pub hlc_json: &'a str,
}

/// Whether the entry can be appended to: it was created by the user (`origin = 'user'`; agent and import entries are not appended to, §9.3),
/// it was created no more than ten minutes ago, it belongs to today's journal day, and all its groups have a supported
/// `sv` (ADR-0001 §4 item 4: an entry with a newer group is read-only). A corrupt `hlc_json` (does not parse, no
/// `body` group, a group has no `sv`) is an error, not "no": the entry is not skipped silently, the corruption is visible.
pub fn can_append(
    candidate: &AppendCandidate<'_>,
    now_ms: i64,
    today: &str,
) -> Result<bool, GroupError> {
    can_append_origin(candidate, ORIGIN_USER, now_ms, today)
}

/// The same rule for an entry with a given origin: the agent (ADR-0009) appends only to an entry with `origin = 'agent'`,
/// the user only to `origin = 'user'`. "Own" entry is decided by `origin`, not by the MCP client name.
pub fn can_append_origin(
    candidate: &AppendCandidate<'_>,
    expected_origin: &str,
    now_ms: i64,
    today: &str,
) -> Result<bool, GroupError> {
    let entries = serde_json::from_str::<BTreeMap<String, Value>>(candidate.hlc_json)
        .map_err(|e| GroupError::Corrupt(e.to_string()))?;
    if !entries.contains_key("body") {
        return Err(GroupError::Corrupt("no HLC for group body".into()));
    }
    let mut supported = true;
    for entry in entries.values() {
        let sv = entry["sv"]
            .as_u64()
            .ok_or_else(|| GroupError::Corrupt("group has no sv".into()))?;
        supported &= sv <= u64::from(SCHEMA_VERSION);
    }
    Ok(supported
        && candidate.origin == expected_origin
        && candidate.journal_date == today
        && now_ms.saturating_sub(candidate.created_at) <= APPEND_WINDOW_MS)
}

/// The entry text after appending: a new paragraph (`\n\n`), because line breaks in the feed join lines (`breaks: true`).
/// An entry that has no text yet (a photo sent alone, appendix §C.6) gets the addition as is, with no leading blank lines.
pub fn joined_body(old_body_md: &str, addition: &str) -> String {
    if old_body_md.is_empty() {
        return addition.to_string();
    }
    format!("{old_body_md}\n\n{addition}")
}

/// The current state of an entry needed to rewrite `body`.
#[derive(Debug, Clone, Copy)]
pub struct CurrentBody<'a> {
    pub groups_raw: &'a str,
    pub hlc_json: &'a str,
    /// The current version of the `body` group is **confirmed by the server or came from outside** (ADR-0001 §3.4: "the version that the
    /// server confirmed is edited"). Unconfirmed means "an outbox row without `acked_seq` or `server_known = 0`": **the caller
    /// must take both signs into account** (slices 1-10 have no outbox, and the `server_known` column is enough; once the outbox appears
    /// (slice 11), a version with an unconfirmed outbox row is unconfirmed even if `server_known = 1`).
    pub body_confirmed: bool,
}

/// An entry after rewriting `body`: values for the columns and the new `groups_raw` / `hlc_json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RewrittenBody {
    pub body_md: String,
    pub edited_at: i64,
    pub body_base: Option<String>,
    /// A JSON array of HLCs for the `body_supersedes` column.
    pub body_supersedes: String,
    pub groups_raw: String,
    pub hlc_json: String,
    /// The maximum `sv` over the groups (ADR-0001 §4).
    pub schema_version: u32,
}

/// Rewrites the `body` group with a new HLC (ADR-0001 §3.3: the client writes canonical bytes only for the group it itself
/// edits). The other groups -- known and unknown -- and the unknown fields of `body` itself are kept verbatim.
///
/// `base` and `supersedes` are per §3.4:
/// - the version is confirmed (`body_confirmed`, see [`CurrentBody`]): `base = hlc(current)`, `supersedes = []`;
/// - own unconfirmed version V: `base = V.base`, `supersedes = V.supersedes ∪ {hlc(V)}`, at most 64 (the oldest
///   are dropped).
///
/// Errors: corrupt bytes, a `body` group with `sv` higher than supported, a new HLC not greater than the previous one, a group longer than
/// [`MAX_BODY_GROUP_BYTES`] after joining.
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
