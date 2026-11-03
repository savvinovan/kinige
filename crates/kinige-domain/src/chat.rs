//! Chats (slice 3): a new chat with all six groups, rewriting `meta` (name, emoji) and `state` (pinned, archive),
//! the order key `order.sort_key` and parsing columns from the groups (ADR-0001 §3.2, appendix §A.0). No IO.

use crate::groups::{ChatRow, GroupError, INBOX_ID, SCHEMA_VERSION, assemble};
use crate::rewrite::{
    Groups, Rewritten, corrupt, flag, group_object, opt_int, opt_str, req_str, rewrite_group,
};
use serde_json::{Value, json};
use std::fmt;

/// The default chat emoji (slice 3: "no emoji means 💬").
pub const DEFAULT_ICON: &str = "💬";
/// Chat name length in characters (slice 3: from 1 to 128).
pub const MAX_TITLE_CHARS: usize = 128;
/// The emoji size limit in bytes: any picker sequence fits. The longest in `emojibase-data` 17.0.0 and 18.0.0 is
/// a couple kissing with different skin tones, 35 bytes (the earlier limit of 32 rejected such emoji).
pub const MAX_ICON_BYTES: usize = 40;

/// Why the entered name or emoji was not accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatInputError {
    TitleEmpty,
    TitleTooLong,
    IconInvalid,
}

impl fmt::Display for ChatInputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ChatInputError::TitleEmpty => "chat name is empty",
            ChatInputError::TitleTooLong => "chat name is longer than 128 characters",
            ChatInputError::IconInvalid => "chat emoji is not accepted",
        })
    }
}

impl std::error::Error for ChatInputError {}

/// Chat name: surrounding spaces trimmed, from 1 to 128 characters, no control characters (line breaks and tabs count as such).
pub fn normalize_title(raw: &str) -> Result<String, ChatInputError> {
    let title = raw.trim();
    if title.is_empty() {
        return Err(ChatInputError::TitleEmpty);
    }
    if title.chars().any(char::is_control) {
        return Err(ChatInputError::TitleEmpty);
    }
    if title.chars().count() > MAX_TITLE_CHARS {
        return Err(ChatInputError::TitleTooLong);
    }
    Ok(title.to_string())
}

/// Chat emoji: a non-empty string up to [`MAX_ICON_BYTES`] without spaces and control characters; no value means 💬.
/// The domain does not check that it really is an emoji: the picker yields only emoji, while a foreign client could have written anything, and
/// ADR-0001 §4 requires showing what was written.
pub fn normalize_icon(raw: Option<&str>) -> Result<String, ChatInputError> {
    let Some(icon) = raw.map(str::trim).filter(|icon| !icon.is_empty()) else {
        return Ok(DEFAULT_ICON.to_string());
    };
    if icon.len() > MAX_ICON_BYTES || icon.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err(ChatInputError::IconInvalid);
    }
    Ok(icon.to_string())
}

const KEY_DIGITS: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
const KEY_WIDTH: usize = 6;

/// The order key for a new chat at the end of the list (PRD §4.2: fractional index). A chat key is `a` plus six digits of a
/// base-36 number, so strings compare the same way as numbers; Inbox has `a0` and goes first. A foreign key of another format
/// (another client) gets a prefix: a string that has the previous maximum as its start is larger than it and all smaller ones. Equal
/// keys on two devices are allowed: `id` decides the order (appendix §A.0).
pub fn key_after(max: Option<&str>) -> String {
    let Some(max) = max else {
        return format!("a{}", "0".repeat(KEY_WIDTH - 1) + "1");
    };
    let digits = max.strip_prefix('a').map(str::as_bytes);
    let own = digits.is_some_and(|d| {
        d.len() == KEY_WIDTH && d.iter().all(|b| KEY_DIGITS.contains(b)) && d != [b'z'; KEY_WIDTH]
    });
    if !own {
        // Inbox (`a0`) and foreign formats: take the starting value or append to the maximum.
        return if max == "a0" || max < "a000001" {
            key_after(None)
        } else {
            format!("{max}1")
        };
    }
    let mut next = digits.unwrap_or_default().to_vec();
    for slot in next.iter_mut().rev() {
        let at = KEY_DIGITS
            .iter()
            .position(|d| d == slot)
            .unwrap_or_default();
        if let Some(&digit) = KEY_DIGITS.get(at + 1) {
            *slot = digit;
            break;
        }
        *slot = b'0';
    }
    // All bytes are ASCII digits (checked above), so `char::from(u8)` matches UTF-8.
    let digits: String = next.into_iter().map(char::from).collect();
    format!("a{digits}")
}

/// The initial values of a new chat; the name and emoji are already normalized.
pub struct NewChat<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub icon: &'a str,
    pub sort_key: &'a str,
    pub created_at: i64,
}

/// A new chat at once with all six groups (`core`, `meta`, `order`, `state`, `privacy`, `deleted`) under one HLC, `sv = 1`.
pub fn new_chat(input: &NewChat<'_>, hlc: &str) -> Result<ChatRow, GroupError> {
    let groups: [(&str, Value); 6] = [
        (
            "core",
            json!({"created_at": input.created_at, "is_inbox": false}),
        ),
        (
            "meta",
            json!({"color": null, "icon": input.icon, "title": input.title}),
        ),
        ("order", json!({"sort_key": input.sort_key})),
        ("state", json!({"archived_at": null, "pinned_at": null})),
        ("privacy", json!({"ai_excluded": false})),
        ("deleted", json!({"deleted_at": null})),
    ];
    let (groups_raw, hlc_json) = assemble(&groups, hlc)?;
    Ok(ChatRow {
        id: input.id.to_string(),
        title: input.title.to_string(),
        icon: Some(input.icon.to_string()),
        color: None,
        sort_key: input.sort_key.to_string(),
        is_inbox: false,
        ai_excluded: false,
        pinned_at: None,
        archived_at: None,
        created_at: input.created_at,
        updated_at: input.created_at,
        deleted_at: None,
        hlc_json,
        schema_version: SCHEMA_VERSION,
        groups_raw,
    })
}

/// The columns of the `chat` table that are built from the groups.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatColumns {
    pub created_at: i64,
    pub is_inbox: bool,
    pub title: String,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub sort_key: String,
    pub pinned_at: Option<i64>,
    pub archived_at: Option<i64>,
    pub ai_excluded: bool,
    pub deleted_at: Option<i64>,
}

/// The chat's columns from its `groups_raw`.
pub fn chat_columns(groups_raw: &str) -> Result<ChatColumns, GroupError> {
    let core = group_object(groups_raw, "core")?;
    let meta = group_object(groups_raw, "meta")?;
    let order = group_object(groups_raw, "order")?;
    let state = group_object(groups_raw, "state")?;
    let privacy = group_object(groups_raw, "privacy")?;
    let deleted = group_object(groups_raw, "deleted")?;
    Ok(ChatColumns {
        created_at: opt_int(&core, "created_at")?.ok_or_else(|| corrupt("no created_at"))?,
        is_inbox: flag(&core, "is_inbox")?,
        title: opt_str(&meta, "title")?.unwrap_or_default(),
        icon: opt_str(&meta, "icon")?,
        color: opt_str(&meta, "color")?,
        sort_key: req_str(&order, "sort_key")?,
        pinned_at: opt_int(&state, "pinned_at")?,
        archived_at: opt_int(&state, "archived_at")?,
        ai_excluded: flag(&privacy, "ai_excluded")?,
        deleted_at: opt_int(&deleted, "deleted_at")?,
    })
}

/// Rewrites `meta`: the name and/or emoji (`None` -- the field stays); `color` and unknown fields are kept verbatim.
/// Inbox cannot be renamed: its name in the database is empty, the interface shows the translated one.
pub fn set_meta(
    current: &Groups<'_>,
    title: Option<&str>,
    icon: Option<&str>,
    new_hlc: &str,
) -> Result<Rewritten, GroupError> {
    rewrite_group(current, "meta", new_hlc, None, |meta, _| {
        if let Some(title) = title {
            meta.insert("title".into(), json!(title));
        }
        if let Some(icon) = icon {
            meta.insert("icon".into(), json!(icon));
        }
        Ok(())
    })
}

/// A change to the `state` group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateChange {
    /// Pin (`Some(time)`) or unpin.
    Pinned(Option<i64>),
    /// Into the archive (`Some(time)`) or out of it. Inbox cannot be archived.
    Archived(Option<i64>),
}

/// Rewrites `state`: pinning or archive; the other field and unknown fields are kept verbatim.
pub fn set_state(
    current: &Groups<'_>,
    change: StateChange,
    new_hlc: &str,
) -> Result<Rewritten, GroupError> {
    rewrite_group(current, "state", new_hlc, None, |state, _| {
        match change {
            StateChange::Pinned(at) => state.insert("pinned_at".into(), json!(at)),
            StateChange::Archived(at) => state.insert("archived_at".into(), json!(at)),
        };
        Ok(())
    })
}

/// Whether this is Inbox (by id, not by a flag, ADR-0001 §3.6).
pub fn is_inbox(chat_id: &str) -> bool {
    chat_id == INBOX_ID
}

/// A valid chat id format: Inbox (a constant) or a lower-case UUID v4 (ADR-0001 §1). Whether the chat exists is the store's concern.
pub fn is_chat_id(chat_id: &str) -> bool {
    is_inbox(chat_id) || crate::hlc::is_uuid_v4(chat_id)
}
