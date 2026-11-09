//! The search index and `message_ext` (slice 4, task 4.6): migration 3 -> 4 with a copy, the index matches the text after
//! every operation, a rebuild on a version change, a rollback together with the write, `abort` right after the commit.

// The integration test root is test code as a whole: `cfg(test)` gives helpers outside #[test] the same clippy.toml
// relaxations (unwrap/expect/panic/indexing) as the tests in src/.
#![cfg(test)]

mod common;

use common::old_db::downgrade_to;
use common::*;
use kinige_domain::MessageFlag;
use kinige_domain::groups::INBOX_ID;
use kinige_domain::history::DAY_MS;
use kinige_domain::search::{SEARCH_INDEX_VERSION, fts_query};
use kinige_store::{LATEST_VERSION, Store};
use std::path::Path;
use std::process::Command;

/// Ids of the entries found by a query in the index (without the trash and ignoring order).
fn found(dir: &Path, input: &str) -> Vec<String> {
    let query = fts_query(input).expect("the query has words");
    let conn = raw(dir);
    let mut statement = conn
        .prepare(
            "SELECT mp.message_id FROM search_fts f JOIN search_map mp ON mp.rowid = f.rowid \
             WHERE search_fts MATCH ?1 ORDER BY mp.message_id",
        )
        .unwrap();
    statement
        .query_map([query], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

fn count(dir: &Path, table: &str) -> i64 {
    raw(dir)
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
        .unwrap()
}

fn user_version(dir: &Path) -> i64 {
    raw(dir)
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap()
}

/// The slice 3 database: schema 3 as migrations 1-3 make it (no migration 4 tables), `user_version = 3`, with the rows it had.
fn downgrade_to_three(dir: &Path) {
    downgrade_to(dir, 3);
}

#[test]
fn migration_four_takes_a_copy_and_builds_the_index_and_ext_from_old_records() {
    let dir = tempfile::tempdir().unwrap();
    let (first, second, third, imported) = {
        let store = open_without_mirror(dir.path());
        let a = store
            .send_message(INBOX_ID, "Купить молоко", &moscow(T0))
            .unwrap()
            .id;
        let b = store
            .send_message(INBOX_ID, "Книгу про ёлку", &moscow(T0 + MINUTE))
            .unwrap()
            .id;
        let c = store
            .send_message(INBOX_ID, "Удалённая вещь", &moscow(T0 + 2 * MINUTE))
            .unwrap()
            .id;
        store.delete_message(&c, &moscow(T0 + 3 * MINUTE)).unwrap();
        let d = store
            .send_message(INBOX_ID, "Из телеграма", &moscow(T0 + 4 * MINUTE))
            .unwrap()
            .id;
        (a, b, c, d)
    };
    // An entry with an external id, as an import leaves it (the `ext_json` field), and a corrupted `ext_json` row of another entry.
    raw(dir.path())
        .execute(
            "UPDATE message SET ext_json = '{\"chat\":\"9223372036854775807\",\"msg\":\"-5\",\"src\":\"telegram\"}' \
             WHERE id = ?1",
            [&imported],
        )
        .unwrap();
    raw(dir.path())
        .execute(
            "UPDATE message SET ext_json = '{не json' WHERE id = ?1",
            [&first],
        )
        .unwrap();
    downgrade_to_three(dir.path());

    let later = moscow(T0 + 10 * MINUTE);
    let _store = Store::open_at(options(dir.path()), &later).unwrap();
    assert_eq!(user_version(dir.path()), LATEST_VERSION);
    // The copy was taken before the migration, it has schema 3.
    let copies: Vec<String> =
        std::fs::read_dir(db_path(dir.path()).parent().unwrap().join("backups"))
            .unwrap()
            .flatten()
            .map(|e| e.file_name().into_string().unwrap())
            .collect();
    assert_eq!(
        copies,
        [format!(
            "pre-migration-v3-{}.db",
            kinige_store::utc_stamp(later.ms)
        )]
    );
    let copy = db_path(dir.path())
        .parent()
        .unwrap()
        .join("backups")
        .join(&copies[0]);
    let in_copy: i64 = rusqlite::Connection::open(&copy)
        .unwrap()
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(in_copy, 3);

    // The index is full: live entries and the deleted one (the query cuts off the trash, not the index).
    assert_eq!(count(dir.path(), "search_map"), 4);
    assert_eq!(found(dir.path(), "молоко "), vec![first.clone()]);
    assert_eq!(found(dir.path(), "книги "), vec![second.clone()]);
    assert_eq!(found(dir.path(), "елка "), vec![second]);
    assert_eq!(found(dir.path(), "удалённая"), vec![third]);
    assert_eq!(found(dir.path(), "моло"), vec![first]);
    assert_eq!(
        raw(dir.path())
            .query_row(
                "SELECT value FROM app_setting WHERE key = 'search_index_version'",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
        SEARCH_INDEX_VERSION.to_string()
    );
    // `message_ext` is built from `ext_json`; a corrupted `ext_json` yields no rows.
    let ext: Vec<(String, String, String, String)> = raw(dir.path())
        .prepare("SELECT src, ext_chat, ext_msg, message_id FROM message_ext")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        ext,
        [(
            "telegram".to_string(),
            "9223372036854775807".to_string(),
            "-5".to_string(),
            imported
        )]
    );
}

#[test]
fn index_follows_send_append_edit_toggle_and_purge() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    // Send.
    let sent = store
        .send_message(
            INBOX_ID,
            "Сегодня аксолотль\n- [ ] позвонить маме",
            &moscow(T0),
        )
        .unwrap();
    assert_eq!(found(dir.path(), "аксолотль "), vec![sent.id.clone()]);
    assert_eq!(found(dir.path(), "позвонить мама "), vec![sent.id.clone()]);
    // Edit: the old word is gone, the new one is found.
    store
        .edit_message(
            &sent.id,
            "Теперь бегемотик\n- [ ] позвонить маме",
            &moscow(T0 + MINUTE),
        )
        .unwrap();
    assert!(found(dir.path(), "аксолотль ").is_empty());
    assert_eq!(found(dir.path(), "бегемотик "), vec![sent.id.clone()]);
    // The `- [ ]` checkbox: the words are the same, the index neither breaks nor duplicates.
    store
        .toggle_task_item(&sent.id, 0, true, &moscow(T0 + 2 * MINUTE))
        .unwrap();
    assert_eq!(found(dir.path(), "бегемотик "), vec![sent.id.clone()]);
    assert_eq!(count(dir.path(), "search_map"), 1);
    // Appending from the capture window (the entry was created by the user, not older than ten minutes).
    let outcome = store
        .capture_append("и ещё фламинго", &moscow(T0 + 3 * MINUTE))
        .unwrap();
    assert_eq!(outcome.message.id, sent.id, "appended to the same entry");
    assert_eq!(found(dir.path(), "фламинго "), vec![sent.id.clone()]);
    assert_eq!(found(dir.path(), "бегемотик "), vec![sent.id.clone()]);
    // Trash: the index stays (the entry can be restored), a permanent delete removes it.
    store
        .delete_message(&sent.id, &moscow(T0 + 4 * MINUTE))
        .unwrap();
    assert_eq!(found(dir.path(), "бегемотик "), vec![sent.id.clone()]);
    store
        .purge_message(&sent.id, &moscow(T0 + 5 * MINUTE))
        .unwrap();
    assert!(found(dir.path(), "бегемотик ").is_empty());
    assert!(found(dir.path(), "фламинго ").is_empty());
    assert_eq!(count(dir.path(), "search_map"), 0);
    // The index has no rows at all: both the documents and their mapping rows are deleted.
    let rows: i64 = raw(dir.path())
        .query_row("SELECT COUNT(*) FROM search_fts_docsize", [], |r| r.get(0))
        .unwrap();
    assert_eq!(rows, 0);
}

#[test]
fn flags_date_and_move_do_not_touch_the_index_but_it_stays_consistent() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let sent = store
        .send_message(INBOX_ID, "Пеликан идёт", &moscow(T0))
        .unwrap();
    let chat = store.create_chat("Птицы", None, &moscow(T0)).unwrap();
    store
        .set_message_flag(&sent.id, MessageFlag::Favorite, true, &moscow(T0 + MINUTE))
        .unwrap();
    store
        .set_message_date(&sent.id, "2026-10-01", &moscow(T0 + 2 * MINUTE))
        .unwrap();
    store
        .move_message(&sent.id, &chat.id, &moscow(T0 + DAY_MS))
        .unwrap();
    assert_eq!(found(dir.path(), "пеликан "), [sent.id]);
    assert_eq!(count(dir.path(), "search_map"), 1);
}

#[test]
fn changing_the_version_rebuilds_the_index_on_open() {
    let dir = tempfile::tempdir().unwrap();
    let id = {
        let store = open_without_mirror(dir.path());
        store
            .send_message(INBOX_ID, "Вермишель и мармелад", &moscow(T0))
            .unwrap()
            .id
    };
    // The index is spoiled by "old rules": empty and with a different version.
    {
        let conn = raw(dir.path());
        conn.execute(
            "INSERT INTO search_fts (search_fts) VALUES ('delete-all')",
            [],
        )
        .unwrap();
        conn.execute("DELETE FROM search_map", []).unwrap();
        conn.execute(
            "UPDATE app_setting SET value = '0' WHERE key = 'search_index_version'",
            [],
        )
        .unwrap();
    }
    assert!(found(dir.path(), "вермишель ").is_empty());
    let _store = open_without_mirror(dir.path());
    assert_eq!(found(dir.path(), "вермишель "), vec![id.clone()]);
    assert_eq!(found(dir.path(), "мармелад "), [id]);
    // The version matched: reopening does not touch the index (the mark of the mapping row id does not change).
    let before: i64 = raw(dir.path())
        .query_row("SELECT rowid FROM search_map", [], |r| r.get(0))
        .unwrap();
    let _again = open_without_mirror(dir.path());
    let after: i64 = raw(dir.path())
        .query_row("SELECT rowid FROM search_map", [], |r| r.get(0))
        .unwrap();
    assert_eq!(before, after);
}

#[test]
fn a_failing_index_write_rolls_back_the_record_too() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let kept = store
        .send_message(INBOX_ID, "Остаётся", &moscow(T0))
        .unwrap();
    // An index write failure (like `ops_abort.rs`: a trigger): a new entry must not be left without an index.
    raw(dir.path())
        .execute_batch(
            "CREATE TRIGGER fail_index BEFORE INSERT ON search_map BEGIN SELECT RAISE(ABORT, 'index down'); END;",
        )
        .unwrap();
    let error = store
        .send_message(INBOX_ID, "Не запишется", &moscow(T0 + MINUTE))
        .unwrap_err();
    assert_eq!(error.code(), kinige_domain::ErrorCode::StoreWriteFailed);
    assert_eq!(count(dir.path(), "message"), 1);
    assert_eq!(count(dir.path(), "search_map"), 1);
    assert!(found(dir.path(), "запишется ").is_empty());
    // Edit: a failure after the index row is replaced (the copy mark follows in the same transaction) rolls back the index too.
    raw(dir.path())
        .execute_batch(
            "DROP TRIGGER fail_index; \
             CREATE TRIGGER fail_after_index BEFORE INSERT ON mirror_dirty \
             BEGIN SELECT RAISE(ABORT, 'mirror down'); END;",
        )
        .unwrap();
    assert!(
        store
            .edit_message(&kept.id, "Изменённый текст", &moscow(T0 + 2 * MINUTE))
            .is_err()
    );
    assert_eq!(found(dir.path(), "остаётся "), [kept.id]);
    assert!(found(dir.path(), "изменённый ").is_empty());
}

const CHILD_ENV: &str = "KINIGE_SEARCH_ABORT_DIR";

#[test]
fn abort_right_after_commit_keeps_the_index() {
    if let Ok(dir) = std::env::var(CHILD_ENV) {
        let store = open_without_mirror(dir.as_ref());
        store
            .send_message(INBOX_ID, "Переживёт саламандра", &moscow(T0))
            .expect("send");
        std::process::abort();
    }
    let dir = tempfile::tempdir().unwrap();
    let status = Command::new(std::env::current_exe().unwrap())
        .args([
            "abort_right_after_commit_keeps_the_index",
            "--exact",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD_ENV, dir.path())
        .status()
        .unwrap();
    assert!(!status.success(), "the child process should have crashed");
    let ids = found(dir.path(), "саламандры ");
    assert_eq!(ids.len(), 1, "the index survived abort");
    let quick: String = raw(dir.path())
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(quick, "ok");
    let store = open_without_mirror(dir.path());
    assert_eq!(
        store
            .list_messages(INBOX_ID, None, 10, &moscow(T0 + MINUTE))
            .unwrap()
            .messages[0]
            .id,
        ids[0]
    );
}
