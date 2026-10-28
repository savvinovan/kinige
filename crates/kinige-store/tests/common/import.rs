//! Помощники тестов импорта: экспорт Telegram как JSON, запуск импорта, разбор результата из базы.

#![allow(dead_code)]

use super::{moscow, raw};
use kinige_domain::Language;
use kinige_domain::import::{ImportProgress, ImportReport};
use kinige_domain::telegram::{TgExport, parse_export};
use kinige_store::{Now, Store, StoreError};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

/// 2019-03-08 12:00:00 по Москве (09:00 UTC).
pub const SAVED_T: i64 = 1_552_035_600;
/// Id владельца и «Избранного» в тестовых экспортах.
pub const OWNER: &str = "1000001";
/// Id собеседника в личном чате.
pub const PETER: &str = "2000002";

/// Простое сообщение в формате экспорта: id, `date_unixtime`, текст, автор.
pub fn msg(id: u64, unix: i64, text: &str, from_id: &str) -> String {
    format!(
        r#"{{"id":{id},"type":"message","date":"x","date_unixtime":"{unix}","from":"Имя {from_id}","from_id":"{from_id}","text":{t},"text_entities":[{{"type":"plain","text":{t}}}]}}"#,
        t = serde_json::to_string(text).unwrap()
    )
}

/// Экспорт одного чата.
pub fn single(kind: &str, id: &str, name: Option<&str>, messages: &[String]) -> String {
    let name = name
        .map(|n| format!(r#""name":{},"#, serde_json::to_string(n).unwrap()))
        .unwrap_or_default();
    format!(
        r#"{{{name}"type":"{kind}","id":{id},"messages":[{}]}}"#,
        messages.join(",")
    )
}

/// Полный экспорт из готовых объектов чатов.
pub fn full(chats: &[String]) -> String {
    format!(
        r#"{{"personal_information":{{"user_id":{OWNER}}},"chats":{{"list":[{}]}}}}"#,
        chats.join(",")
    )
}

/// «Избранное» из `n` сообщений: по одному в секунду, текст `запись {i}`.
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
    Arc::new(parse_export(json).expect("тестовый экспорт разбирается"))
}

pub fn all_chats(export: &TgExport) -> Vec<String> {
    export.chats.iter().map(|c| c.id.clone()).collect()
}

pub fn now() -> Now {
    moscow(super::T0)
}

/// Импорт выбранных чатов (все, если `chats` пуст), без остановки и прогресса.
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

/// Число строк `message` с условием.
pub fn count_where(dir: &Path, condition: &str) -> i64 {
    raw(dir)
        .query_row(
            &format!("SELECT COUNT(*) FROM message WHERE {condition}"),
            [],
            |r| r.get(0),
        )
        .unwrap()
}
