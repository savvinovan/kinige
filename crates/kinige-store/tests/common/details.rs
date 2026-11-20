//! Helpers of the slice 7b tests: an entry with a photo and a PDF, the three groups of an attachment, changes as JSON.

#![allow(dead_code)]

use super::attach::{jpeg, write};
use super::raw;
use kinige_domain::AttachmentView;
use kinige_domain::MessageView;
use kinige_domain::attachment::AttachmentDetailsChanges;
use kinige_domain::groups::INBOX_ID;
use kinige_store::{Now, Store};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::Path;

/// `AttachmentDetailsChanges` as a command receives it: parsed from JSON, so that "absent" and `null` are what IPC gives.
pub fn changes(value: Value) -> AttachmentDetailsChanges {
    serde_json::from_value(value).expect("changes that parse")
}

/// An entry in Inbox with the photo `IMG_0001.jpg` and the file `menu.pdf`, in that order.
pub fn photo_and_pdf(store: &Store, dir: &Path, when: &Now) -> MessageView {
    let source = dir.join("src");
    std::fs::create_dir_all(&source).unwrap();
    let photo = store
        .attach_path(
            INBOX_ID,
            &write(&source, "IMG_0001.jpg", &jpeg(640, 480)),
            when,
        )
        .unwrap();
    let pdf = store
        .attach_path(INBOX_ID, &write(&source, "menu.pdf", b"%PDF-1.4"), when)
        .unwrap();
    store
        .send_entry(INBOX_ID, "подпись", None, &[photo.id, pdf.id], when)
        .unwrap()
}

/// The groups of an attachment verbatim: `name -> bytes`.
pub fn groups(dir: &Path, id: &str) -> BTreeMap<String, String> {
    let text: String = raw(dir)
        .query_row(
            "SELECT groups_raw FROM attachment WHERE id = ?1",
            [id],
            |r| r.get(0),
        )
        .unwrap();
    serde_json::from_str::<BTreeMap<String, Box<serde_json::value::RawValue>>>(&text)
        .unwrap()
        .into_iter()
        .map(|(name, bytes)| (name, bytes.get().to_string()))
        .collect()
}

/// `{"<group>": {"h": .., "sv": ..}}` of an attachment, as values.
pub fn hlcs(dir: &Path, id: &str) -> BTreeMap<String, Value> {
    let text: String = raw(dir)
        .query_row("SELECT hlc_json FROM attachment WHERE id = ?1", [id], |r| {
            r.get(0)
        })
        .unwrap();
    serde_json::from_str(&text).unwrap()
}

/// The HLC of one group of an attachment.
pub fn hlc_of(dir: &Path, id: &str, group: &str) -> String {
    hlcs(dir, id)[group]["h"].as_str().unwrap().to_string()
}

/// `(display_name, description, spoiler)` columns of an attachment.
pub fn columns(dir: &Path, id: &str) -> (Option<String>, Option<String>, i64) {
    raw(dir)
        .query_row(
            "SELECT display_name, description, spoiler FROM attachment WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap()
}

/// Makes the row look as one written before 7b (and so before 5c): no `name`, `description`, `privacy` or `sticker` group, default columns.
pub fn make_legacy(dir: &Path, id: &str) {
    let conn = raw(dir);
    let (groups_raw, hlc_json): (String, String) = conn
        .query_row(
            "SELECT groups_raw, hlc_json FROM attachment WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    let mut raw_groups: BTreeMap<String, Value> = serde_json::from_str(&groups_raw).unwrap();
    let mut hlc_entries: BTreeMap<String, Value> = serde_json::from_str(&hlc_json).unwrap();
    for group in ["name", "description", "privacy", "sticker"] {
        raw_groups.remove(group);
        hlc_entries.remove(group);
    }
    conn.execute(
        "UPDATE attachment SET groups_raw = ?1, hlc_json = ?2, display_name = NULL, description = NULL, spoiler = 0, \
         sticker = 0, sticker_emoji = NULL WHERE id = ?3",
        (
            serde_json::to_string(&raw_groups).unwrap(),
            serde_json::to_string(&hlc_entries).unwrap(),
            id,
        ),
    )
    .unwrap();
}

/// Replaces the bytes of one group of an attachment, with `sv` for its `hlc_json` entry (a foreign client's value).
pub fn set_group(dir: &Path, id: &str, group: &str, bytes: &str, sv: u64) {
    let conn = raw(dir);
    let (groups_raw, hlc_json): (String, String) = conn
        .query_row(
            "SELECT groups_raw, hlc_json FROM attachment WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    let mut raw_groups: BTreeMap<String, Box<serde_json::value::RawValue>> =
        serde_json::from_str(&groups_raw).unwrap();
    let mut hlc_entries: BTreeMap<String, Value> = serde_json::from_str(&hlc_json).unwrap();
    let h = hlc_entries.get(group).map_or_else(
        || json!("0001793000000-00000-5f0c3e2a-9b1d-4c7e-8a64-0d2f1b3c4e5f"),
        |e| e["h"].clone(),
    );
    raw_groups.insert(
        group.to_string(),
        serde_json::value::RawValue::from_string(bytes.to_string()).unwrap(),
    );
    hlc_entries.insert(group.to_string(), json!({"h": h, "sv": sv}));
    let (groups_text, hlc_text) = (
        serde_json::to_string(&raw_groups).unwrap(),
        serde_json::to_string(&hlc_entries).unwrap(),
    );
    // The columns hold the read function's output (what a pull will do in slice 11), so a bypass keeps them in step.
    let (kind, filename): (String, String) = conn
        .query_row(
            "SELECT kind, filename FROM attachment WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    let details = kinige_domain::attachment::read_details(
        kinige_domain::attachment::AttachmentKind::parse(&kind).unwrap(),
        &filename,
        &groups_text,
        &hlc_text,
    );
    conn.execute(
        "UPDATE attachment SET groups_raw = ?1, hlc_json = ?2, display_name = ?3, description = ?4, spoiler = ?5 WHERE id = ?6",
        rusqlite::params![
            groups_text,
            hlc_text,
            details.display_name,
            details.description,
            details.spoiler,
            id
        ],
    )
    .unwrap();
}

/// The attachment with `id` of a freshly read entry.
pub fn view_in_feed(store: &Store, message_id: &str, id: &str, now: &Now) -> AttachmentView {
    store
        .list_messages(INBOX_ID, None, 50, now)
        .unwrap()
        .messages
        .into_iter()
        .find(|m| m.id == message_id)
        .expect("the entry is in the feed")
        .attachments
        .into_iter()
        .find(|a| a.id == id)
        .expect("the attachment is in the entry")
}

/// One number from the database: `SELECT COUNT(*) ...`.
pub fn count(dir: &Path, sql: &str) -> i64 {
    raw(dir).query_row(sql, [], |r| r.get(0)).unwrap()
}

/// A few lines the tests use to say what a group holds.
pub fn json_of(text: &str) -> Value {
    serde_json::from_str(text).unwrap()
}
