//! Помощники тестов быстрого захвата: чтение строки записи, группы, счётчики.

#![allow(dead_code)]

use super::{MINUTE, edit_raw_groups, raw};
use kinige_domain::groups::parse_groups_raw;
use serde_json::{Value, json, value::RawValue};
use std::path::Path;

pub const MIN10: i64 = 10 * MINUTE;

pub struct Row {
    pub body_md: String,
    pub edited_at: Option<i64>,
    pub created_at: i64,
    pub body_base: Option<String>,
    pub body_supersedes: String,
    pub hlc_json: String,
    pub groups_raw: String,
    pub schema_version: i64,
    pub origin: String,
    pub server_known: i64,
}

pub fn row(dir: &Path, id: &str) -> Row {
    raw(dir)
        .query_row(
            "SELECT body_md, edited_at, created_at, body_base, body_supersedes, hlc_json, groups_raw, schema_version, \
             origin, server_known FROM message WHERE id = ?1",
            [id],
            |r| {
                Ok(Row {
                    body_md: r.get(0)?,
                    edited_at: r.get(1)?,
                    created_at: r.get(2)?,
                    body_base: r.get(3)?,
                    body_supersedes: r.get(4)?,
                    hlc_json: r.get(5)?,
                    groups_raw: r.get(6)?,
                    schema_version: r.get(7)?,
                    origin: r.get(8)?,
                    server_known: r.get(9)?,
                })
            },
        )
        .unwrap()
}

pub fn count(dir: &Path) -> i64 {
    raw(dir)
        .query_row("SELECT COUNT(*) FROM message", [], |r| r.get(0))
        .unwrap()
}

pub fn state(dir: &Path, key: &str) -> String {
    raw(dir)
        .query_row("SELECT value FROM sync_state WHERE key = ?1", [key], |r| {
            r.get(0)
        })
        .unwrap()
}

pub fn body_hlc(row: &Row) -> String {
    serde_json::from_str::<Value>(&row.hlc_json).unwrap()["body"]["h"]
        .as_str()
        .unwrap()
        .to_string()
}

pub fn group(row: &Row, name: &str) -> String {
    parse_groups_raw(&row.groups_raw).unwrap()[name]
        .get()
        .to_string()
}

pub fn set_group(dir: &Path, id: &str, name: &'static str, text: &'static str, sv: u64) {
    edit_raw_groups(dir, id, |groups, hlcs| {
        groups.insert(name.into(), RawValue::from_string(text.into()).unwrap());
        let h = hlcs
            .get("body")
            .map(|b| serde_json::from_str::<Value>(b.get()).unwrap()["h"].clone())
            .unwrap();
        hlcs.insert(
            name.into(),
            RawValue::from_string(json!({"h": h, "sv": sv}).to_string()).unwrap(),
        );
    });
}
