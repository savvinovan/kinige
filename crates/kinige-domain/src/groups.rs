//! Field groups of a new entry and chat (ADR-0001 §3.2, appendix §A.0): every entry is created at once with all groups,
//! each with its own HLC and `sv = 1`. Table columns and the groups' canonical bytes are built from the same values, so
//! they cannot diverge (invariant `canonical(columns) == groups_raw[g]`, appendix §B item 38).

use crate::hlc::is_uuid_v4;
use crate::jcs::{self, JcsError};
use crate::journal::is_valid_journal_date;
use serde_json::{Map, Value, json, value::RawValue};
use std::collections::BTreeMap;
use std::fmt;

/// The `sv` of the groups this client writes (ADR-0001 §4).
pub const SCHEMA_VERSION: u32 = 1;

/// Inbox is one constant for all accounts (ADR-0001, decision 2); it is identified by id, not by a flag (§3.6).
/// The value is in UUID v4 format (version 4, variant 8), chosen by a human and fixed in the ADR-0001 appendix, §C.
pub const INBOX_ID: &str = "00000000-0000-4000-8000-000000000001";
/// The Inbox emoji in `meta`: chat data, the user will be able to change it in slice 3.
pub const INBOX_ICON: &str = "📥";
/// The Inbox order key (fractional index, PRD §4.2): first in the list.
pub const INBOX_SORT_KEY: &str = "a0";
/// The limit of the `body` group in canonical JCS bytes: the envelope is capped at 1 MiB before padding (ADR-0001 §5.3), the rest of the space
/// is for the other groups. The group's bytes are counted, not the text length: escaping (`"` → `\"`, line break → `\n`)
/// doubles the size, and a 512 KiB text could produce a 1 MiB group.
pub const MAX_BODY_GROUP_BYTES: usize = 512 * 1024;
/// Who created the entry: the interface and quick capture (ADR-0001, "origin").
pub const ORIGIN_USER: &str = "user";
/// Who created the entry: an AI agent over MCP (ADR-0009). A new value of the immutable `core.origin`, additive, `sv` does not grow.
pub const ORIGIN_AGENT: &str = "agent";

/// Group assembly errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupError {
    Jcs(JcsError),
    /// `groups_raw` or `hlc_json` is corrupt.
    Corrupt(String),
    /// The group to be rewritten has an `sv` above the supported one (ADR-0001 §4 item 4): the entry is read-only.
    UnsupportedVersion,
    /// The new HLC is not greater than the previous one: rewriting a group requires a strictly greater one (ADR-0001 §3.3).
    HlcNotNewer,
    /// The group is longer than [`MAX_BODY_GROUP_BYTES`] in canonical bytes.
    TooLarge,
    /// The request makes no sense for this entry or chat (the date does not exist, the name is empty, "done" on a non-task).
    Invalid(&'static str),
}

impl fmt::Display for GroupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GroupError::Jcs(error) => write!(f, "{error}"),
            GroupError::Corrupt(why) => write!(f, "entry groups are corrupt: {why}"),
            GroupError::UnsupportedVersion => f.write_str("group version is unsupported"),
            GroupError::HlcNotNewer => f.write_str("new HLC is not above the previous one"),
            GroupError::TooLarge => f.write_str("the group is longer than the limit"),
            GroupError::Invalid(why) => write!(f, "invalid request: {why}"),
        }
    }
}

impl std::error::Error for GroupError {}

impl From<JcsError> for GroupError {
    fn from(error: JcsError) -> Self {
        GroupError::Jcs(error)
    }
}

/// The reason an entry text was not accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyError {
    /// Only whitespace characters.
    Empty,
    /// The `body` group is longer than [`MAX_BODY_GROUP_BYTES`] in canonical bytes.
    TooLarge,
}

/// Entry text before saving: line breaks are normalized to `\n`, leading empty lines and trailing spaces are removed
/// (leading spaces of the first non-empty line stay: the indent may be significant), empty and too long text is an error.
pub fn normalize_body(raw: &str) -> Result<String, BodyError> {
    let unified = raw.replace("\r\n", "\n").replace('\r', "\n");
    let mut lines = unified
        .split('\n')
        .skip_while(|line| line.trim().is_empty());
    let first = lines.next();
    let Some(first) = first else {
        return Err(BodyError::Empty);
    };
    let mut text = String::from(first);
    for line in lines {
        text.push('\n');
        text.push_str(line);
    }
    let text = text.trim_end().to_string();
    if text.is_empty() {
        return Err(BodyError::Empty);
    }
    let group = jcs::canonical_bytes(&body_group(&text)).map_err(|_| BodyError::TooLarge)?;
    if group.len() > MAX_BODY_GROUP_BYTES {
        return Err(BodyError::TooLarge);
    }
    Ok(text)
}

/// Like [`normalize_body`], but a text of only whitespace gives `""` instead of `Empty` (appendix §C.6: an empty body is valid
/// exactly when the entry has a live attachment; the caller checks that, in the same transaction).
pub fn normalize_body_or_empty(raw: &str) -> Result<String, BodyError> {
    match normalize_body(raw) {
        Err(BodyError::Empty) => Ok(String::new()),
        other => other,
    }
}

/// The value of a new entry's `body` group (one place for both the size check and the group's bytes).
fn body_group(body_md: &str) -> Value {
    json!({"base": null, "body_md": body_md, "edited_at": null, "supersedes": []})
}

/// The initial values of a new entry; the text is already normalized.
pub struct NewMessage<'a> {
    pub id: &'a str,
    pub chat_id: &'a str,
    pub body_md: &'a str,
    pub created_at: i64,
    pub created_tz: &'a str,
    pub journal_date: &'a str,
}

/// A whole row of the `message` table: the parsed columns and the groups' raw bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageRow {
    pub id: String,
    pub chat_id: String,
    pub body_md: String,
    pub journal_date: String,
    pub created_at: i64,
    pub created_tz: String,
    pub edited_at: Option<i64>,
    pub is_task: bool,
    pub task_done_at: Option<i64>,
    pub favorite_at: Option<i64>,
    pub pinned_at: Option<i64>,
    pub reply_to_id: Option<String>,
    pub origin: String,
    pub ext_json: Option<String>,
    pub deleted_at: Option<i64>,
    pub body_base: Option<String>,
    /// A JSON array of HLCs, `[]` for a new entry.
    pub body_supersedes: String,
    pub placement_base: Option<String>,
    pub placement_supersedes: String,
    /// `{"<group>":{"h":"HLC","sv":1}}`.
    pub hlc_json: String,
    pub schema_version: u32,
    /// `{"<group>":<canonical bytes>}` -- the source for re-sending (ADR-0001 §C).
    pub groups_raw: String,
}

/// The entry's provenance -- what makes the `core` group differ between a user's entry and an imported one (slice 4).
pub(crate) struct Provenance<'a> {
    pub origin: &'a str,
    pub reply_to_id: Option<&'a str>,
    /// The external import id (`ext`), an optional immutable `core` field.
    pub ext: Option<Value>,
    /// The entry is created as a task right away: the `flags` group with `is_task = true` (agent only, ADR-0009).
    pub is_task: bool,
}

/// A new entry at once with all five groups (`core`, `body`, `flags`, `placement`, `deleted`) under one HLC: it is created as one event.
pub fn new_message(input: &NewMessage<'_>, hlc: &str) -> Result<MessageRow, GroupError> {
    let user = Provenance {
        origin: ORIGIN_USER,
        reply_to_id: None,
        ext: None,
        is_task: false,
    };
    build_message(input, &user, hlc)
}

/// A user's reply to an entry (slice 5): the same five groups under one HLC, `core.reply_to_id` is the id of the old entry. The
/// `core` group is immutable: the link is neither edited nor removed. `reply_to_id` must be a lower-case UUID v4.
pub fn new_reply_message(
    input: &NewMessage<'_>,
    reply_to_id: &str,
    hlc: &str,
) -> Result<MessageRow, GroupError> {
    if !is_uuid_v4(reply_to_id) {
        return Err(GroupError::Invalid("reply_to_id is not a UUID v4"));
    }
    let reply = Provenance {
        origin: ORIGIN_USER,
        reply_to_id: Some(reply_to_id),
        ext: None,
        is_task: false,
    };
    build_message(input, &reply, hlc)
}

/// A new agent entry (ADR-0009): the same five groups under one HLC, `origin = 'agent'`, without `ext`. With `task = true` the
/// `flags` group is created right away with `is_task = true` -- otherwise the agent would have to rewrite the group right after creation.
pub fn new_agent_message(
    input: &NewMessage<'_>,
    task: bool,
    hlc: &str,
) -> Result<MessageRow, GroupError> {
    let agent = Provenance {
        origin: ORIGIN_AGENT,
        reply_to_id: None,
        ext: None,
        is_task: task,
    };
    build_message(input, &agent, hlc)
}

/// Shared assembly of a new entry: five groups under one HLC and the columns from the same values. `new_message` and
/// `import::new_imported_message` differ only in [`Provenance`].
pub(crate) fn build_message(
    input: &NewMessage<'_>,
    provenance: &Provenance<'_>,
    hlc: &str,
) -> Result<MessageRow, GroupError> {
    // `journal_date` goes into the `placement` group and into the indexes: an impossible date must not reach the sync bytes.
    if !is_valid_journal_date(input.journal_date) {
        return Err(GroupError::Invalid("journal_date"));
    }
    let mut core = Map::new();
    core.insert("created_at".to_string(), json!(input.created_at));
    core.insert("created_tz".to_string(), json!(input.created_tz));
    core.insert("origin".to_string(), json!(provenance.origin));
    core.insert("reply_to_id".to_string(), json!(provenance.reply_to_id));
    if let Some(ext) = &provenance.ext {
        core.insert("ext".to_string(), ext.clone());
    }
    let ext_json = provenance
        .ext
        .as_ref()
        .map(jcs::canonical_string)
        .transpose()?;
    let groups: [(&str, Value); 5] = [
        ("core", Value::Object(core)),
        ("body", body_group(input.body_md)),
        (
            "flags",
            json!({"favorite_at": null, "is_task": provenance.is_task, "pinned_at": null, "task_done_at": null}),
        ),
        (
            "placement",
            json!({"base": null, "chat_id": input.chat_id, "journal_date": input.journal_date, "supersedes": []}),
        ),
        ("deleted", json!({"deleted_at": null})),
    ];
    let (groups_raw, hlc_json) = assemble(&groups, hlc)?;
    Ok(MessageRow {
        id: input.id.to_string(),
        chat_id: input.chat_id.to_string(),
        body_md: input.body_md.to_string(),
        journal_date: input.journal_date.to_string(),
        created_at: input.created_at,
        created_tz: input.created_tz.to_string(),
        edited_at: None,
        is_task: provenance.is_task,
        task_done_at: None,
        favorite_at: None,
        pinned_at: None,
        reply_to_id: provenance.reply_to_id.map(str::to_string),
        origin: provenance.origin.to_string(),
        ext_json,
        deleted_at: None,
        body_base: None,
        body_supersedes: "[]".to_string(),
        placement_base: None,
        placement_supersedes: "[]".to_string(),
        hlc_json,
        schema_version: SCHEMA_VERSION,
        groups_raw,
    })
}

/// A whole row of the `chat` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatRow {
    pub id: String,
    pub title: String,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub sort_key: String,
    pub is_inbox: bool,
    pub ai_excluded: bool,
    pub pinned_at: Option<i64>,
    pub archived_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
    pub hlc_json: String,
    pub schema_version: u32,
    pub groups_raw: String,
}

/// Inbox: a constant id and a constant `core` group (ADR-0001 §3.6) -- `created_at = 0`, `is_inbox = true`, so that two
/// Inboxes created before linking merge without conflict. The stored name is empty: the interface shows the translated one.
pub fn inbox_chat(hlc: &str, now_ms: i64) -> Result<ChatRow, GroupError> {
    let groups: [(&str, Value); 6] = [
        ("core", json!({"created_at": 0, "is_inbox": true})),
        (
            "meta",
            json!({"color": null, "icon": INBOX_ICON, "title": ""}),
        ),
        ("order", json!({"sort_key": INBOX_SORT_KEY})),
        ("state", json!({"archived_at": null, "pinned_at": null})),
        ("privacy", json!({"ai_excluded": false})),
        ("deleted", json!({"deleted_at": null})),
    ];
    let (groups_raw, hlc_json) = assemble(&groups, hlc)?;
    Ok(ChatRow {
        id: INBOX_ID.to_string(),
        title: String::new(),
        icon: Some(INBOX_ICON.to_string()),
        color: None,
        sort_key: INBOX_SORT_KEY.to_string(),
        is_inbox: true,
        ai_excluded: false,
        pinned_at: None,
        archived_at: None,
        created_at: 0,
        updated_at: now_ms,
        deleted_at: None,
        hlc_json,
        schema_version: SCHEMA_VERSION,
        groups_raw,
    })
}

/// Groups from `groups_raw` with verbatim bytes (for the invariant check and for merging).
pub fn parse_groups_raw(groups_raw: &str) -> Result<BTreeMap<String, Box<RawValue>>, GroupError> {
    serde_json::from_str(groups_raw).map_err(|e| GroupError::Corrupt(e.to_string()))
}

/// Builds `groups_raw` (the groups' canonical bytes verbatim) and `hlc_json` (one HLC for all groups of the event).
pub(crate) fn assemble(
    groups: &[(&str, Value)],
    hlc: &str,
) -> Result<(String, String), GroupError> {
    let mut raw: BTreeMap<String, Box<RawValue>> = BTreeMap::new();
    let mut hlcs: BTreeMap<&str, Value> = BTreeMap::new();
    for (name, value) in groups {
        let text = jcs::canonical_string(value)?;
        raw.insert(
            (*name).to_string(),
            RawValue::from_string(text).map_err(|e| GroupError::Corrupt(e.to_string()))?,
        );
        hlcs.insert(name, json!({"h": hlc, "sv": SCHEMA_VERSION}));
    }
    let groups_raw = serde_json::to_string(&raw).map_err(|e| GroupError::Corrupt(e.to_string()))?;
    let hlc_json = serde_json::to_string(&hlcs).map_err(|e| GroupError::Corrupt(e.to_string()))?;
    Ok((groups_raw, hlc_json))
}

#[cfg(test)]
mod tests;
