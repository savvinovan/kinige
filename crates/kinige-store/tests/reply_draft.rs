//! Черновик с ответом (слайс 5): строка «Ответ на запись» переживает перезапуск и `kill -9`, очищается отправкой, миграция
//! 5 → 6 сохраняет черновики.

mod common;

use common::days::at;
use common::*;
use kinige_domain::ReplyState;
use kinige_domain::groups::INBOX_ID;
use kinige_store::{LATEST_VERSION, Now, Store, StoreError};
use std::process::Command;

const CHILD_ENV: &str = "KINIGE_REPLY_DRAFT_CHILD_DIR";

fn now() -> Now {
    at(2026, 10, 31, 12, 0)
}

fn old_entry(store: &Store) -> String {
    store
        .send_message(INBOX_ID, "Старая мысль", &at(2023, 10, 31, 12, 5))
        .unwrap()
        .id
}

#[test]
fn a_draft_keeps_the_reply_target_and_is_deleted_only_when_text_and_reply_are_both_empty() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let old = old_entry(&store);

    assert_eq!(store.chat_draft(INBOX_ID, &now()).unwrap().reply_to, None);
    store
        .save_chat_draft(INBOX_ID, "", Some(&old), &now())
        .unwrap();
    let only_reply = store.chat_draft(INBOX_ID, &now()).unwrap();
    assert_eq!(only_reply.body, "");
    let quote = only_reply.reply_to.expect("строка ответа без текста");
    assert_eq!(
        (quote.id.as_str(), quote.state, quote.years_later),
        (old.as_str(), ReplyState::Live, 3)
    );
    assert_eq!(quote.excerpt, "Старая мысль");

    store
        .save_chat_draft(INBOX_ID, "текст", Some(&old), &now())
        .unwrap();
    store
        .save_chat_draft(INBOX_ID, "текст", None, &now())
        .unwrap();
    let text_only = store.chat_draft(INBOX_ID, &now()).unwrap();
    assert_eq!(
        (text_only.body.as_str(), text_only.reply_to),
        ("текст", None),
        "отмена ответа убирает только ответ"
    );

    store.save_chat_draft(INBOX_ID, "", None, &now()).unwrap();
    let rows: i64 = raw(dir.path())
        .query_row("SELECT COUNT(*) FROM draft", [], |r| r.get(0))
        .unwrap();
    assert_eq!(rows, 0, "пусты и текст, и ответ — строки нет");
}

#[test]
fn sending_the_reply_clears_the_draft_with_its_reply_in_the_same_transaction() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let old = old_entry(&store);
    store
        .save_chat_draft(INBOX_ID, "ответ", Some(&old), &now())
        .unwrap();
    let sent = store
        .send_reply(INBOX_ID, "ответ", Some(&old), &now())
        .unwrap();
    assert!(sent.reply_to.is_some());
    let draft = store.chat_draft(INBOX_ID, &now()).unwrap();
    assert_eq!((draft.body.as_str(), draft.reply_to), ("", None));
}

#[test]
fn a_refused_reply_keeps_the_draft_and_its_reply_line() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let old = old_entry(&store);
    store
        .save_chat_draft(INBOX_ID, "ответ", Some(&old), &now())
        .unwrap();
    store.delete_message(&old, &now()).unwrap();
    let result = store.send_reply(INBOX_ID, "ответ", Some(&old), &now());
    assert!(
        matches!(result, Err(StoreError::ReplyTargetNotFound)),
        "{result:?}"
    );
    let draft = store.chat_draft(INBOX_ID, &now()).unwrap();
    assert_eq!(draft.body, "ответ", "текст остаётся в поле");
    assert_eq!(
        draft.reply_to.unwrap().state,
        ReplyState::Trash,
        "строка показывает «в корзине»"
    );
    store.purge_message(&old, &now()).unwrap();
    assert_eq!(
        store
            .chat_draft(INBOX_ID, &now())
            .unwrap()
            .reply_to
            .unwrap()
            .state,
        ReplyState::Missing
    );
}

#[test]
fn garbage_reply_ids_are_not_saved_into_a_draft() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    store
        .save_chat_draft(INBOX_ID, "черновик", None, &now())
        .unwrap();
    let huge = "x".repeat(1 << 20);
    for bad in [
        "",
        "не uuid",
        "5B1C0D9E-3F2A-4A7B-9C8D-1E2F3A4B5C6D",
        huge.as_str(),
    ] {
        let result = store.save_chat_draft(INBOX_ID, "другое", Some(bad), &now());
        assert!(
            matches!(result, Err(StoreError::ReplyTargetNotFound)),
            "{result:?}"
        );
    }
    let draft = store.chat_draft(INBOX_ID, &now()).unwrap();
    assert_eq!(
        (draft.body.as_str(), draft.reply_to),
        ("черновик", None),
        "прежний черновик цел"
    );
}

#[test]
fn the_capture_draft_never_carries_a_reply() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    store.set_capture_draft("из окна захвата", &now()).unwrap();
    let reply: Option<String> = raw(dir.path())
        .query_row(
            "SELECT reply_to_id FROM draft WHERE key = 'capture'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(reply, None);
}

#[test]
fn migration_five_to_six_keeps_drafts_and_takes_a_copy_first() {
    let dir = tempfile::tempdir().unwrap();
    {
        let store = open_without_mirror(dir.path());
        store
            .set_chat_draft(INBOX_ID, "черновик до миграции 6", &now())
            .unwrap();
        store
            .set_capture_draft("захват до миграции 6", &now())
            .unwrap();
    }
    wait_closed(dir.path());
    {
        let conn = raw(dir.path());
        conn.execute_batch("ALTER TABLE draft DROP COLUMN reply_to_id;")
            .unwrap();
        conn.pragma_update(None, "user_version", 5).unwrap();
    }
    let later = moscow(T0 + 10 * MINUTE);
    let mut opts = options(dir.path());
    opts.start_mirror = false;
    let store = Store::open_at(opts, &later).unwrap();
    let version: i64 = raw(dir.path())
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, LATEST_VERSION);
    let draft = store.chat_draft(INBOX_ID, &later).unwrap();
    assert_eq!(
        (draft.body.as_str(), draft.reply_to),
        ("черновик до миграции 6", None)
    );
    assert_eq!(store.get_capture_draft().unwrap(), "захват до миграции 6");
    let copies: Vec<String> =
        std::fs::read_dir(db_path(dir.path()).parent().unwrap().join("backups"))
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
    assert!(
        copies.iter().any(|n| n.starts_with("pre-migration-v5-")),
        "{copies:?}"
    );
}

/// Дочерний процесс: сохраняет черновик-ответ и падает через `abort` сразу после ответа (то есть после `COMMIT`).
#[test]
fn abort_right_after_saving_a_reply_draft_keeps_the_draft_and_its_reply_line() {
    if let Ok(dir) = std::env::var(CHILD_ENV) {
        let store = open_without_mirror(dir.as_ref());
        let old = old_entry(&store);
        store
            .save_chat_draft(INBOX_ID, "недописанный ответ", Some(&old), &now())
            .expect("черновик");
        std::process::abort();
    }
    let dir = tempfile::tempdir().unwrap();
    let status = Command::new(std::env::current_exe().unwrap())
        .args([
            "abort_right_after_saving_a_reply_draft_keeps_the_draft_and_its_reply_line",
            "--exact",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD_ENV, dir.path())
        .status()
        .unwrap();
    assert!(!status.success(), "дочерний процесс должен был упасть");

    let store = open_without_mirror(dir.path());
    let draft = store.chat_draft(INBOX_ID, &now()).unwrap();
    assert_eq!(draft.body, "недописанный ответ");
    let quote = draft.reply_to.expect("строка «Ответ на запись» на месте");
    assert_eq!(
        (quote.state, quote.excerpt.as_str()),
        (ReplyState::Live, "Старая мысль")
    );
}

fn wait_closed(dir: &std::path::Path) {
    // Фоновые потоки закрытого хранилища отпускают файл базы не мгновенно.
    wait_until("база свободна", || {
        raw(dir)
            .query_row("SELECT COUNT(*) FROM sqlite_master", [], |r| {
                r.get::<_, i64>(0)
            })
            .is_ok()
    });
}
