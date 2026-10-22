//! Черновики: сохранение, очистка в транзакции отправки, перезапуск; не попадают в Markdown-копию.

mod common;

use common::*;
use kinige_domain::groups::INBOX_ID;
use kinige_store::StoreError;

#[test]
fn drafts_are_stored_replaced_removed_and_survive_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    {
        let store = open_without_mirror(dir.path());
        assert_eq!(store.get_capture_draft().unwrap(), "");
        assert_eq!(store.get_chat_draft(INBOX_ID).unwrap(), "");
        store.set_capture_draft("раз", &moscow(T0)).unwrap();
        store.set_capture_draft("раз два", &moscow(T0 + 1)).unwrap();
        store
            .set_chat_draft(INBOX_ID, "в главном\nокне", &moscow(T0 + 2))
            .unwrap();
    }
    let store = open_without_mirror(dir.path());
    assert_eq!(store.get_capture_draft().unwrap(), "раз два");
    assert_eq!(store.get_chat_draft(INBOX_ID).unwrap(), "в главном\nокне");
    let rows: i64 = raw(dir.path())
        .query_row("SELECT COUNT(*) FROM draft", [], |r| r.get(0))
        .unwrap();
    assert_eq!(rows, 2, "по строке на ключ");

    store.set_capture_draft("", &moscow(T0 + 3)).unwrap();
    assert_eq!(store.get_capture_draft().unwrap(), "");
    let rows: i64 = raw(dir.path())
        .query_row("SELECT COUNT(*) FROM draft", [], |r| r.get(0))
        .unwrap();
    assert_eq!(rows, 1, "пустой текст удаляет строку");
}

#[test]
fn a_draft_for_a_missing_chat_is_refused_and_an_oversized_one_too() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    assert!(matches!(
        store.set_chat_draft("нет-такого-чата", "x", &moscow(T0)),
        Err(StoreError::ChatNotFound)
    ));
    let rows: i64 = raw(dir.path())
        .query_row("SELECT COUNT(*) FROM draft", [], |r| r.get(0))
        .unwrap();
    assert_eq!(rows, 0);
    let huge = "x".repeat(kinige_store::MAX_DRAFT_BYTES + 1);
    assert!(matches!(
        store.set_capture_draft(&huge, &moscow(T0)),
        Err(StoreError::TooLarge)
    ));
    assert_eq!(store.get_capture_draft().unwrap(), "");
}

#[test]
fn sending_from_the_main_window_clears_only_that_chat_draft_in_the_same_transaction() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    store
        .set_chat_draft(INBOX_ID, "готово к отправке", &moscow(T0))
        .unwrap();
    store.set_capture_draft("чужой", &moscow(T0)).unwrap();
    store
        .send_message(INBOX_ID, "готово к отправке", &moscow(T0 + 1))
        .unwrap();
    assert_eq!(store.get_chat_draft(INBOX_ID).unwrap(), "");
    assert_eq!(store.get_capture_draft().unwrap(), "чужой");

    // Отказ записи: черновик остаётся вместе с текстом.
    store
        .set_chat_draft(INBOX_ID, "не отправится", &moscow(T0 + 2))
        .unwrap();
    let conn = raw(dir.path());
    conn.execute_batch(
        "CREATE TRIGGER fail_dirty BEFORE UPDATE ON mirror_dirty BEGIN SELECT RAISE(ABORT, 'нет места'); END;",
    )
    .unwrap();
    assert!(
        store
            .send_message(INBOX_ID, "не отправится", &moscow(T0 + 3))
            .is_err()
    );
    assert_eq!(store.get_chat_draft(INBOX_ID).unwrap(), "не отправится");
}

#[test]
fn a_failed_clipboard_goes_below_the_saved_capture_draft_and_never_replaces_it() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());

    // Пустого черновика нет: в поле только текст буфера.
    let (text, saved) = store
        .merge_clipboard_into_capture_draft("первый буфер", &moscow(T0))
        .unwrap();
    assert_eq!((text.as_str(), saved), ("первый буфер", true));
    assert_eq!(store.get_capture_draft().unwrap(), "первый буфер");

    // Есть черновик: прежний текст сохраняется, буфер — ниже отдельным абзацем.
    store
        .set_capture_draft("недописанная мысль", &moscow(T0 + 1))
        .unwrap();
    let (text, saved) = store
        .merge_clipboard_into_capture_draft("второй буфер", &moscow(T0 + 2))
        .unwrap();
    assert_eq!(text, "недописанная мысль\n\nвторой буфер");
    assert!(saved);
    assert_eq!(store.get_capture_draft().unwrap(), text);

    // Повторный сбой не теряет ни прежнее, ни первый буфер.
    let (text, _) = store
        .merge_clipboard_into_capture_draft("третий буфер", &moscow(T0 + 3))
        .unwrap();
    assert_eq!(text, "недописанная мысль\n\nвторой буфер\n\nтретий буфер");
}

#[test]
fn a_clipboard_that_does_not_fit_the_draft_limit_keeps_the_stored_draft_and_returns_the_text() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    store.set_capture_draft("прежний", &moscow(T0)).unwrap();
    let huge = "x".repeat(kinige_store::MAX_DRAFT_BYTES);
    let (text, saved) = store
        .merge_clipboard_into_capture_draft(&huge, &moscow(T0 + 1))
        .unwrap();
    assert!(!saved, "не поместилось в базу");
    assert!(text.starts_with("прежний\n\n") && text.len() > huge.len());
    assert_eq!(
        store.get_capture_draft().unwrap(),
        "прежний",
        "в базе прежний черновик цел"
    );
}

#[test]
fn drafts_are_not_part_of_the_markdown_copy_or_the_message_tables() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path());
    store
        .set_capture_draft("СЕКРЕТНЫЙ ЧЕРНОВИК", &moscow(T0))
        .unwrap();
    store
        .send_message(INBOX_ID, "обычная", &moscow(T0))
        .unwrap();
    let file = mirror_dir(dir.path())
        .join("Inbox")
        .join("2026")
        .join("2026-10-21.md");
    wait_until("копия", || file.exists());
    assert!(
        !std::fs::read_to_string(&file)
            .unwrap()
            .contains("СЕКРЕТНЫЙ")
    );
    let in_messages: i64 = raw(dir.path())
        .query_row(
            "SELECT COUNT(*) FROM message WHERE body_md LIKE '%СЕКРЕТНЫЙ%' OR groups_raw LIKE '%СЕКРЕТНЫЙ%'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(in_messages, 0);
}
