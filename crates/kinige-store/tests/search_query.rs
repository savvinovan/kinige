//! Запрос поиска (слайс 4, задача 4.9): словоформы и префикс, корзина и удалённый чат, фильтры по отдельности и вместе,
//! keyset без пропусков и повторов, пустой запрос, чаты по названию.

mod common;

use common::*;
use kinige_domain::groups::INBOX_ID;
use kinige_domain::history::DAY_MS;
use kinige_domain::{FeedCursor, MessageFlag, SearchFilters};
use kinige_store::{Store, StoreError};

fn texts(store: &Store, query: &str, filters: &SearchFilters) -> Vec<String> {
    store
        .search(query, filters, None, 100)
        .unwrap()
        .hits
        .into_iter()
        .map(|h| h.message.body_md)
        .collect()
}

fn any() -> SearchFilters {
    SearchFilters::default()
}

fn send(store: &Store, chat: &str, text: &str, minutes: i64) -> String {
    store
        .send_message(chat, text, &moscow(T0 + minutes * MINUTE))
        .unwrap()
        .id
}

#[test]
fn word_forms_and_prefix_are_found_newest_first() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    send(&store, INBOX_ID, "Купил книга", 1);
    send(&store, INBOX_ID, "Читаю книгу", 2);
    send(&store, INBOX_ID, "Разговор о книгой", 3);
    send(&store, INBOX_ID, "Книжка не книга? нет, книжка", 4);
    send(&store, INBOX_ID, "Купить молоко", 5);
    send(&store, INBOX_ID, "ничего", 6);
    assert_eq!(
        texts(&store, "книги", &any()),
        [
            "Книжка не книга? нет, книжка",
            "Разговор о книгой",
            "Читаю книгу",
            "Купил книга"
        ],
        "четыре формы слова, новые сверху"
    );
    assert_eq!(texts(&store, "моло", &any()), ["Купить молоко"]);
    assert_eq!(texts(&store, "МОЛОКА ", &any()), ["Купить молоко"]);
    assert_eq!(
        texts(&store, "читаю молоко ", &any()),
        Vec::<String>::new(),
        "термы соединены через «и»"
    );
    assert_eq!(texts(&store, "купить моло", &any()), ["Купить молоко"]);
}

#[test]
fn hit_has_snippet_terms_and_chat_info() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let chat = store
        .create_chat("Чтение", Some("📚"), &moscow(T0))
        .unwrap();
    send(&store, &chat.id, "**Читаю** книгу про море", 1);
    send(&store, INBOX_ID, "Книга дома", 2);
    let page = store.search("книги", &any(), None, 10).unwrap();
    let in_inbox = &page.hits[0];
    assert!(in_inbox.chat_is_inbox);
    assert_eq!(in_inbox.terms, ["Книга"]);
    assert_eq!(in_inbox.snippet, "Книга дома");
    let in_chat = &page.hits[1];
    assert!(!in_chat.chat_is_inbox);
    assert_eq!(in_chat.chat_title, "Чтение");
    assert_eq!(in_chat.chat_icon.as_deref(), Some("📚"));
    assert_eq!(in_chat.terms, ["книгу"]);
    assert_eq!(in_chat.snippet, "Читаю книгу про море", "разметка снята");
}

#[test]
fn trash_deleted_chats_and_old_revisions_are_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let kept = send(&store, INBOX_ID, "Жираф живой", 1);
    let trashed = send(&store, INBOX_ID, "Жираф в корзине", 2);
    let edited = send(&store, INBOX_ID, "Жираф старый текст", 3);
    let chat = store.create_chat("Зоопарк", None, &moscow(T0)).unwrap();
    send(&store, &chat.id, "Жираф в зоопарке", 4);
    store
        .delete_message(&trashed, &moscow(T0 + 10 * MINUTE))
        .unwrap();
    store
        .edit_message(&edited, "Жираф новый текст", &moscow(T0 + 11 * MINUTE))
        .unwrap();
    assert_eq!(
        texts(&store, "жирафы ", &any()),
        ["Жираф в зоопарке", "Жираф новый текст", "Жираф живой"]
    );
    assert!(
        texts(&store, "старый ", &any()).is_empty(),
        "ревизии не индексируются"
    );
    // Восстановленная из корзины запись находится снова.
    store
        .restore_message(&trashed, &moscow(T0 + 12 * MINUTE))
        .unwrap();
    assert!(texts(&store, "жирафы ", &any()).contains(&"Жираф в корзине".to_string()));
    // Удалённый чат: его записи в результаты не попадают.
    raw(dir.path())
        .execute("UPDATE chat SET deleted_at = 1 WHERE id = ?1", [&chat.id])
        .unwrap();
    assert!(!texts(&store, "жирафы ", &any()).contains(&"Жираф в зоопарке".to_string()));
    store.purge_message(&kept, &moscow(T0 + 13 * MINUTE)).ok();
    store
        .delete_message(&kept, &moscow(T0 + 14 * MINUTE))
        .unwrap();
    store
        .purge_message(&kept, &moscow(T0 + 15 * MINUTE))
        .unwrap();
    assert!(!texts(&store, "жирафы ", &any()).contains(&"Жираф живой".to_string()));
}

#[test]
fn each_filter_alone_and_all_together() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let chat = store.create_chat("Дела", None, &moscow(T0)).unwrap();
    // Пять записей с одним словом в разные дни (по записи в сутки).
    let day = |n: i64| T0 + n * DAY_MS;
    let a = store
        .send_message(INBOX_ID, "ключ раз", &moscow(day(0)))
        .unwrap()
        .id;
    let b = store
        .send_message(&chat.id, "ключ два", &moscow(day(1)))
        .unwrap()
        .id;
    let c = store
        .send_message(INBOX_ID, "ключ три", &moscow(day(2)))
        .unwrap()
        .id;
    let d = store
        .send_message(&chat.id, "ключ четыре", &moscow(day(3)))
        .unwrap()
        .id;
    let _e = store
        .send_message(INBOX_ID, "ключ пять", &moscow(day(4)))
        .unwrap()
        .id;
    store
        .set_message_flag(&b, MessageFlag::Favorite, true, &moscow(day(5)))
        .unwrap();
    store
        .set_message_flag(&c, MessageFlag::Favorite, true, &moscow(day(5)))
        .unwrap();
    store
        .set_message_flag(&c, MessageFlag::Task, true, &moscow(day(5)))
        .unwrap();
    store
        .set_message_flag(&d, MessageFlag::Task, true, &moscow(day(5)))
        .unwrap();
    let ids = |filters: SearchFilters| -> Vec<String> {
        store
            .search("ключ ", &filters, None, 100)
            .unwrap()
            .hits
            .into_iter()
            .map(|h| h.message.id)
            .collect()
    };
    assert_eq!(ids(any()).len(), 5);
    assert_eq!(
        ids(SearchFilters {
            chat_id: Some(chat.id.clone()),
            ..any()
        }),
        [d.clone(), b.clone()]
    );
    assert_eq!(
        ids(SearchFilters {
            favorite: true,
            ..any()
        }),
        [c.clone(), b.clone()]
    );
    assert_eq!(
        ids(SearchFilters {
            tasks: true,
            ..any()
        }),
        [d.clone(), c.clone()]
    );
    let from = journal_of(&store, &c);
    let to = journal_of(&store, &d);
    assert_eq!(
        ids(SearchFilters {
            from: Some(from.clone()),
            ..any()
        })
        .len(),
        3,
        "c, d, e"
    );
    assert_eq!(
        ids(SearchFilters {
            to: Some(to.clone()),
            ..any()
        })
        .len(),
        4,
        "a, b, c, d"
    );
    assert_eq!(
        ids(SearchFilters {
            from: Some(from.clone()),
            to: Some(to.clone()),
            ..any()
        }),
        [d.clone(), c.clone()]
    );
    // Всё вместе: чат + задача + даты.
    assert_eq!(
        ids(SearchFilters {
            chat_id: Some(chat.id.clone()),
            tasks: true,
            favorite: false,
            from: Some(from),
            to: Some(to),
        }),
        vec![d.clone()]
    );
    assert_eq!(
        ids(SearchFilters {
            favorite: true,
            tasks: true,
            ..any()
        }),
        [c]
    );
    assert!(
        ids(SearchFilters {
            chat_id: Some(chat.id),
            favorite: true,
            tasks: true,
            ..any()
        })
        .is_empty()
    );
    assert_eq!(ids(any()).last(), Some(&a), "самая старая запись последней");
}

fn journal_of(store: &Store, id: &str) -> String {
    store
        .search("ключ ", &any(), None, 100)
        .unwrap()
        .hits
        .into_iter()
        .find(|h| h.message.id == id)
        .unwrap()
        .message
        .journal_date
}

#[test]
fn invalid_dates_are_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    for bad in ["2026-02-30", "вчера", "2026-1-1", ""] {
        let error = store
            .search(
                "ключ",
                &SearchFilters {
                    from: Some(bad.to_string()),
                    ..any()
                },
                None,
                10,
            )
            .unwrap_err();
        assert!(matches!(error, StoreError::InvalidDate), "{bad:?}");
    }
}

#[test]
fn keyset_pages_have_no_gaps_and_no_repeats() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let mut all = Vec::new();
    for i in 0..30 {
        // Часть записей в одну минуту: порядок решает id.
        all.push(send(&store, INBOX_ID, &format!("страница {i}"), i / 3));
    }
    let mut seen: Vec<String> = Vec::new();
    let mut cursor: Option<FeedCursor> = None;
    let mut pages = 0;
    loop {
        let page = store
            .search("страницы ", &any(), cursor.as_ref(), 7)
            .unwrap();
        pages += 1;
        seen.extend(page.hits.iter().map(|h| h.message.id.clone()));
        match page.older {
            Some(next) => {
                assert_eq!(next.id, page.hits.last().unwrap().message.id);
                cursor = Some(next);
            }
            None => break,
        }
    }
    assert_eq!(pages, 5);
    assert_eq!(seen.len(), 30);
    let mut unique = seen.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), 30, "повторов нет");
    let mut expected = all.clone();
    expected.sort();
    assert_eq!(unique, expected, "пропусков нет");
    // Порядок — как в ленте, по убыванию.
    let feed: Vec<String> = store
        .list_messages(INBOX_ID, None, 100, &moscow(T0 + DAY_MS))
        .unwrap()
        .messages
        .into_iter()
        .rev()
        .map(|m| m.id)
        .collect();
    assert_eq!(seen, feed);
}

#[test]
fn empty_query_is_an_empty_page_without_error() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    send(&store, INBOX_ID, "что-то есть", 1);
    for query in ["", "   ", "***", "\" OR *", "-"] {
        let page = store.search(query, &any(), None, 10).unwrap();
        assert!(
            page.hits.is_empty() && page.chats.is_empty() && page.older.is_none(),
            "{query:?}"
        );
    }
    // Синтаксис FTS5 в запросе безвреден.
    let page = store
        .search("\" OR * NEAR( ^ - :col", &any(), None, 10)
        .unwrap();
    assert!(page.hits.is_empty());
}

#[test]
fn chats_whose_title_matches_come_only_on_the_first_page() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let books = store
        .create_chat("Книги и фильмы", Some("📚"), &moscow(T0))
        .unwrap();
    store.create_chat("Работа", None, &moscow(T0)).unwrap();
    for i in 0..5 {
        send(&store, INBOX_ID, &format!("книга {i}"), i);
    }
    let first = store.search("книга", &any(), None, 3).unwrap();
    assert_eq!(
        first.chats.iter().map(|c| c.id.clone()).collect::<Vec<_>>(),
        vec![books.id.clone()]
    );
    assert_eq!(first.hits.len(), 3);
    let second = store
        .search("книга", &any(), first.older.as_ref(), 3)
        .unwrap();
    assert!(
        second.chats.is_empty(),
        "названия — только на первой странице"
    );
    // Название находится по словоформе и по префиксу; Inbox в Rust не сравнивается.
    assert_eq!(
        store.search("фильм", &any(), None, 3).unwrap().chats.len(),
        1
    );
    assert_eq!(
        store
            .search("книг фильм", &any(), None, 3)
            .unwrap()
            .chats
            .len(),
        1
    );
    assert!(
        store
            .search("книги работа", &any(), None, 3)
            .unwrap()
            .chats
            .is_empty()
    );
    assert!(
        store
            .search("входящие", &any(), None, 3)
            .unwrap()
            .chats
            .is_empty()
    );
}
