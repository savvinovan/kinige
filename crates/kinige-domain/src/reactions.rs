//! Reactions on an entry (slice 5a, PRD §4.1, ADR-0001 appendix §A.0 and §C.9). No IO.
//!
//! One group `reactions` on the message, `sv = 1`, canonical JCS:
//! `{"items":[{"at":<ms>|null,"e":"<emoji>","h":"<HLC>","tz":"<IANA zone>"|null}, …]}`, one item per [key](reaction_key), sorted by key.
//! `at` and `tz` are `null` in a removal marker. This file is the one pinned reader of the group; [`write`] changes it and
//! [`present`] turns it into what the feed, the Markdown copy and MCP show.
//!
//! Three promises, constants of `sv = 1` (changing any of them is an `sv` bump):
//! 1. a removal marker is never dropped: the caps ([`MAX_LIVE`], [`MAX_KEYS`]) are input rules of a put, nothing trims a group;
//! 2. only well-formed groups are written, and [`read_group`] decides what is well-formed;
//! 3. the key is [`reaction_key`], its own function with reference vectors, not the picker's `emoji_identity`.

pub mod present;
pub mod write;

use crate::emoji::is_emoji_sequence;
use crate::groups::{GroupError, SCHEMA_VERSION, parse_groups_raw};
use crate::hlc;
use crate::jcs;
use crate::rewrite::{Groups, corrupt};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

/// The name of the group on a message.
pub const GROUP: &str = "reactions";
/// How many live reactions an entry may hold when a new one is put.
pub const MAX_LIVE: usize = 10;
/// How many different keys an entry may ever have had: a 33rd new key is refused, removal markers count.
pub const MAX_KEYS: usize = 32;
/// The largest canonical group the reader accepts, in bytes.
pub const MAX_GROUP_BYTES: usize = 8 * 1024;
/// The longest time zone name in bytes.
pub const MAX_TZ_BYTES: usize = 64;
/// The largest `at`: a millisecond time in the year 2286.
pub const MAX_AT: i64 = 9_999_999_999_999;

const VS16: char = '\u{FE0F}';

/// A live reaction the reader lets through: valid emoji, a time and a time zone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reaction {
    pub key: String,
    /// The emoji in the form the picker gave.
    pub emoji: String,
    /// When it was put (UTC ms).
    pub put_at: i64,
    /// The device's IANA zone when it was put.
    pub put_tz: String,
}

/// What a stored group shows: the live valid reactions in the order they were put (ties by key), and how many different keys the group holds.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Shown {
    pub reactions: Vec<Reaction>,
    /// Items in the group, removal markers included: what the 32-key cap counts.
    pub keys: usize,
}

/// The comparison key of an emoji: the string without U+FE0F, compared in UTF-8 byte order (promise 3). `❤` and `❤️` are one
/// reaction. It is not `emoji_identity`, which has changed before: the stored group depends on this one forever.
pub fn reaction_key(emoji: &str) -> String {
    emoji.chars().filter(|c| *c != VS16).collect()
}

/// What the store holds for a record: nothing, a group that is not well-formed or has another `sv`, or the items.
#[derive(Debug)]
pub enum Stored {
    Absent,
    Unreadable,
    Group(Group),
}

/// A well-formed group's items, as the objects the group holds (unknown fields included).
#[derive(Debug)]
pub struct Group {
    pub(crate) items: Vec<Item>,
}

#[derive(Debug, Clone)]
pub(crate) struct Item {
    pub(crate) key: String,
    pub(crate) object: Map<String, Value>,
}

impl Item {
    pub(crate) fn emoji(&self) -> &str {
        self.object.get("e").and_then(Value::as_str).unwrap_or("")
    }

    pub(crate) fn at(&self) -> Option<i64> {
        self.object.get("at").and_then(Value::as_i64)
    }

    /// The zone, only when it is valid.
    fn tz(&self) -> Option<&str> {
        self.object
            .get("tz")
            .and_then(Value::as_str)
            .filter(|tz| is_valid_tz(tz))
    }

    /// Put and not removed, whatever the emoji and zone look like: what the live cap counts.
    pub(crate) fn is_live(&self) -> bool {
        self.at().is_some()
    }

    /// What the feed, the copy and MCP may show.
    pub(crate) fn shown(&self) -> Option<Reaction> {
        let (at, tz) = (self.at()?, self.tz()?);
        is_emoji_sequence(self.emoji()).then(|| Reaction {
            key: self.key.clone(),
            emoji: self.emoji().to_string(),
            put_at: at,
            put_tz: tz.to_string(),
        })
    }
}

impl Group {
    pub(crate) fn live_count(&self) -> usize {
        self.items.iter().filter(|item| item.is_live()).count()
    }

    pub fn shown(&self) -> Shown {
        let mut reactions: Vec<Reaction> = self.items.iter().filter_map(Item::shown).collect();
        reactions.sort_by(|a, b| (a.put_at, &a.key).cmp(&(b.put_at, &b.key)));
        Shown {
            reactions,
            keys: self.items.len(),
        }
    }
}

/// A zone name the reader accepts: a non-empty string of at most [`MAX_TZ_BYTES`] bytes without control characters. Whether this
/// device knows the zone is the reader of the date's business (an unknown one reads as UTC).
pub fn is_valid_tz(tz: &str) -> bool {
    !tz.is_empty() && tz.len() <= MAX_TZ_BYTES && !tz.chars().any(char::is_control)
}

/// The reader (promise 2). `Err` only when the record's own maps do not parse; a group that is not well-formed, or whose `sv` is not
/// the supported one, is `Unreadable`: it shows nothing and a local write on it is refused as a corrupt group.
pub fn read_group(current: &Groups<'_>) -> Result<Stored, GroupError> {
    let groups = parse_groups_raw(current.groups_raw)?;
    let hlcs: BTreeMap<String, Value> = serde_json::from_str(current.hlc_json).map_err(corrupt)?;
    let (raw, entry) = (groups.get(GROUP), hlcs.get(GROUP));
    let (raw, entry) = match (raw, entry) {
        (None, None) => return Ok(Stored::Absent),
        (Some(raw), Some(entry)) => (raw, entry),
        _ => return Ok(Stored::Unreadable),
    };
    let group_hlc = entry.get("h").and_then(Value::as_str);
    let supported = entry.get("sv").and_then(Value::as_u64) == Some(u64::from(SCHEMA_VERSION));
    let Some(group_hlc) = group_hlc.filter(|h| supported && hlc::parse(h).is_ok()) else {
        return Ok(Stored::Unreadable);
    };
    Ok(parse_group(raw.get(), group_hlc).map_or(Stored::Unreadable, Stored::Group))
}

/// The well-formedness rules of the group's bytes: canonical, at most [`MAX_GROUP_BYTES`], an object whose `items` is an array of
/// objects with a string `e`, one item per key, a strict HLC `h` not above the group's, and `at` null or in range. Unknown fields stay.
fn parse_group(raw: &str, group_hlc: &str) -> Option<Group> {
    if raw.len() > MAX_GROUP_BYTES {
        return None;
    }
    let value: Value = serde_json::from_str(raw).ok()?;
    if jcs::canonical_string(&value).ok()? != raw {
        return None;
    }
    let mut items = Vec::new();
    let mut seen = BTreeSet::new();
    for item in value.get("items")?.as_array()? {
        let object = item.as_object()?;
        let emoji = object.get("e")?.as_str()?;
        let stamp = object.get("h")?.as_str()?;
        if hlc::parse(stamp).is_err() || stamp > group_hlc {
            return None;
        }
        match object.get("at")? {
            Value::Null => {}
            Value::Number(at) if (0..=MAX_AT).contains(&at.as_i64()?) => {}
            _ => return None,
        }
        let key = reaction_key(emoji);
        if !seen.insert(key.clone()) {
            return None;
        }
        items.push(Item {
            key,
            object: object.clone(),
        });
    }
    Some(Group { items })
}

/// What a record's reactions show: nothing when the group is absent, not well-formed or of another `sv`, or when the record's maps do not parse.
pub fn shown(groups_raw: &str, hlc_json: &str) -> Shown {
    let current = Groups {
        groups_raw,
        hlc_json,
    };
    match read_group(&current) {
        Ok(Stored::Group(group)) => group.shown(),
        _ => Shown::default(),
    }
}

#[cfg(test)]
mod caps_tests;
#[cfg(test)]
mod present_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod write_tests;
