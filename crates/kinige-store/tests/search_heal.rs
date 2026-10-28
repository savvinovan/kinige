//! Самолечение индекса поиска (слайс 4, guardian): повреждённая таблица `search_fts` не превращается в экран «база
//! повреждена», а перестраивается из записей; настоящее повреждение базы по-прежнему подаёт сигнал.

mod common;

use common::*;
use kinige_domain::SearchFilters;
use kinige_domain::groups::INBOX_ID;
use kinige_domain::search::SEARCH_INDEX_VERSION;
use kinige_store::{CorruptionHook, Store};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

fn open_counting(dir: &Path) -> (Store, Arc<AtomicUsize>) {
    let signals = Arc::new(AtomicUsize::new(0));
    let counter = signals.clone();
    let mut opts = options(dir);
    opts.start_mirror = false;
    opts.on_corruption = Some(CorruptionHook(Arc::new(move || {
        counter.fetch_add(1, Ordering::SeqCst);
    })));
    (
        Store::open_at(opts, &moscow(T0)).expect("хранилище открывается"),
        signals,
    )
}

fn found(store: &Store, query: &str) -> usize {
    store
        .search(query, &SearchFilters::default(), None, 50)
        .unwrap()
        .hits
        .len()
}

/// Портит блоки данных индекса FTS5 (теневая таблица `search_fts_data`), не трогая остальную базу.
fn damage_index(dir: &Path) {
    let conn = raw(dir);
    let changed = conn
        .execute(
            "UPDATE search_fts_data SET block = randomblob(length(block)) WHERE id > 1",
            [],
        )
        .unwrap();
    assert!(changed > 0, "в индексе есть блоки для порчи");
    // Порча настоящая: прямой запрос к индексу падает с повреждением, а не возвращает пустоту.
    let broken = conn.query_row(
        "SELECT COUNT(*) FROM search_fts WHERE search_fts MATCH 'книг*'",
        [],
        |r| r.get::<_, i64>(0),
    );
    assert!(broken.is_err(), "индекс повреждён: {broken:?}");
}

#[test]
fn a_damaged_search_index_is_rebuilt_on_the_first_query_without_a_corruption_signal() {
    let dir = tempfile::tempdir().unwrap();
    {
        let store = open_without_mirror(dir.path());
        for (i, text) in ["читаю книгу", "купил книги", "пишу заметку"]
            .into_iter()
            .enumerate()
        {
            store
                .send_message(INBOX_ID, text, &moscow(T0 + i as i64 * MINUTE))
                .unwrap();
        }
    }
    damage_index(dir.path());
    let (store, signals) = open_counting(dir.path());
    assert_eq!(
        found(&store, "книга"),
        2,
        "запрос вылечил индекс и нашёл записи"
    );
    assert_eq!(found(&store, "заметки"), 1);
    assert_eq!(
        signals.load(Ordering::SeqCst),
        0,
        "сигнала о повреждении базы нет"
    );
    let version: String = raw(dir.path())
        .query_row(
            "SELECT value FROM app_setting WHERE key = 'search_index_version'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(version, SEARCH_INDEX_VERSION.to_string());
}

#[test]
fn a_healthy_index_is_not_touched_by_queries() {
    let dir = tempfile::tempdir().unwrap();
    let (store, signals) = open_counting(dir.path());
    store.send_message(INBOX_ID, "книга", &moscow(T0)).unwrap();
    assert_eq!(found(&store, "книга"), 1);
    assert_eq!(signals.load(Ordering::SeqCst), 0);
}
