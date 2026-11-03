//! Helpers for the import tests: a Telegram export as JSON, running an import, parsing the result from the database.

#![allow(dead_code)]

use super::{moscow, raw};
use kinige_domain::Language;
use kinige_domain::import::{ImportProgress, ImportReport};
use kinige_domain::telegram::{TgExport, parse_export};
use kinige_store::{Now, Store, StoreError};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

/// 2019-03-08 12:00:00 Moscow time (09:00 UTC).
pub const SAVED_T: i64 = 1_552_035_600;
/// Id of the owner and of "Saved Messages" in the test exports.
pub const OWNER: &str = "1000001";
/// Id of the other party in a personal chat.
pub const PETER: &str = "2000002";

/// A simple message in the export format: id, `date_unixtime`, text, author.
pub fn msg(id: u64, unix: i64, text: &str, from_id: &str) -> String {
    format!(
        r#"{{"id":{id},"type":"message","date":"x","date_unixtime":"{unix}","from":"Имя {from_id}","from_id":"{from_id}","text":{t},"text_entities":[{{"type":"plain","text":{t}}}]}}"#,
        t = serde_json::to_string(text).unwrap()
    )
}

/// An export of one chat.
pub fn single(kind: &str, id: &str, name: Option<&str>, messages: &[String]) -> String {
    let name = name
        .map(|n| format!(r#""name":{},"#, serde_json::to_string(n).unwrap()))
        .unwrap_or_default();
    format!(
        r#"{{{name}"type":"{kind}","id":{id},"messages":[{}]}}"#,
        messages.join(",")
    )
}

/// A full export from ready-made chat objects.
pub fn full(chats: &[String]) -> String {
    format!(
        r#"{{"personal_information":{{"user_id":{OWNER}}},"chats":{{"list":[{}]}}}}"#,
        chats.join(",")
    )
}

/// "Saved Messages" of `n` messages: one per second, text `запись {i}`.
pub fn saved_n(n: u64) -> String {
    let messages: Vec<String> = (1..=n)
        .map(|i| {
            msg(
                i,
                SAVED_T + i as i64,
                &format!("запись {i}"),
                &format!("user{OWNER}"),
            )
        })
        .collect();
    single("saved_messages", OWNER, None, &messages)
}

pub fn parse(json: &str) -> Arc<TgExport> {
    Arc::new(parse_export(json).expect("the test export parses"))
}

pub fn all_chats(export: &TgExport) -> Vec<String> {
    export.chats.iter().map(|c| c.id.clone()).collect()
}

pub fn now() -> Now {
    moscow(super::T0)
}

/// Import of the selected chats (all if `chats` is empty), without a stop and progress.
pub fn run(
    store: &Store,
    export: &Arc<TgExport>,
    chats: &[&str],
) -> Result<ImportReport, StoreError> {
    let ids = if chats.is_empty() {
        all_chats(export)
    } else {
        chats.iter().map(|c| c.to_string()).collect()
    };
    store.import_telegram(
        export.clone(),
        ids,
        Language::Ru,
        &now(),
        |_: ImportProgress| {},
        Arc::new(AtomicBool::new(false)),
    )
}

/// The number of `message` rows matching a condition.
pub fn count_where(dir: &Path, condition: &str) -> i64 {
    raw(dir)
        .query_row(
            &format!("SELECT COUNT(*) FROM message WHERE {condition}"),
            [],
            |r| r.get(0),
        )
        .unwrap()
}
