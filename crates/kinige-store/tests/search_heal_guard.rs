//! Предохранитель лечения индекса поиска (слайс 4, guardian, третья проверка): «перестроить» вместо «восстановить из копии»
//! допустимо только при порче одной таблицы `search_fts`. Смешанный вердикт (порча и индекса поиска, и обычной таблицы) и
//! порча страницы b-tree самой теневой таблицы (код 11, не 267) по-прежнему означают порчу базы: сигнал подаётся,
//! индекс не перестраивается.

mod common;

use common::ops::wait_closed;
use common::*;
use kinige_domain::groups::INBOX_ID;
use kinige_store::{CorruptionHook, Integrity, Store};
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

/// Структура индекса FTS5 заменена мусором: прямой запрос падает с кодом 267.
fn damage_fts_data(dir: &Path) {
    raw(dir)
        .execute(
            "UPDATE search_fts_data SET block = X'FFFFFFFFFFFF' WHERE id = 10",
            [],
        )
        .unwrap();
}

/// Все изменения в основном файле (WAL перенесён), чтобы порча страницы попала в то, что читает проверка.
fn checkpoint(dir: &Path) {
    let conn = raw(dir);
    conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))
        .unwrap();
}

/// Заголовок корневой страницы таблицы заменён мусором прямо в файле: обычное повреждение b-tree (код 11).
fn damage_root_page(dir: &Path, table: &str) {
    let conn = raw(dir);
    let root: i64 = conn
        .query_row(
            "SELECT rootpage FROM sqlite_master WHERE name = ?1",
            [table],
            |r| r.get(0),
        )
        .unwrap();
    let page_size: i64 = conn
        .pragma_query_value(None, "page_size", |r| r.get(0))
        .unwrap();
    drop(conn);
    let path = db_path(dir);
    let mut bytes = std::fs::read(&path).unwrap();
    let start = ((root - 1) * page_size) as usize;
    for byte in &mut bytes[start..start + 16] {
        *byte = 0xFF;
    }
    std::fs::write(&path, bytes).unwrap();
}

fn seeded(dir: &Path) {
    let store = open_without_mirror(dir);
    for i in 0..3 {
        store
            .send_message(
                INBOX_ID,
                &format!("запись про книгу {i}"),
                &moscow(T0 + i * MINUTE),
            )
            .unwrap();
    }
    drop(store);
    wait_closed(dir);
}

#[test]
fn a_mixed_verdict_is_corruption_not_a_rebuild() {
    let dir = tempfile::tempdir().unwrap();
    seeded(dir.path());
    damage_fts_data(dir.path());
    checkpoint(dir.path());
    damage_root_page(dir.path(), "message");
    assert_eq!(
        kinige_store::verify_database(&db_path(dir.path())),
        Integrity::Corrupt,
        "порча и индекса поиска, и таблицы записей"
    );
}

#[test]
fn maintenance_with_a_mixed_verdict_signals_and_never_rebuilds() {
    let dir = tempfile::tempdir().unwrap();
    seeded(dir.path());
    let (store, signals) = open_counting(dir.path());
    damage_fts_data(dir.path());
    checkpoint(dir.path());
    damage_root_page(dir.path(), "message");
    let report = store.run_maintenance(&moscow(T0)).unwrap();
    assert!(!report.integrity_ok, "{report:?}");
    assert_eq!(signals.load(Ordering::SeqCst), 1, "хук сработал");
    assert_eq!(store.search_heal_count(), 0, "индекс не перестраивался");
}

#[test]
fn a_damaged_btree_page_of_the_index_shadow_table_is_corruption() {
    let dir = tempfile::tempdir().unwrap();
    seeded(dir.path());
    checkpoint(dir.path());
    damage_root_page(dir.path(), "search_fts_data");
    assert_eq!(
        kinige_store::verify_database(&db_path(dir.path())),
        Integrity::Corrupt,
        "страница b-tree (код 11) — не порча данных индекса (код 267)"
    );
    // Та же порча под живым хранилищем: проход обслуживания сигналит и не лечит.
    let dir = tempfile::tempdir().unwrap();
    seeded(dir.path());
    let (store, signals) = open_counting(dir.path());
    checkpoint(dir.path());
    damage_root_page(dir.path(), "search_fts_data");
    let report = store.run_maintenance(&moscow(T0)).unwrap();
    assert!(!report.integrity_ok, "{report:?}");
    assert_eq!(signals.load(Ordering::SeqCst), 1);
    assert_eq!(store.search_heal_count(), 0);
}
