//! Самолечение индекса поиска (слайс 4, guardian): порча только таблицы `search_fts` (данные индекса — `SQLITE_CORRUPT_VTAB`
//! 267, настройки — `SQLITE_ERROR` «invalid fts5 file format») не превращается в экран «база повреждена» ни в поиске, ни в
//! записи, ни в обслуживании, ни при копии и восстановлении: индекс перестраивается из записей. Настоящая порча базы
//! по-прежнему подаёт сигнал (тесты `maintenance_safety.rs`).

mod common;

use common::ops::wait_closed;
use common::*;
use kinige_domain::SearchFilters;
use kinige_domain::groups::INBOX_ID;
use kinige_domain::search::SEARCH_INDEX_VERSION;
use kinige_store::{CorruptionHook, Integrity, Store};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};

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

/// База с тремя записями, закрытая (все данные в основном файле).
fn seeded(dir: &Path) {
    let store = open_without_mirror(dir);
    for (i, text) in ["читаю книгу", "купил книги", "пишу заметку"]
        .into_iter()
        .enumerate()
    {
        store
            .send_message(INBOX_ID, text, &moscow(T0 + i as i64 * MINUTE))
            .unwrap();
    }
    drop(store);
    wait_closed(dir);
}

/// Порча данных индекса FTS5: прямой запрос к нему падает (`SQLITE_CORRUPT_VTAB`), остальная база цела.
fn damage_data(dir: &Path) {
    let conn = raw(dir);
    let changed = conn
        .execute(
            // Запись структуры индекса (id = 10) заменена мусором одного и того же вида: порча воспроизводится без случайности.
            "UPDATE search_fts_data SET block = X'FFFFFFFFFFFF' WHERE id = 10",
            [],
        )
        .unwrap();
    assert!(changed > 0, "в индексе есть блоки для порчи");
    let broken = conn.query_row(
        "SELECT COUNT(*) FROM search_fts WHERE search_fts MATCH 'книг*'",
        [],
        |r| r.get::<_, i64>(0),
    );
    assert!(broken.is_err(), "индекс повреждён: {broken:?}");
}

/// Порча настроек индекса: `SQLITE_ERROR`, а не `CORRUPT`; такая таблица не открывается и не удаляется.
fn damage_config(dir: &Path) {
    let conn = raw(dir);
    conn.execute(
        "UPDATE search_fts_config SET v = 77 WHERE k = 'version'",
        [],
    )
    .unwrap();
    let broken = conn.query_row(
        "SELECT COUNT(*) FROM search_fts WHERE search_fts MATCH 'книг*'",
        [],
        |r| r.get::<_, i64>(0),
    );
    assert!(broken.is_err(), "настройки индекса повреждены: {broken:?}");
}

fn index_version(dir: &Path) -> String {
    raw(dir)
        .query_row(
            "SELECT value FROM app_setting WHERE key = 'search_index_version'",
            [],
            |r| r.get(0),
        )
        .unwrap()
}

/// Индекс согласован с записями: каждая запись в `search_map`, лишних строк нет, версия правил на месте.
fn assert_index_consistent(dir: &Path) {
    let conn = raw(dir);
    let (messages, mapped): (i64, i64) = conn
        .query_row(
            "SELECT (SELECT COUNT(*) FROM message), (SELECT COUNT(*) FROM search_map)",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(
        messages, mapped,
        "в индексе столько же записей, сколько в базе"
    );
    let integrity: String = conn
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(integrity, "ok");
    assert_eq!(index_version(dir), SEARCH_INDEX_VERSION.to_string());
}

#[test]
fn a_damaged_search_index_is_rebuilt_on_the_first_query_without_a_corruption_signal() {
    let dir = tempfile::tempdir().unwrap();
    seeded(dir.path());
    damage_data(dir.path());
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
    assert_eq!(store.search_heal_count(), 1);
    assert_index_consistent(dir.path());
}

#[test]
fn damaged_index_settings_are_healed_too_by_a_query_and_by_a_write() {
    // Запрос.
    let dir = tempfile::tempdir().unwrap();
    seeded(dir.path());
    damage_config(dir.path());
    let (store, signals) = open_counting(dir.path());
    assert_eq!(found(&store, "книга"), 2);
    assert_eq!(signals.load(Ordering::SeqCst), 0);
    assert_index_consistent(dir.path());
    drop(store);

    // Запись: сначала порча, потом отправка и правка — обе проходят, запись не теряется.
    let dir = tempfile::tempdir().unwrap();
    seeded(dir.path());
    damage_config(dir.path());
    let (store, signals) = open_counting(dir.path());
    let sent = store
        .send_message(INBOX_ID, "новая запись про книгу", &moscow(T0 + 9 * MINUTE))
        .expect("запись проходит при повреждённом индексе");
    assert_eq!(signals.load(Ordering::SeqCst), 0);
    assert_eq!(found(&store, "книга"), 3);
    store
        .edit_message(&sent.id, "теперь про море", &moscow(T0 + 10 * MINUTE))
        .unwrap();
    assert_eq!(found(&store, "море"), 1);
    assert_eq!(found(&store, "книга"), 2);
    assert_index_consistent(dir.path());
}

#[test]
fn a_write_with_a_damaged_index_is_saved_and_the_index_is_rebuilt_in_the_same_transaction() {
    let dir = tempfile::tempdir().unwrap();
    seeded(dir.path());
    damage_data(dir.path());
    let (store, signals) = open_counting(dir.path());
    let sent = store
        .send_message(
            INBOX_ID,
            "запись при порче индекса",
            &moscow(T0 + 9 * MINUTE),
        )
        .expect("отправка проходит");
    assert_eq!(
        signals.load(Ordering::SeqCst),
        0,
        "экран «база повреждена» не вызван"
    );
    assert_eq!(found(&store, "порче"), 1);
    assert_eq!(found(&store, "книги"), 2, "старые записи в индексе тоже");
    // Правка и удаление навсегда при порче: то же.
    damage_data(dir.path());
    store
        .edit_message(&sent.id, "правка текста", &moscow(T0 + 10 * MINUTE))
        .unwrap();
    assert_eq!(signals.load(Ordering::SeqCst), 0);
    assert_eq!(found(&store, "правка"), 1);
    assert_eq!(found(&store, "порче"), 0);
    assert_index_consistent(dir.path());
}

#[test]
fn maintenance_with_a_damaged_index_only_heals_it_and_signals_nothing() {
    let dir = tempfile::tempdir().unwrap();
    seeded(dir.path());
    damage_data(dir.path());
    let (store, signals) = open_counting(dir.path());
    let report = store.run_maintenance(&moscow(T0)).unwrap();
    assert!(report.integrity_ok, "база цела: {report:?}");
    assert!(report.backup_made, "копия дня снята");
    assert_eq!(signals.load(Ordering::SeqCst), 0);
    assert_eq!(store.search_heal_count(), 1);
    assert_eq!(found(&store, "книга"), 2);
    assert_index_consistent(dir.path());
    // Порча настроек: проход тоже не сигналит (проверка целостности сама возвращает ошибку).
    damage_config(dir.path());
    let later = moscow(T0 + 2 * 86_400_000);
    let report = store.run_maintenance(&later).unwrap();
    assert!(report.integrity_ok, "{report:?}");
    assert_eq!(signals.load(Ordering::SeqCst), 0);
    assert_index_consistent(dir.path());
}

#[test]
fn verdicts_name_a_damaged_index_apart_from_a_damaged_database() {
    let dir = tempfile::tempdir().unwrap();
    seeded(dir.path());
    let db = db_path(dir.path());
    assert_eq!(kinige_store::verify_database(&db), Integrity::Ok);
    damage_data(dir.path());
    assert_eq!(kinige_store::verify_database(&db), Integrity::SearchIndex);
    seeded_fresh_for_config(dir.path());
}

fn seeded_fresh_for_config(dir: &Path) {
    // Перестройка вернула здоровую базу; порча настроек — тоже только индекс.
    let (store, _) = open_counting(dir);
    assert_eq!(found(&store, "книга"), 2);
    drop(store);
    wait_closed(dir);
    damage_config(dir);
    assert_eq!(
        kinige_store::verify_database(&db_path(dir)),
        Integrity::SearchIndex
    );
}

#[test]
fn a_copy_taken_with_a_damaged_index_is_made_and_restoring_it_leaves_a_consistent_index() {
    let dir = tempfile::tempdir().unwrap();
    seeded(dir.path());
    damage_data(dir.path());
    let (store, signals) = open_counting(dir.path());
    // Копия перед импортом не отказывает из-за индекса.
    let name = store
        .backup_before_import(&moscow(T0 + 3 * MINUTE))
        .expect("копия снимается при повреждённом индексе");
    assert_eq!(signals.load(Ordering::SeqCst), 0);
    drop(store);
    wait_closed(dir.path());
    // Восстановление из такой копии: база открывается, индекс и версия согласованы.
    let db = db_path(dir.path());
    kinige_store::restore_backup(&db, &name, T0 + 4 * MINUTE, None)
        .expect("копия годится для восстановления");
    let (store, signals) = open_counting(dir.path());
    assert_eq!(found(&store, "книга"), 2);
    assert_eq!(signals.load(Ordering::SeqCst), 0);
    assert_index_consistent(dir.path());
}

#[test]
fn concurrent_queries_over_one_damaged_index_cause_one_rebuild() {
    let dir = tempfile::tempdir().unwrap();
    seeded(dir.path());
    damage_data(dir.path());
    let (store, signals) = open_counting(dir.path());
    let threads = 8;
    let gate = Arc::new(Barrier::new(threads));
    let handles: Vec<_> = (0..threads)
        .map(|_| {
            let (store, gate) = (store.clone(), gate.clone());
            std::thread::spawn(move || {
                gate.wait();
                found(&store, "книга")
            })
        })
        .collect();
    for handle in handles {
        assert_eq!(handle.join().unwrap(), 2);
    }
    assert_eq!(
        store.search_heal_count(),
        1,
        "одна перестройка на все запросы"
    );
    assert_eq!(signals.load(Ordering::SeqCst), 0);
}

#[test]
fn a_healthy_index_is_not_touched_by_queries() {
    let dir = tempfile::tempdir().unwrap();
    let (store, signals) = open_counting(dir.path());
    store.send_message(INBOX_ID, "книга", &moscow(T0)).unwrap();
    assert_eq!(found(&store, "книга"), 1);
    assert_eq!(signals.load(Ordering::SeqCst), 0);
    assert_eq!(store.search_heal_count(), 0);
}
