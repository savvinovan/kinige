//! Помощники тестов действий с записью и чатом: группы дословно, HLC, неизвестные группы, колонки.

#![allow(dead_code)]

use super::{edit_raw_groups, raw};
use kinige_domain::groups::parse_groups_raw;
use serde_json::{Value, json, value::RawValue};
use std::collections::BTreeMap;
use std::path::Path;

/// Неизвестная группа с пробелами в байтах.
pub const FUTURE_GROUP: &str = r#"{ "z": [1, 2],  "a": "ё" }"#;

/// Группы записи дословно: `имя → байты`.
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

/// Записи `hlc_json` дословно: `имя → байты записи`.
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

/// HLC группы.
pub fn group_hlc(dir: &Path, table: &str, id: &str, group: &str) -> String {
    let entry = &hlc_entries(dir, table, id)[group];
    serde_json::from_str::<Value>(entry).unwrap()["h"]
        .as_str()
        .unwrap()
        .to_string()
}

/// Состояние часов HLC в `sync_state`.
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

/// Добавляет записи неизвестную группу `future` (в поддерживаемой `sv`, пробелы в байтах) и лишнее поле в запись `hlc_json`
/// группы `flags`. Эти байты должны пережить перезапись любой другой группы дословно.
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

/// То же для чата.
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

/// Группа с `sv = 2`: запись становится «только для чтения».
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

/// Колонки, которые строятся из групп, одним значением для сравнения.
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

/// Инвариант приложения §B п. 38: колонки записи — разобранное представление каноничных байтов её групп.
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
    // Каждая группа канонична: перекодирование даёт те же байты (кроме чужих, записанных чужим клиентом).
    for (name, bytes) in groups(dir, "message", id) {
        if name == "future" {
            continue;
        }
        let value: Value = serde_json::from_str(&bytes).unwrap();
        assert_eq!(
            kinige_domain::jcs::canonical_bytes(&value).unwrap(),
            bytes.as_bytes(),
            "{name} каноничен"
        );
    }
}

/// Хранилище закрыто, и файл базы записан целиком: поток-писатель закрывает соединение уже после `drop(store)`, и пока
/// рядом лежит `-wal`, часть записей ещё не в основном файле.
pub fn wait_closed(dir: &Path) {
    let wal = dir.join("kinige.db-wal");
    super::wait_until("база закрыта", || !wal.exists());
}

/// Число ревизий записи.
pub fn revisions(dir: &Path, message_id: &str) -> i64 {
    raw(dir)
        .query_row(
            "SELECT COUNT(*) FROM message_revision WHERE message_id = ?1",
            [message_id],
            |r| r.get(0),
        )
        .unwrap()
}

/// Число строк в таблице.
pub fn count_rows(dir: &Path, table: &str) -> i64 {
    raw(dir)
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
        .unwrap()
}

/// Группы `before` и `after` совпадают дословно везде, кроме `changed`.
pub fn assert_only_changed(
    before: &BTreeMap<String, String>,
    after: &BTreeMap<String, String>,
    changed: &[&str],
) {
    assert_eq!(
        before.keys().collect::<Vec<_>>(),
        after.keys().collect::<Vec<_>>(),
        "набор групп"
    );
    for (name, bytes) in before {
        if changed.contains(&name.as_str()) {
            assert_ne!(&after[name], bytes, "{name} должна измениться");
        } else {
            assert_eq!(&after[name], bytes, "{name} дословно");
        }
    }
}
