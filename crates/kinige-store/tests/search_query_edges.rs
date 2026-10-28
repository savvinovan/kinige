//! Запрос поиска (слайс 4, задача 4.9), края: пустой запрос и синтаксис FTS5, чаты по названию. Остальное — `search_query.rs`.

mod common;

use common::*;
use kinige_domain::SearchFilters;
use kinige_domain::groups::INBOX_ID;
use kinige_store::Store;

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
    // Название находится по словоформе и по префиксу; Inbox в Rust не сравнивается (его добавляет интерфейс).
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
