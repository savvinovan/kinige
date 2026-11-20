//! Restoring from a copy and agent access (slice 5d, ADR-0022): a restore can only narrow access; the copy folder is the copy's own;
//! the old `settings_file_hash` row is neither read nor written.

// The integration test root is test code as a whole: `cfg(test)` gives helpers outside #[test] the same clippy.toml
// relaxations (unwrap/expect/panic/indexing) as the tests in src/.
#![cfg(test)]

mod common;

use common::agent::{call, log_rows};
use common::ops::*;
use common::*;
use kinige_domain::AgentMode;
use kinige_domain::TextSize;
use kinige_domain::groups::INBOX_ID;
use kinige_store::{Now, backups, restore_backup};
use serde_json::json;
use std::fs;
use std::path::Path;

const BACKUP_MS: i64 = 1_700_000_000_000;

fn at(ms: i64) -> Now {
    moscow(ms)
}

fn setting(dir: &Path, key: &str) -> Option<String> {
    raw(dir)
        .query_row("SELECT value FROM app_setting WHERE key = ?1", [key], |r| {
            r.get(0)
        })
        .ok()
}

/// A database whose copy holds: access on, "Read and write", three visible chats, a chosen copy folder, a large text size and the
/// old file hash row.
fn database_with_open_access(dir: &Path, folder: &Path) -> (String, Vec<String>) {
    let store = open_without_mirror(dir);
    store.send_message(INBOX_ID, "из копии", &at(T0)).unwrap();
    let chats: Vec<String> = ["Работа", "Дом", "Книги"]
        .into_iter()
        .map(|title| store.create_chat(title, None, &at(T0)).unwrap().id)
        .collect();
    store.set_agent_chats(chats.clone()).unwrap();
    store.set_agent_mode(AgentMode::ReadWrite).unwrap();
    store.set_agent_enabled(true).unwrap();
    store.set_mirror_dir(folder).unwrap();
    store.set_text_size(TextSize::Large).unwrap();
    raw(dir)
        .execute(
            "INSERT INTO app_setting(key, value) VALUES ('settings_file_hash', 'old-hash-value')",
            [],
        )
        .unwrap();
    store.run_maintenance(&at(T0)).unwrap();
    drop(store);
    wait_closed(dir);
    (backups(&db_path(dir))[0].name.clone(), chats)
}

#[test]
fn a_restore_leaves_access_off_no_chat_visible_and_the_mode_read_whatever_the_copy_held() {
    let dir = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let folder = elsewhere.path().join("Kinige-B");
    let day = folder.join("Inbox/2026/2026-10-22.md");
    fs::create_dir_all(day.parent().unwrap()).unwrap();
    fs::write(&day, "запись после снимка").unwrap();
    let (name, _chats) = database_with_open_access(dir.path(), &folder);
    assert_eq!(setting(dir.path(), "mcp_enabled").as_deref(), Some("1"));

    let report = restore_backup(
        &db_path(dir.path()),
        &name,
        BACKUP_MS,
        Some(&mirror_dir(dir.path())),
    )
    .unwrap();

    assert_eq!(setting(dir.path(), "mcp_enabled").as_deref(), Some("0"));
    assert_eq!(setting(dir.path(), "mcp_chats").as_deref(), Some("[]"));
    assert_eq!(setting(dir.path(), "mcp_mode").as_deref(), Some("read"));
    // The copy folder is the copy's own; the folder that was there moved aside with its extra day.
    assert_eq!(
        setting(dir.path(), "mirror_dir").as_deref(),
        folder.to_str()
    );
    assert!(!folder.exists());
    let moved = report.previous_mirror.expect("the folder moved aside");
    assert_eq!(
        fs::read_to_string(moved.join("Inbox/2026/2026-10-22.md")).unwrap(),
        "запись после снимка"
    );
    // The rest of the copy is carried over, and the retired hash row is untouched.
    assert_eq!(setting(dir.path(), "text_size").as_deref(), Some("large"));
    assert_eq!(
        setting(dir.path(), "settings_file_hash").as_deref(),
        Some("old-hash-value")
    );
}

#[test]
fn after_a_restore_every_call_is_refused_until_the_user_switches_access_on_and_then_no_chat_is_visible()
 {
    let dir = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let (name, _) = database_with_open_access(dir.path(), &elsewhere.path().join("B"));
    restore_backup(
        &db_path(dir.path()),
        &name,
        BACKUP_MS,
        Some(&mirror_dir(dir.path())),
    )
    .unwrap();
    let store = open_without_mirror(dir.path());
    for tool in ["get_settings", "set_setting", "list_chats"] {
        let result = call(
            &store,
            tool,
            json!({"key": "text_size", "value": "normal"}),
            T0,
        );
        assert_eq!(result, Err(kinige_store::AgentError::Disabled), "{tool}");
    }
    // The user turns access on in the window: the chats of the copy are gone, the mode is read.
    store.set_agent_enabled(true).unwrap();
    let reply = call(&store, "get_settings", json!({}), T0).unwrap();
    assert_eq!(reply["agent_access"]["mode"], "read");
    assert_eq!(reply["agent_access"]["visible_chat_ids"], json!([]));
    assert_eq!(
        call(
            &store,
            "set_setting",
            json!({"key": "text_size", "value": "normal"}),
            T0
        ),
        Err(kinige_store::AgentError::ReadOnly)
    );
    // Three refusals before, then one read and one refusal: one row per call.
    assert_eq!(log_rows(dir.path()).len(), 5);
}
