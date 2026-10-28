#![allow(dead_code)]

pub mod capture;
pub mod import;
pub mod ops;

use kinige_store::{Now, Store, StoreOptions};
use serde_json::value::RawValue;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// 2026-10-21 09:12 по Москве (06:12 UTC).
pub const T0: i64 = 1_792_563_120_000;
/// 2026-10-21 00:30 по Москве (20 октября, 21:30 UTC): ещё вчерашний день записей (граница 04:00).
pub const T_NIGHT: i64 = 1_792_531_800_000;
pub const MINUTE: i64 = 60_000;

pub fn moscow(ms: i64) -> Now {
    Now::at(ms, "Europe/Moscow")
}

pub fn db_path(dir: &Path) -> PathBuf {
    dir.join("kinige.db")
}

pub fn mirror_dir(dir: &Path) -> PathBuf {
    dir.join("mirror")
}

pub fn options(dir: &Path) -> StoreOptions {
    let mut options = StoreOptions::new(db_path(dir), Some(mirror_dir(dir)));
    options.mirror_retry = Duration::from_millis(40);
    // Фоновое обслуживание (корзина, проверка целостности, копия дня) тесты запускают сами, с заданным временем.
    options.start_maintenance = false;
    options
}

pub fn open(dir: &Path) -> Store {
    Store::open_at(options(dir), &moscow(T0)).expect("хранилище открывается")
}

pub fn open_without_mirror(dir: &Path) -> Store {
    let mut options = options(dir);
    options.start_mirror = false;
    Store::open_at(options, &moscow(T0)).expect("хранилище открывается")
}

/// Ждёт, пока условие станет истинным (фоновая запись копии асинхронна).
pub fn wait_until(what: &str, mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if condition() {
            return;
        }
        std::thread::sleep(Duration::from_millis(15));
    }
    panic!("не дождались: {what}");
}

pub fn raw(dir: &Path) -> rusqlite::Connection {
    rusqlite::Connection::open(db_path(dir)).unwrap()
}

/// Чат, который называет себя Inbox (`is_inbox = 1`), но id у него другой.
pub fn insert_impostor_inbox(dir: &Path, id: &str) {
    raw(dir)
        .execute(
            "INSERT INTO chat (id, title, sort_key, is_inbox, created_at, updated_at, hlc_json, schema_version, groups_raw) \
             VALUES (?1, 'Самозванец', 'a1', 1, 0, 0, '{}', 1, '{}')",
            [id],
        )
        .unwrap();
}

/// Правка записи в обход хранилища: `f` получает разобранные `groups_raw` и `hlc_json` (значения дословно), результат
/// пишется обратно. Колонки правит вызывающий отдельно.
pub fn edit_raw_groups(
    dir: &Path,
    id: &str,
    f: impl FnOnce(&mut BTreeMap<String, Box<RawValue>>, &mut BTreeMap<String, Box<RawValue>>),
) {
    let conn = raw(dir);
    let (groups_raw, hlc_json): (String, String) = conn
        .query_row(
            "SELECT groups_raw, hlc_json FROM message WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    let mut groups: BTreeMap<String, Box<RawValue>> = serde_json::from_str(&groups_raw).unwrap();
    let mut hlcs: BTreeMap<String, Box<RawValue>> = serde_json::from_str(&hlc_json).unwrap();
    f(&mut groups, &mut hlcs);
    conn.execute(
        "UPDATE message SET groups_raw = ?1, hlc_json = ?2 WHERE id = ?3",
        (
            serde_json::to_string(&groups).unwrap(),
            serde_json::to_string(&hlcs).unwrap(),
            id,
        ),
    )
    .unwrap();
}
