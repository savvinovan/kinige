#![allow(dead_code)]

use kinige_store::{Now, Store, StoreOptions};
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
