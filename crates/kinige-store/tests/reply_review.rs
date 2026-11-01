//! Правки по ревью слайса 5: адресат, перенесённый в другой чат; журнал обращений без цитат; адресат, пришедший позже;
//! скрытие карточки не попадает в `settings.json`; страницы, где у каждой записи есть ответ (смещения колонок `VIEW_LEN`).

// Корень интеграционных тестов целиком — тестовый код: `cfg(test)` даёт хелперам вне #[test] те же послабления
// clippy.toml (unwrap/expect/panic/индексация), что и тестам в src/.
#![cfg(test)]

mod common;

use common::days::at;
use common::settings_file::rig;
use common::*;
use kinige_domain::AgentMode;
use kinige_domain::groups::INBOX_ID;
use kinige_domain::{ReplyState, SearchFilters, TextSize};
use kinige_store::StoreError;
use serde_json::json;

fn today() -> kinige_store::Now {
    at(2026, 10, 31, 12, 0)
}

#[test]
fn a_draft_whose_target_moved_to_another_chat_says_moved_and_a_plain_send_still_works() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let old = store
        .send_message(INBOX_ID, "Старая мысль", &at(2023, 10, 31, 12, 5))
        .unwrap();
    let books = store.create_chat("Книги", None, &today()).unwrap();
    store
        .save_chat_draft(INBOX_ID, "ответ", Some(&old.id), &today())
        .unwrap();
    assert_eq!(
        store
            .chat_draft(INBOX_ID, &today())
            .unwrap()
            .reply_to
            .unwrap()
            .state,
        ReplyState::Live
    );

    store.move_message(&old.id, &books.id, &today()).unwrap();
    let draft = store.chat_draft(INBOX_ID, &today()).unwrap();
    assert_eq!(
        draft.reply_to.expect("строка ответа").state,
        ReplyState::Moved
    );
    assert_eq!(draft.body, "ответ");
    // Ответ Rust по-прежнему не принимает (адресат в другом чате), обычная запись уходит и очищает черновик.
    let refused = store.send_reply(INBOX_ID, "ответ", Some(&old.id), &today());
    assert!(
        matches!(refused, Err(StoreError::ReplyTargetNotFound)),
        "{refused:?}"
    );
    store.send_message(INBOX_ID, "ответ", &today()).unwrap();
    let after = store.chat_draft(INBOX_ID, &today()).unwrap();
    assert_eq!((after.body.as_str(), after.reply_to), ("", None));
    // В ленте самого ответа запись в другом чате остаётся живой: «moved» бывает только у строки над полем.
    let reply_view = store.list_messages(INBOX_ID, None, 10, &today()).unwrap();
    assert!(reply_view.messages.iter().all(|m| m.reply_to.is_none()));
}

#[test]
fn the_agent_log_shows_what_the_agent_saw_without_the_reply_quote() {
    let a = agent_env();
    let old = a
        .store
        .send_message(INBOX_ID, "СТАРЫЙ-ТЕКСТ", &at(2023, 3, 14, 9, 0))
        .unwrap();
    let reply = a
        .store
        .send_reply(INBOX_ID, "ответ", Some(&old.id), &at(2026, 3, 14, 9, 0))
        .unwrap();
    assert!(reply.reply_to.is_some());
    common::agent::call(
        &a.store,
        "get_entries",
        json!({"date": "2026-03-14"}),
        at(2026, 3, 14, 10, 0).ms,
    )
    .unwrap();
    let call = a
        .store
        .agent_log_page(None)
        .unwrap()
        .rows
        .into_iter()
        .find(|r| r.tool == "get_entries")
        .expect("обращение");
    let entries = a.store.agent_log_entries(call.id).unwrap();
    let shown = entries
        .iter()
        .filter_map(|e| e.message.as_ref())
        .find(|m| m.id == reply.id)
        .expect("запись в журнале");
    assert_eq!(
        shown.reply_to, None,
        "журнал не отдаёт цитату: агент её не видел"
    );
    assert_eq!(shown.body_md, "ответ");
}

fn agent_env() -> common::agent::Agent {
    common::agent::agent(AgentMode::Read, true)
}

#[test]
fn a_target_that_arrives_later_turns_the_quote_from_missing_to_live_without_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let old = store
        .send_message(INBOX_ID, "Придёт позже", &at(2023, 10, 31, 12, 5))
        .unwrap();
    let reply = store
        .send_reply(INBOX_ID, "ответ", Some(&old.id), &today())
        .unwrap();
    let state = |store: &kinige_store::Store| {
        store
            .list_messages(INBOX_ID, None, 10, &today())
            .unwrap()
            .messages
            .into_iter()
            .find(|m| m.id == reply.id)
            .and_then(|m| m.reply_to)
            .unwrap()
            .state
    };
    // Запись ещё не пришла при pull: её id пока занят другим (запись «исчезла» из базы).
    let conn = raw(dir.path());
    conn.execute("UPDATE message SET id = 'parked' WHERE id = ?1", [&old.id])
        .unwrap();
    assert_eq!(state(&store), ReplyState::Missing);
    // Пришла: строка с тем же id появилась в базе, цитата ожила без перезапуска.
    conn.execute("UPDATE message SET id = ?1 WHERE id = 'parked'", [&old.id])
        .unwrap();
    assert_eq!(state(&store), ReplyState::Live);
}

#[test]
fn dismissing_the_card_never_reaches_settings_json() {
    let rig = rig();
    rig.store.dismiss_on_this_day("2026-10-31").unwrap();
    // Любая правка настроек переписывает файл: ключа скрытия в нём нет ни до, ни после.
    rig.file
        .edit(|store| store.set_text_size(TextSize::Large))
        .unwrap()
        .unwrap();
    let text = String::from_utf8(rig.bytes()).unwrap();
    assert!(!text.contains("on_this_day"), "{text}");
    assert!(!text.contains("2026-10-31"), "{text}");
    assert_eq!(
        rig.setting("on_this_day_dismissed").as_deref(),
        Some("2026-10-31")
    );
}

#[test]
fn every_message_of_a_page_search_hits_and_trash_carry_their_own_quote() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let books = store.create_chat("Книги", Some("📚"), &today()).unwrap();
    let old = store
        .send_message(INBOX_ID, "Мысль про лес", &at(2023, 10, 31, 12, 5))
        .unwrap();
    let old_books = store
        .send_message(&books.id, "Мысль про книги", &at(2022, 10, 31, 12, 5))
        .unwrap();
    let mut ids = Vec::new();
    for n in 0..4 {
        ids.push(
            store
                .send_reply(
                    INBOX_ID,
                    &format!("ответ про лес {n}"),
                    Some(&old.id),
                    &at(2026, 10, 31, 9, n),
                )
                .unwrap()
                .id,
        );
    }
    let in_books = store
        .send_reply(
            &books.id,
            "ответ про книги лес",
            Some(&old_books.id),
            &at(2026, 10, 31, 10, 0),
        )
        .unwrap();

    let page = store
        .list_messages(INBOX_ID, None, 50, &today())
        .unwrap()
        .messages;
    for message in page.iter().filter(|m| ids.contains(&m.id)) {
        let quote = message.reply_to.as_ref().expect("цитата в ленте");
        assert_eq!(
            (quote.id.as_str(), quote.excerpt.as_str(), quote.years_later),
            (old.id.as_str(), "Мысль про лес", 3)
        );
    }
    let hits = store
        .search("лес", &SearchFilters::default(), None, 50)
        .unwrap()
        .hits;
    assert!(hits.len() >= 5);
    for hit in hits.iter().filter(|h| h.message.reply_to.is_some()) {
        let quote = hit.message.reply_to.as_ref().unwrap();
        let (target, title) = if hit.message.id == in_books.id {
            (&old_books.id, "Книги")
        } else {
            (&old.id, "")
        };
        assert_eq!(&quote.id, target);
        assert_eq!(
            hit.chat_title, title,
            "название чата не сдвинулось из-за новых колонок"
        );
        assert!(!hit.snippet.is_empty());
    }
    for id in ids.iter().chain([&in_books.id]) {
        store.delete_message(id, &today()).unwrap();
    }
    let trash = store.list_trash(&today()).unwrap();
    assert_eq!(trash.len(), 5);
    for item in &trash {
        assert!(item.message.reply_to.is_some());
        assert_eq!(
            item.chat_title,
            if item.message.id == in_books.id {
                "Книги"
            } else {
                ""
            }
        );
        assert!(item.days_left >= 29 && item.deleted_at > 0);
    }
}
