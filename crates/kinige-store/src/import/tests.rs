//! Tests of import internals that are not reachable through the public API: an external id taken inside the transaction itself.

use super::*;
use crate::StoreOptions;
use kinige_domain::hlc::Clock;
use kinige_domain::telegram::parse_export;

fn open(dir: &std::path::Path) -> Store {
    let mut options = StoreOptions::new(dir.join("kinige.db"), None);
    options.start_maintenance = false;
    Store::open(options).unwrap()
}

fn prepared(ext_msg: &str, body: &str) -> Prepared {
    Prepared {
        id: uuid::Uuid::new_v4().to_string(),
        ext_msg: ext_msg.to_string(),
        body: body.to_string(),
        created_at: 1_552_035_600_000,
        journal_date: "2019-03-08".to_string(),
        reply_to_id: None,
    }
}

#[test]
fn a_taken_external_id_inside_the_transaction_skips_the_record_and_keeps_the_rest() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path());
    let export = parse_export(r#"{"type":"saved_messages","id":1000001,"messages":[]}"#).unwrap();
    let chat = &export.chats[0];
    // Someone else's entry took the external id "2" after the import had read `existing` (for example, it came from
    // another device): inserting the key does not add a row.
    store
        .write(|conn| {
            crate::ext::insert(conn, EXT_SRC_TELEGRAM, "1000001", "2", "other-record")
                .map_err(write_err)
        })
        .unwrap();
    let rows = vec![
        prepared("1", "первая"),
        prepared("2", "вторая"),
        prepared("3", "третья"),
    ];
    let lost = rows[1].id.clone();
    let outcome = store
        .write_batch(
            chat,
            Some(INBOX_ID.to_string()),
            rows,
            1_800_000_000_000,
            "UTC".into(),
        )
        .unwrap();
    assert_eq!(outcome.skipped, 1);
    let (bodies, lost_rows, owner): (Vec<String>, i64, String) = store
        .read(|conn| {
            let mut statement = conn
                .prepare("SELECT body_md FROM message WHERE chat_id = ?1 ORDER BY body_md")
                .map_err(read_err)?;
            let bodies = statement
                .query_map([INBOX_ID], |row| row.get::<_, String>(0))
                .map_err(read_err)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(read_err)?;
            let lost_rows = conn
                .query_row(
                    "SELECT COUNT(*) FROM message WHERE id = ?1",
                    [&lost],
                    |row| row.get(0),
                )
                .map_err(read_err)?;
            let owner = conn
                .query_row(
                    "SELECT message_id FROM message_ext WHERE ext_msg = '2'",
                    [],
                    |row| row.get(0),
                )
                .map_err(read_err)?;
            Ok((bodies, lost_rows, owner))
        })
        .unwrap();
    assert!(bodies.contains(&"первая".to_string()) && bodies.contains(&"третья".to_string()));
    assert!(!bodies.contains(&"вторая".to_string()));
    assert_eq!(lost_rows, 0, "there is no entry with the taken external id");
    assert_eq!(owner, "other-record", "the key stayed with the first entry");
}

#[test]
fn hlc_stays_strictly_increasing_when_the_device_clock_lags_behind_the_stored_one() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path());
    let export = parse_export(r#"{"type":"saved_messages","id":1000001,"messages":[]}"#).unwrap();
    // The device clock went back: the stored "wall" is 100,000,000 s ahead of the import moment.
    let ahead = Clock {
        l: 1_900_000_000_000,
        c: 5,
    };
    store
        .write(move |conn| {
            let tx = conn.transaction().map_err(write_err)?;
            save_clock(&tx, ahead).map_err(write_err)?;
            tx.commit().map_err(write_err)
        })
        .unwrap();
    let rows: Vec<Prepared> = (1..=4)
        .map(|i| prepared(&i.to_string(), "запись"))
        .collect();
    let ids: Vec<String> = rows.iter().map(|r| r.id.clone()).collect();
    store
        .write_batch(
            &export.chats[0],
            Some(INBOX_ID.to_string()),
            rows,
            1_800_000_000_000,
            "UTC".into(),
        )
        .unwrap();
    let (hlcs, stored): (Vec<Clock>, Clock) = store
        .read(move |conn| {
            let mut found = Vec::new();
            for id in &ids {
                let json: String = conn
                    .query_row("SELECT hlc_json FROM message WHERE id = ?1", [id], |row| {
                        row.get(0)
                    })
                    .map_err(read_err)?;
                let value: serde_json::Value = serde_json::from_str(&json).unwrap();
                // One label for all groups of the entry.
                let labels: BTreeSet<String> = value
                    .as_object()
                    .unwrap()
                    .values()
                    .map(|group| group["h"].as_str().unwrap().to_string())
                    .collect();
                assert_eq!(labels.len(), 1);
                let label = labels.into_iter().next().unwrap();
                found.push(kinige_domain::hlc::parse(&label).unwrap().0);
            }
            let read = |key: &str| -> String {
                conn.query_row(
                    "SELECT value FROM sync_state WHERE key = ?1",
                    [key],
                    |row| row.get(0),
                )
                .unwrap()
            };
            let stored = Clock {
                l: read("hlc_l").parse().unwrap(),
                c: read("hlc_c").parse().unwrap(),
            };
            Ok((found, stored))
        })
        .unwrap();
    let key = |c: &Clock| (c.l, c.c);
    assert!(
        key(&hlcs[0]) > key(&ahead),
        "the first label is newer than the previous clock"
    );
    assert!(
        hlcs.windows(2).all(|w| key(&w[0]) < key(&w[1])),
        "labels strictly increase: {hlcs:?}"
    );
    assert_eq!(
        stored,
        *hlcs.last().unwrap(),
        "the database stores the clock of the last label"
    );
    assert_eq!(
        hlcs[0].l, ahead.l,
        "the \"wall\" does not roll back to the device clock"
    );
}

/// The entry that already owns the external id "1" (UUID v4: `reply_to_id` is validated).
const OWNER_ID: &str = "7b2c9d10-1111-4222-8333-444455556666";

fn reply_of(store: &Store, id: &str) -> Option<String> {
    store
        .read(|conn| {
            conn.query_row("SELECT reply_to_id FROM message WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .map_err(read_err)
        })
        .unwrap()
}

#[test]
fn a_reply_to_a_skipped_record_points_to_the_real_owner_of_its_external_id() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path());
    let export = parse_export(r#"{"type":"saved_messages","id":1000001,"messages":[]}"#).unwrap();
    // Entry "1" has already appeared (for example, by an import on another device) after `existing` was read.
    store
        .write(|conn| {
            conn.execute(
                "INSERT INTO message (id, chat_id, body_md, journal_date, created_at, created_tz, origin, hlc_json, schema_version, groups_raw) \
                 SELECT '7b2c9d10-1111-4222-8333-444455556666', id, 'уже есть', '2019-03-08', 1, 'UTC', 'import_telegram', '{}', 1, '{}' FROM chat LIMIT 1",
                [],
            )
            .map_err(write_err)?;
            crate::ext::insert(conn, EXT_SRC_TELEGRAM, "1000001", "1", OWNER_ID).map_err(write_err)
        })
        .unwrap();
    let first = prepared("1", "пропускается");
    let mut reply = prepared("2", "ответ на первую");
    reply.reply_to_id = Some(first.id.clone());
    let (skipped_id, reply_id) = (first.id.clone(), reply.id.clone());
    let outcome = store
        .write_batch(
            &export.chats[0],
            Some(INBOX_ID.to_string()),
            vec![first, reply],
            1_800_000_000_000,
            "UTC".into(),
        )
        .unwrap();
    assert_eq!(outcome.skipped, 1);
    assert_eq!(
        outcome.replaced.get(&skipped_id).map(String::as_str),
        Some(OWNER_ID)
    );
    assert_eq!(
        reply_of(&store, &reply_id).as_deref(),
        Some(OWNER_ID),
        "the reply goes to the real owner, not to the id of a nonexistent entry"
    );
}

#[test]
fn a_personal_chat_whose_first_batch_is_all_taken_is_not_created_empty() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path());
    let export =
        parse_export(r#"{"name":"Пётр","type":"personal_chat","id":2000002,"messages":[]}"#)
            .unwrap();
    for ext in ["1", "2"] {
        store
            .write(move |conn| {
                crate::ext::insert(conn, EXT_SRC_TELEGRAM, "2000002", ext, "someone")
                    .map_err(write_err)
            })
            .unwrap();
    }
    let count = |store: &Store| -> i64 {
        store
            .read(|conn| {
                conn.query_row("SELECT COUNT(*) FROM chat", [], |r| r.get(0))
                    .map_err(read_err)
            })
            .unwrap()
    };
    let before = count(&store);
    let outcome = store
        .write_batch(
            &export.chats[0],
            None,
            vec![prepared("1", "а"), prepared("2", "б")],
            1_800_000_000_000,
            "UTC".into(),
        )
        .unwrap();
    assert_eq!((outcome.skipped, outcome.created_chat), (2, false));
    assert_eq!(
        outcome.chat_id, None,
        "there is no destination chat: the next batch will create it if there is something to write"
    );
    assert_eq!(count(&store), before, "the empty chat is not created");
}
