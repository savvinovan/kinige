//! Migration 1 -> 2 and process abort (`abort`) after the commit of an append and after saving a draft.

// The root of the integration tests is entirely test code: `cfg(test)` gives helpers outside #[test] the same
// clippy.toml relaxations (unwrap/expect/panic/indexing) as the tests in src/.
#![cfg(test)]

mod common;

use common::capture::*;
use common::*;
use kinige_domain::groups::INBOX_ID;
use kinige_store::{LATEST_VERSION, Store};
use std::path::Path;
use std::process::Command;

#[test]
fn migration_two_adds_only_the_draft_table_and_keeps_every_existing_row() {
    let dir = tempfile::tempdir().unwrap();
    let sent = {
        let store = open_without_mirror(dir.path());
        store
            .send_message(INBOX_ID, "до миграции", &moscow(T0))
            .unwrap()
    };
    let before = row(dir.path(), &sent.id);
    // Return the database to the slice 1 state: without the drafts table and with `user_version = 1`.
    {
        let conn = raw(dir.path());
        conn.execute_batch(
            "DROP TABLE draft; DROP INDEX message_deleted; DROP TABLE message_revision; DROP TABLE tombstone; \
             DROP TABLE message_tag; DROP TABLE mirror_chat; DROP TABLE message_ext; DROP TABLE search_fts; \
             DROP TABLE search_map; DROP TABLE mcp_log; DROP TABLE mcp_log_message; \
             DROP TABLE attachment; DROP TABLE attachment_local; DROP TABLE draft_attachment; DROP TABLE mirror_file; \
             DELETE FROM app_setting WHERE key = 'search_index_version';",
        )
        .unwrap();
        conn.pragma_update(None, "user_version", 1).unwrap();
    }
    let store = open_without_mirror(dir.path());
    let conn = raw(dir.path());
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, LATEST_VERSION);
    assert_eq!(LATEST_VERSION, 7);
    let columns: Vec<(String, String)> = conn
        .prepare("SELECT name, type FROM pragma_table_info('draft') ORDER BY cid")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        columns,
        [
            ("key".to_string(), "TEXT".to_string()),
            ("body".to_string(), "TEXT".to_string()),
            ("updated_at".to_string(), "INTEGER".to_string()),
            ("reply_to_id".to_string(), "TEXT".to_string())
        ]
    );
    let after = row(dir.path(), &sent.id);
    assert_eq!(after.groups_raw, before.groups_raw);
    assert_eq!(after.hlc_json, before.hlc_json);
    assert_eq!(after.body_md, "до миграции");
    assert_eq!(store.list_chats().unwrap()[0].count, 1);
    // Reopening changes nothing.
    drop(store);
    drop(open_without_mirror(dir.path()));
}

// ---------- process abort ----------

const CHILD_ENV: &str = "KINIGE_CAPTURE_ABORT_DIR";
const CHILD_MODE: &str = "KINIGE_CAPTURE_ABORT_MODE";

fn run_child(test: &str, dir: &Path, mode: &str) {
    let status = Command::new(std::env::current_exe().unwrap())
        .args([test, "--exact", "--nocapture", "--test-threads=1"])
        .env(CHILD_ENV, dir)
        .env(CHILD_MODE, mode)
        .status()
        .unwrap();
    assert!(!status.success(), "the child process should have died");
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(status.signal(), Some(6), "SIGABRT");
    }
}

#[test]
fn abort_right_after_an_append_commit_keeps_the_rewritten_entry() {
    if let Ok(dir) = std::env::var(CHILD_ENV) {
        let store: Store = open_without_mirror(dir.as_ref());
        store.capture_send("основа", &moscow(T0)).unwrap();
        store
            .set_capture_draft("не отправлено", &moscow(T0))
            .unwrap();
        store
            .capture_append("дописано", &moscow(T0 + MINUTE))
            .unwrap();
        std::process::abort();
    }
    let dir = tempfile::tempdir().unwrap();
    run_child(
        "abort_right_after_an_append_commit_keeps_the_rewritten_entry",
        dir.path(),
        "append",
    );

    let store = open_without_mirror(dir.path());
    let view = store.capture_view(&moscow(T0 + 2 * MINUTE)).unwrap();
    assert_eq!(view.context.len(), 1);
    assert_eq!(view.context[0].body_md, "основа\n\nдописано");
    assert_eq!(
        view.draft, "",
        "the draft was cleared by the same transaction"
    );
    let after = row(dir.path(), &view.context[0].id);
    assert_eq!(after.edited_at, Some(T0 + MINUTE));
    assert!(after.body_supersedes.starts_with("[\""));
    let dirty: i64 = raw(dir.path())
        .query_row("SELECT COUNT(*) FROM mirror_dirty", [], |r| r.get(0))
        .unwrap();
    assert_eq!(dirty, 1, "the copy mark survived the abort");
}

#[test]
fn abort_right_after_saving_a_draft_keeps_the_draft() {
    if let Ok(dir) = std::env::var(CHILD_ENV) {
        let store: Store = open_without_mirror(dir.as_ref());
        store
            .set_capture_draft("писал, писал…\nи вдруг", &moscow(T0))
            .unwrap();
        store
            .set_chat_draft(INBOX_ID, "и в главном", &moscow(T0))
            .unwrap();
        std::process::abort();
    }
    let dir = tempfile::tempdir().unwrap();
    run_child(
        "abort_right_after_saving_a_draft_keeps_the_draft",
        dir.path(),
        "draft",
    );
    let store = open_without_mirror(dir.path());
    assert_eq!(store.get_capture_draft().unwrap(), "писал, писал…\nи вдруг");
    assert_eq!(store.get_chat_draft(INBOX_ID).unwrap(), "и в главном");
}
