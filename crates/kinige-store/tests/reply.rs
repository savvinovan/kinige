//! Ответ на запись (слайс 5): проверка `reply_to_id`, байты `core`, цитата в ленте и её состояния, неизменяемость ссылки.

mod common;

use common::days::at;
use common::import::{OWNER, PETER, SAVED_T, all_chats, msg, parse, run as import, single};
use common::ops::groups;
use common::*;
use kinige_domain::groups::INBOX_ID;
use kinige_domain::{MessageView, ReplyState};
use kinige_store::{Store, StoreError};

const OLD_DAY: (i32, u32, u32) = (2023, 10, 31);

/// Старая запись 2023 года и ответ на неё сегодня (31 октября 2026).
fn old_and_reply(store: &Store) -> (MessageView, MessageView) {
    let old = store
        .send_message(
            INBOX_ID,
            "Так будет **лучше**\nвторая строка",
            &at(OLD_DAY.0, OLD_DAY.1, OLD_DAY.2, 12, 5),
        )
        .unwrap();
    let reply = store
        .send_reply(
            INBOX_ID,
            "До сих пор так думаю",
            Some(&old.id),
            &at(2026, 10, 31, 9, 0),
        )
        .unwrap();
    (old, reply)
}

fn today() -> kinige_store::Now {
    at(2026, 10, 31, 12, 0)
}

fn feed(store: &Store) -> Vec<MessageView> {
    store
        .list_messages(INBOX_ID, None, 100, &today())
        .unwrap()
        .messages
}

fn message_count(dir: &std::path::Path) -> i64 {
    raw(dir)
        .query_row("SELECT COUNT(*) FROM message", [], |r| r.get(0))
        .unwrap()
}

#[test]
fn a_reply_writes_reply_to_id_into_core_and_comes_back_with_the_quote() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let (old, reply) = old_and_reply(&store);

    let core = &groups(dir.path(), "message", &reply.id)["core"];
    assert_eq!(
        *core,
        format!(
            r#"{{"created_at":{},"created_tz":"Europe/Moscow","origin":"user","reply_to_id":"{}"}}"#,
            reply.created_at, old.id
        )
    );
    let quote = reply
        .reply_to
        .as_ref()
        .expect("цитата в ответе на отправку");
    assert_eq!(quote.id, old.id);
    assert_eq!(quote.state, ReplyState::Live);
    assert_eq!(quote.years_later, 3);
    assert_eq!(quote.journal_date.as_deref(), Some("2023-10-31"));
    assert_eq!(quote.excerpt, "Так будет лучше вторая строка");
    assert_eq!(quote.chat_id.as_deref(), Some(INBOX_ID));

    let in_feed = feed(&store);
    assert_eq!(in_feed.len(), 2);
    assert_eq!(in_feed[0].reply_to, None, "у старой записи цитаты нет");
    assert_eq!(in_feed[1].reply_to.as_ref(), Some(quote));
}

#[test]
fn garbage_reply_targets_are_refused_and_nothing_is_written() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let (old, _) = old_and_reply(&store);
    let books = store.create_chat("Книги", None, &today()).unwrap();
    let other_chat = store
        .send_message(&books.id, "в другом чате", &today())
        .unwrap();
    let trashed = store
        .send_message(INBOX_ID, "уйдёт в корзину", &today())
        .unwrap();
    store.delete_message(&trashed.id, &today()).unwrap();
    store
        .set_chat_draft(INBOX_ID, "черновик остаётся", &today())
        .unwrap();
    let before = message_count(dir.path());

    let huge = "a".repeat(1 << 20);
    let upper = old.id.to_uppercase();
    let no_hyphens = old.id.replace('-', "");
    let missing = "5b1c0d9e-3f2a-4a7b-9c8d-1e2f3a4b5c6d".to_string();
    for bad in [
        "",
        "не uuid",
        huge.as_str(),
        upper.as_str(),
        no_hyphens.as_str(),
        "5b1c0d9e-3f2a-1a7b-9c8d-1e2f3a4b5c6d",
        missing.as_str(),
        other_chat.id.as_str(),
        trashed.id.as_str(),
        "'; DROP TABLE message; --",
    ] {
        let result = store.send_reply(INBOX_ID, "ответ", Some(bad), &today());
        assert!(
            matches!(result, Err(StoreError::ReplyTargetNotFound)),
            "{}: {result:?}",
            &bad[..bad.len().min(30)]
        );
    }
    assert_eq!(message_count(dir.path()), before, "ничего не записано");
    assert_eq!(
        store.get_chat_draft(INBOX_ID).unwrap(),
        "черновик остаётся",
        "черновик на месте"
    );
}

#[test]
fn an_empty_text_is_still_an_empty_message_error_even_with_a_valid_target() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let (old, _) = old_and_reply(&store);
    let result = store.send_reply(INBOX_ID, "  \n ", Some(&old.id), &today());
    assert!(matches!(result, Err(StoreError::Empty)), "{result:?}");
}

#[test]
fn the_link_survives_edit_move_date_change_and_append() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path());
    let (old, reply) = old_and_reply(&store);
    let books = store.create_chat("Книги", None, &today()).unwrap();
    let core_before = groups(dir.path(), "message", &reply.id)["core"].clone();
    let link = |store: &Store| {
        let page = store.list_messages(INBOX_ID, None, 100, &today()).unwrap();
        page.messages
            .into_iter()
            .find(|m| m.id == reply.id)
            .and_then(|m| m.reply_to)
    };

    store
        .edit_message(&reply.id, "Правка ответа", &today())
        .unwrap();
    assert_eq!(link(&store).map(|q| q.id), Some(old.id.clone()));
    store
        .set_message_date(&reply.id, "2026-10-30", &today())
        .unwrap();
    assert_eq!(link(&store).map(|q| q.id), Some(old.id.clone()));
    assert_eq!(
        groups(dir.path(), "message", &reply.id)["core"],
        core_before,
        "core не переписывается"
    );
    // Дописывание ⌘Enter: ответ создан не сегодняшним числом по журналу, поэтому проверяем сам путь, а не цель.
    let appended = store
        .send_reply(
            INBOX_ID,
            "ещё ответ",
            Some(&old.id),
            &at(2026, 10, 31, 9, 30),
        )
        .unwrap();
    let outcome = store
        .capture_append("дописано", &at(2026, 10, 31, 9, 35))
        .unwrap();
    assert_eq!(outcome.message.id, appended.id);
    assert_eq!(
        outcome.message.reply_to.as_ref().map(|q| q.id.as_str()),
        Some(old.id.as_str())
    );
    // Перенос ответа в другой чат: ссылка остаётся, старая запись теперь «в другом чате», но жива.
    store.move_message(&reply.id, &books.id, &today()).unwrap();
    let moved = store.list_messages(&books.id, None, 10, &today()).unwrap();
    let quote = moved.messages[0]
        .reply_to
        .as_ref()
        .expect("ссылка после переноса");
    assert_eq!(
        (quote.id.as_str(), quote.state),
        (old.id.as_str(), ReplyState::Live)
    );
    assert_eq!(
        groups(dir.path(), "message", &reply.id)["core"],
        core_before
    );
}

#[test]
fn the_quote_follows_the_old_entry_trash_restore_and_purge() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path());
    let (old, reply) = old_and_reply(&store);
    let state = |store: &Store| {
        let page = store.list_messages(INBOX_ID, None, 100, &today()).unwrap();
        page.messages
            .into_iter()
            .find(|m| m.id == reply.id)
            .and_then(|m| m.reply_to)
            .expect("ответ остаётся в ленте")
    };

    store.delete_message(&old.id, &today()).unwrap();
    let trash = state(&store);
    assert_eq!(trash.state, ReplyState::Trash);
    assert_eq!(trash.journal_date.as_deref(), Some("2023-10-31"));
    store.restore_message(&old.id, &today()).unwrap();
    assert_eq!(state(&store).state, ReplyState::Live);
    store.delete_message(&old.id, &today()).unwrap();
    store.purge_message(&old.id, &today()).unwrap();
    let gone = state(&store);
    assert_eq!(gone.state, ReplyState::Missing);
    assert_eq!(
        (
            gone.excerpt.as_str(),
            gone.chat_id,
            gone.journal_date,
            gone.years_later
        ),
        ("", None, None, 0)
    );
    assert!(
        feed(&store).iter().any(|m| m.id == reply.id),
        "сам ответ остаётся"
    );
}

#[test]
fn a_reply_dated_earlier_than_the_original_has_zero_years_later() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let (_, reply) = old_and_reply(&store);
    let moved = store
        .set_message_date(&reply.id, "2020-01-01", &today())
        .unwrap();
    assert_eq!(moved.reply_to.unwrap().years_later, 0);
}

#[test]
fn reply_to_id_from_a_foreign_device_that_is_not_a_uuid_shows_a_missing_quote() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let (_, reply) = old_and_reply(&store);
    raw(dir.path())
        .execute(
            "UPDATE message SET reply_to_id = 'мусор из будущей версии' WHERE id = ?1",
            [&reply.id],
        )
        .unwrap();
    let shown = feed(&store).into_iter().find(|m| m.id == reply.id).unwrap();
    assert_eq!(shown.reply_to.unwrap().state, ReplyState::Missing);
}

#[test]
fn imported_telegram_replies_have_a_live_quote_without_a_years_label() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let theirs = format!("user{PETER}");
    let mine = format!("user{OWNER}");
    let reply_to = |id: u64, target: u64, text: &str| {
        format!(
            r#"{{"id":{id},"type":"message","date_unixtime":"{}","from":"Я","from_id":"{mine}","reply_to_message_id":{target},"text":"{text}","text_entities":[{{"type":"plain","text":"{text}"}}]}}"#,
            SAVED_T + id as i64
        )
    };
    let messages = vec![
        msg(1, SAVED_T + 1, "поехали в субботу?", &theirs),
        reply_to(2, 1, "да, давай"),
        reply_to(3, 999, "ответ на то, чего нет в файле"),
    ];
    let export = parse(&single(
        "personal_chat",
        PETER,
        Some("Пётр Тестов"),
        &messages,
    ));
    import(
        &store,
        &export,
        &all_chats(&export)
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
    )
    .unwrap();
    let chat = store
        .list_chats()
        .unwrap()
        .into_iter()
        .find(|c| !c.is_inbox)
        .unwrap();
    let page = store
        .list_messages(&chat.id, None, 50, &today())
        .unwrap()
        .messages;
    assert_eq!(page.len(), 3);
    assert_eq!(page[0].reply_to, None);
    let quote = page[1]
        .reply_to
        .as_ref()
        .expect("цитата у импортированного ответа");
    assert_eq!(quote.state, ReplyState::Live);
    assert_eq!(quote.years_later, 0);
    assert_eq!(quote.id, page[0].id);
    assert_eq!(
        quote.excerpt, "Имя user2000002 поехали в субботу?",
        "имя автора и начало реплики"
    );
    assert_eq!(
        page[2].reply_to, None,
        "ответ на непереданную реплику цитаты не имеет"
    );
}
