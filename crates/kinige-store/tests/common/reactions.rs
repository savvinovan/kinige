//! Helpers for the reaction tests (slice 5a): an entry, its reaction group verbatim, the derived rows, a group written behind the store's back.

#![allow(dead_code)]

use super::days::at;
use super::raw;
use kinige_domain::MessageView;
use kinige_domain::groups::INBOX_ID;
use kinige_store::{Now, Store};
use serde_json::{Value, json};
use std::path::Path;

pub const HEART: &str = "\u{2764}\u{FE0F}";

/// 2026-11-18 09:05 Moscow.
pub fn morning() -> Now {
    at(2026, 11, 18, 9, 5)
}

/// Minutes after [`morning`].
pub fn later(minutes: u32) -> Now {
    at(2026, 11, 18, 9, 5 + minutes)
}

pub fn send(store: &Store, text: &str) -> MessageView {
    store
        .send_entry(INBOX_ID, text, None, &[], &morning())
        .unwrap()
}

/// The `reactions` group of an entry verbatim; `None` when the entry has none.
pub fn group(dir: &Path, id: &str) -> Option<String> {
    groups_raw(dir, id)
        .get("reactions")
        .map(|raw| raw.get().to_string())
}

fn groups_raw(
    dir: &Path,
    id: &str,
) -> std::collections::BTreeMap<String, Box<serde_json::value::RawValue>> {
    let text: String = raw(dir)
        .query_row("SELECT groups_raw FROM message WHERE id = ?1", [id], |r| {
            r.get(0)
        })
        .unwrap();
    serde_json::from_str(&text).unwrap()
}

/// The group as JSON.
pub fn group_json(dir: &Path, id: &str) -> Value {
    serde_json::from_str(&group(dir, id).expect("the entry has the group")).unwrap()
}

/// The derived rows of an entry: `(key, emoji, put_at, put_tz)` in the order put.
pub fn rows(dir: &Path, id: &str) -> Vec<(String, String, i64, String)> {
    let conn = raw(dir);
    let mut statement = conn
        .prepare(
            "SELECT key, emoji, put_at, put_tz FROM message_reaction WHERE message_id = ?1 ORDER BY put_at, key",
        )
        .unwrap();
    statement
        .query_map([id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

/// Every derived row of the database, for "rebuilt equals incremental".
pub fn all_rows(
    dir: &Path,
) -> (
    Vec<(String, String, String, i64, String)>,
    Vec<(String, String)>,
) {
    let conn = raw(dir);
    let reactions = conn
        .prepare(
            "SELECT message_id, key, emoji, put_at, put_tz FROM message_reaction ORDER BY 1, 2",
        )
        .unwrap()
        .query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let keys = conn
        .prepare("SELECT message_id, keys FROM message_reaction_keys ORDER BY 1")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    (reactions, keys)
}

/// Writes a `reactions` group (and its `hlc_json` entry) behind the store's back, as a peer would: the bytes are what the test passes,
/// the HLC is `h`, `sv` is as given. No derived rows are made.
pub fn put_group(dir: &Path, id: &str, bytes: &str, h: &str, sv: u64) {
    super::edit_raw_groups(dir, id, |groups, hlcs| {
        groups.insert(
            "reactions".into(),
            serde_json::value::RawValue::from_string(bytes.to_string()).unwrap(),
        );
        hlcs.insert(
            "reactions".into(),
            serde_json::value::RawValue::from_string(json!({"h": h, "sv": sv}).to_string())
                .unwrap(),
        );
    });
}

/// An HLC of this device's id above everything a test entry has (the real stamps are in 2026).
pub fn hlc_of(dir: &Path, id: &str, group: &str) -> String {
    super::ops::group_hlc(dir, "message", id, group)
}

pub fn emoji(first: u32, count: u32) -> Vec<String> {
    (0..count)
        .map(|i| char::from_u32(first + i).unwrap().to_string())
        .collect()
}

pub fn set(store: &Store, id: &str, emoji: &str, on: bool, now: &Now) -> MessageView {
    store.set_message_reaction(id, emoji, on, now).unwrap()
}
