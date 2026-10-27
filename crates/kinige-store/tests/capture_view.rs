//! Окно захвата на настоящем SQLite: контекст за сегодня и новая запись из окна.

mod common;

use common::capture::*;
use common::*;
use kinige_domain::CaptureAction;
use kinige_domain::groups::INBOX_ID;

#[test]
fn context_is_up_to_three_latest_undeleted_entries_of_today_oldest_first() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());

    let empty = store.capture_view(&moscow(T0)).unwrap();
    assert_eq!(empty.today, "2026-10-21");
    assert!(empty.context.is_empty() && empty.append_target.is_none() && empty.draft.is_empty());

    store
        .send_message(INBOX_ID, "вчера", &moscow(T0 - 24 * 60 * MINUTE))
        .unwrap();
    assert!(
        store.capture_view(&moscow(T0)).unwrap().context.is_empty(),
        "вчерашняя запись не в контексте"
    );

    let ids: Vec<String> = (0..5)
        .map(|i| {
            store
                .send_message(INBOX_ID, &format!("запись {i}"), &moscow(T0 + i * MINUTE))
                .unwrap()
                .id
        })
        .collect();
    store
        .delete_message(&ids[4], &moscow(T0 + 6 * MINUTE))
        .unwrap();

    let view = store.capture_view(&moscow(T0 + 7 * MINUTE)).unwrap();
    let texts: Vec<&str> = view.context.iter().map(|m| m.body_md.as_str()).collect();
    assert_eq!(
        texts,
        ["запись 1", "запись 2", "запись 3"],
        "три последних неудалённых, самая новая последней"
    );
    assert_eq!(view.append_target.as_deref(), Some(ids[3].as_str()));
}

#[test]
fn capture_send_creates_a_user_entry_in_inbox_and_clears_the_capture_draft() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    store
        .set_capture_draft("недописанное", &moscow(T0))
        .unwrap();
    store
        .set_chat_draft(INBOX_ID, "черновик чата", &moscow(T0))
        .unwrap();

    let outcome = store
        .capture_send("Идея: **попробовать**", &moscow(T0 + MINUTE))
        .unwrap();
    assert_eq!(outcome.action, CaptureAction::Created);
    assert_eq!(outcome.message.chat_id, INBOX_ID);
    assert_eq!(outcome.message.body_md, "Идея: **попробовать**");
    assert_eq!(row(dir.path(), &outcome.message.id).origin, "user");
    assert_eq!(store.list_chats().unwrap()[0].count, 1);
    assert_eq!(store.get_capture_draft().unwrap(), "");
    assert_eq!(
        store.get_chat_draft(INBOX_ID).unwrap(),
        "черновик чата",
        "черновик главного окна не тронут"
    );
}
