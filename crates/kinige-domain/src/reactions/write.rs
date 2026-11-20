//! Putting and removing a reaction (slice 5a). A write changes one item and leaves every other item byte-identical; the group goes
//! through [`rewrite_or_add_group`], so the first reaction on an entry adds the group (ADR-0001 appendix §C.9) and later ones rewrite it.
//! The caps are input rules here and nowhere else (promise 1): a put is refused when the entry has [`MAX_LIVE`] live reactions or when
//! it would add the 33rd key, and a toggle of a key the entry has had always works.

use super::{GROUP, Group, MAX_AT, MAX_GROUP_BYTES, MAX_KEYS, MAX_LIVE, Stored, is_valid_tz};
use super::{Item, reaction_key, read_group};
use crate::add_group::rewrite_or_add_group;
use crate::emoji::is_emoji_sequence;
use crate::groups::GroupError;
use crate::rewrite::{Groups, Rewritten};
use serde_json::{Map, Value, json};

/// Why a write was refused; nothing is written then.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReactionError {
    /// The text is not one emoji sequence.
    Invalid,
    /// A put on an entry with [`MAX_LIVE`] live reactions, or one that would add the 33rd key.
    Limit,
    /// The stored group is not well-formed (a corrupt group, as §C.7 treats one), or the record's groups are damaged.
    Group(GroupError),
}

impl From<GroupError> for ReactionError {
    fn from(error: GroupError) -> Self {
        ReactionError::Group(error)
    }
}

/// A write that is needed: `plan` found the entry in another state than the request. Nothing is written without one.
#[derive(Debug, Clone)]
pub struct Plan {
    key: String,
    emoji: String,
    put: bool,
    items: Vec<Item>,
}

/// Decides what a put (`on`) or a removal needs. `Ok(None)`: the entry is already in the requested state, so nothing is written and the time
/// of a live reaction is kept.
pub fn plan(current: &Groups<'_>, emoji: &str, on: bool) -> Result<Option<Plan>, ReactionError> {
    if !is_emoji_sequence(emoji) {
        return Err(ReactionError::Invalid);
    }
    let group = match read_group(current)? {
        Stored::Absent => Group { items: Vec::new() },
        Stored::Group(group) => group,
        Stored::Unreadable => {
            return Err(
                GroupError::Corrupt("the reactions group is not well-formed".into()).into(),
            );
        }
    };
    let key = reaction_key(emoji);
    let existing = group.items.iter().find(|item| item.key == key);
    let live = existing.is_some_and(Item::is_live);
    if on {
        if existing.is_some_and(|item| item.shown().is_some()) {
            return Ok(None);
        }
        // A live item that shows nothing (an `e` that is not an emoji) is replaced, so it does not grow the group.
        if !live && group.live_count() >= MAX_LIVE {
            return Err(ReactionError::Limit);
        }
        if existing.is_none() && group.items.len() >= MAX_KEYS {
            return Err(ReactionError::Limit);
        }
    } else if !live {
        return Ok(None);
    }
    Ok(Some(Plan {
        key,
        emoji: emoji.to_string(),
        put: on,
        items: group.items,
    }))
}

/// Writes the planned change under `new_hlc` (strictly above every HLC of the record): the changed item gets it as its `h`, so does the
/// group. `at_ms` and `tz` are the moment and the device's zone of a put; a zone that is not valid reads as `UTC`, a time out of range is clamped.
pub fn apply(
    current: &Groups<'_>,
    plan: &Plan,
    (at_ms, tz): (i64, &str),
    new_hlc: &str,
) -> Result<Rewritten, GroupError> {
    let mut items: Vec<Item> = plan
        .items
        .iter()
        .filter(|i| i.key != plan.key)
        .cloned()
        .collect();
    let mut object = plan
        .items
        .iter()
        .find(|item| item.key == plan.key)
        .map_or_else(Map::new, |item| item.object.clone());
    if plan.put {
        let tz = if is_valid_tz(tz) { tz } else { "UTC" };
        object.insert("e".into(), json!(plan.emoji));
        object.insert("at".into(), json!(at_ms.clamp(0, MAX_AT)));
        object.insert("tz".into(), json!(tz));
    } else {
        object.insert("at".into(), Value::Null);
        object.insert("tz".into(), Value::Null);
    }
    object.insert("h".into(), json!(new_hlc));
    items.push(Item {
        key: plan.key.clone(),
        object,
    });
    items.sort_by(|a, b| a.key.cmp(&b.key));
    let list: Vec<Value> = items
        .into_iter()
        .map(|item| Value::Object(item.object))
        .collect();
    rewrite_or_add_group(
        current,
        "message",
        GROUP,
        new_hlc,
        Some(MAX_GROUP_BYTES),
        |group| {
            group.insert("items".into(), Value::Array(list));
            Ok(())
        },
    )
}
