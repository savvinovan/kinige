//! Helpers for the entry and chat action tests: groups verbatim, HLC, unknown groups, columns.

#![allow(dead_code)]

use super::{edit_raw_groups, raw};
use kinige_domain::groups::parse_groups_raw;
use serde_json::{Value, json, value::RawValue};
use std::collections::BTreeMap;
use std::path::Path;

/// An unknown group with spaces in its bytes.
pub const FUTURE_GROUP: &str = r#"{ "z": [1, 2],  "a": "ё" }"#;

/// The groups of an entry verbatim: `name -> bytes`.
pub fn groups(dir: &Path, table: &str, id: &str) -> BTreeMap<String, String> {
    let text: String = raw(dir)
        .query_row(
            &format!("SELECT groups_raw FROM {table} WHERE id = ?1"),
            [id],
            |r| r.get(0),
        )
        .unwrap();
    parse_groups_raw(&text)
        .unwrap()
        .into_iter()
        .map(|(name, bytes)| (name, bytes.get().to_string()))
        .collect()
}

/// The `hlc_json` records verbatim: `name -> bytes of the record`.
pub fn hlc_entries(dir: &Path, table: &str, id: &str) -> BTreeMap<String, String> {
    let text: String = raw(dir)
        .query_row(
            &format!("SELECT hlc_json FROM {table} WHERE id = ?1"),
            [id],
            |r| r.get(0),
        )
        .unwrap();
    serde_json::from_str::<BTreeMap<String, Box<RawValue>>>(&text)
        .unwrap()
        .into_iter()
        .map(|(name, bytes)| (name, bytes.get().to_string()))
        .collect()
}

/// The HLC of a group.
pub fn group_hlc(dir: &Path, table: &str, id: &str, group: &str) -> String {
    let entry = &hlc_entries(dir, table, id)[group];
    serde_json::from_str::<Value>(entry).unwrap()["h"]
        .as_str()
        .unwrap()
        .to_string()
}

/// The state of the HLC clock in `sync_state`.
pub fn clock(dir: &Path) -> (String, String) {
    let conn = raw(dir);
    let get = |key: &str| -> String {
        conn.query_row("SELECT value FROM sync_state WHERE key = ?1", [key], |r| {
            r.get(0)
        })
        .unwrap()
    };
    (get("hlc_l"), get("hlc_c"))
}

/// Adds to an entry an unknown group `future` (within the supported `sv`, spaces in its bytes) and an extra field in the `hlc_json`
/// record of the `flags` group. These bytes must survive a rewrite of any other group verbatim.
pub fn add_foreign(dir: &Path, id: &str) {
    edit_raw_groups(dir, id, |groups, hlcs| {
        groups.insert(
            "future".into(),
            RawValue::from_string(FUTURE_GROUP.into()).unwrap(),
        );
        let h = serde_json::from_str::<Value>(hlcs["body"].get()).unwrap()["h"].clone();
        hlcs.insert(
            "future".into(),
            RawValue::from_string(format!(r#"{{ "h": {h}, "sv": 1, "x": true }}"#)).unwrap(),
        );
        hlcs.insert(
            "flags".into(),
            RawValue::from_string(format!(r#"{{"h":{h},"sv":1,"note":"keep"}}"#)).unwrap(),
        );
    });
}

/// The same for a chat.
pub fn add_foreign_chat(dir: &Path, id: &str) {
    let conn = raw(dir);
    let (groups_raw, hlc_json): (String, String) = conn
        .query_row(
            "SELECT groups_raw, hlc_json FROM chat WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    let mut groups: BTreeMap<String, Box<RawValue>> = serde_json::from_str(&groups_raw).unwrap();
    let mut hlcs: BTreeMap<String, Box<RawValue>> = serde_json::from_str(&hlc_json).unwrap();
    groups.insert(
        "future".into(),
        RawValue::from_string(FUTURE_GROUP.into()).unwrap(),
    );
    let h = serde_json::from_str::<Value>(hlcs["core"].get()).unwrap()["h"].clone();
    hlcs.insert(
        "future".into(),
        RawValue::from_string(format!(r#"{{ "h": {h}, "sv": 1, "x": true }}"#)).unwrap(),
    );
    hlcs.insert(
        "privacy".into(),
        RawValue::from_string(format!(r#"{{"h":{h},"sv":1,"note":"keep"}}"#)).unwrap(),
    );
    conn.execute(
        "UPDATE chat SET groups_raw = ?1, hlc_json = ?2 WHERE id = ?3",
        (
            serde_json::to_string(&groups).unwrap(),
            serde_json::to_string(&hlcs).unwrap(),
            id,
        ),
    )
    .unwrap();
}

/// A group with `sv = 2`: the entry becomes "read-only".
pub fn make_read_only(dir: &Path, id: &str) {
    edit_raw_groups(dir, id, |_, hlcs| {
        let h = serde_json::from_str::<Value>(hlcs["flags"].get()).unwrap()["h"].clone();
        hlcs.insert(
            "flags".into(),
            RawValue::from_string(json!({"h": h, "sv": 2}).to_string()).unwrap(),
        );
    });
    raw(dir)
        .execute("UPDATE message SET schema_version = 2 WHERE id = ?1", [id])
        .unwrap();
}

/// The columns that are built from the groups, as a single value for comparison.
pub fn columns(dir: &Path, id: &str) -> Value {
    raw(dir)
        .query_row(
            "SELECT chat_id, body_md, journal_date, edited_at, is_task, task_done_at, favorite_at, deleted_at, \
             body_base, body_supersedes, placement_base, placement_supersedes, schema_version \
             FROM message WHERE id = ?1",
            [id],
            |r| {
                Ok(json!({
                    "chat_id": r.get::<_, String>(0)?,
                    "body_md": r.get::<_, String>(1)?,
                    "journal_date": r.get::<_, String>(2)?,
                    "edited_at": r.get::<_, Option<i64>>(3)?,
                    "is_task": r.get::<_, bool>(4)?,
                    "task_done_at": r.get::<_, Option<i64>>(5)?,
                    "favorite_at": r.get::<_, Option<i64>>(6)?,
                    "deleted_at": r.get::<_, Option<i64>>(7)?,
                    "body_base": r.get::<_, Option<String>>(8)?,
                    "body_supersedes": r.get::<_, String>(9)?,
                    "placement_base": r.get::<_, Option<String>>(10)?,
                    "placement_supersedes": r.get::<_, String>(11)?,
                    "schema_version": r.get::<_, i64>(12)?,
                }))
            },
        )
        .unwrap()
}

/// Appendix §B item 38 invariant: the entry columns are the parsed representation of the canonical bytes of its groups.
pub fn assert_columns_match_groups(dir: &Path, id: &str) {
    let text: String = raw(dir)
        .query_row("SELECT groups_raw FROM message WHERE id = ?1", [id], |r| {
            r.get(0)
        })
        .unwrap();
    let parsed = kinige_domain::message_ops::message_columns(&text).unwrap();
    let c = columns(dir, id);
    assert_eq!(c["chat_id"], json!(parsed.chat_id));
    assert_eq!(c["body_md"], json!(parsed.body_md));
    assert_eq!(c["journal_date"], json!(parsed.journal_date));
    assert_eq!(c["edited_at"], json!(parsed.edited_at));
    assert_eq!(c["is_task"], json!(parsed.is_task));
    assert_eq!(c["task_done_at"], json!(parsed.task_done_at));
    assert_eq!(c["favorite_at"], json!(parsed.favorite_at));
    assert_eq!(c["deleted_at"], json!(parsed.deleted_at));
    assert_eq!(c["body_base"], json!(parsed.body_base));
    assert_eq!(c["body_supersedes"], json!(parsed.body_supersedes));
    assert_eq!(c["placement_base"], json!(parsed.placement_base));
    assert_eq!(
        c["placement_supersedes"],
        json!(parsed.placement_supersedes)
    );
    // Every group is canonical: re-encoding gives the same bytes (except foreign ones, written by a foreign client).
    for (name, bytes) in groups(dir, "message", id) {
        if name == "future" {
            continue;
        }
        let value: Value = serde_json::from_str(&bytes).unwrap();
        assert_eq!(
            kinige_domain::jcs::canonical_bytes(&value).unwrap(),
            bytes.as_bytes(),
            "{name} is canonical"
        );
    }
}

/// The database file is written in full: the writer thread closes the connection only after `drop(store)`, and while some entries lie in
/// `-wal`, the main file is incomplete. A checkpoint moves everything into the main file (it repeats while the connection is busy).
pub fn wait_closed(dir: &Path) {
    super::wait_until("WAL checkpoint", || {
        raw(dir)
            .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
                row.get::<_, i64>(0)
            })
            .is_ok_and(|busy| busy == 0)
    });
}

/// The number of revisions of an entry.
pub fn revisions(dir: &Path, message_id: &str) -> i64 {
    raw(dir)
        .query_row(
            "SELECT COUNT(*) FROM message_revision WHERE message_id = ?1",
            [message_id],
            |r| r.get(0),
        )
        .unwrap()
}

/// The number of rows in a table.
pub fn count_rows(dir: &Path, table: &str) -> i64 {
    raw(dir)
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
        .unwrap()
}

/// The groups `before` and `after` match verbatim everywhere except `changed`.
pub fn assert_only_changed(
    before: &BTreeMap<String, String>,
    after: &BTreeMap<String, String>,
    changed: &[&str],
) {
    assert_eq!(
        before.keys().collect::<Vec<_>>(),
        after.keys().collect::<Vec<_>>(),
        "the set of groups"
    );
    for (name, bytes) in before {
        if changed.contains(&name.as_str()) {
            assert_ne!(&after[name], bytes, "{name} must change");
        } else {
            assert_eq!(&after[name], bytes, "{name} verbatim");
        }
    }
}
