//! Хук паники пишет в журнал место и поток, но не текст сообщения (в нём бывают данные пользователя). Отдельный бинарник:
//! логгер и хук процесса глобальные.

// Корень интеграционных тестов целиком — тестовый код: `cfg(test)` даёт хелперам вне #[test] те же послабления
// clippy.toml (unwrap/expect/panic/индексация), что и тестам в src/.
#![cfg(test)]

use log::{Level, Log, Metadata, Record};
use std::panic::catch_unwind;
use std::sync::Mutex;

const SECRET: &str = "синтетический секрет записи";

static RECORDS: Mutex<Vec<(Level, String)>> = Mutex::new(Vec::new());
static LOGGER: Capture = Capture;

struct Capture;

impl Log for Capture {
    fn enabled(&self, _: &Metadata<'_>) -> bool {
        true
    }

    fn log(&self, record: &Record<'_>) {
        RECORDS
            .lock()
            .unwrap()
            .push((record.level(), record.args().to_string()));
    }

    fn flush(&self) {}
}

#[test]
fn a_panic_reaches_the_log_with_its_place_and_thread_but_without_its_message() {
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(log::LevelFilter::Trace);
    kinige_lib::panic_hook::install();

    let caught = std::thread::Builder::new()
        .name("kinige-test-worker".into())
        .spawn(|| catch_unwind(|| panic!("{SECRET}")))
        .unwrap()
        .join()
        .unwrap();
    assert!(caught.is_err(), "паника поймана, поток жив");

    let records = RECORDS.lock().unwrap();
    assert_eq!(records.len(), 1, "{records:?}");
    let (level, text) = &records[0];
    assert_eq!(*level, Level::Error);
    assert!(text.contains("kinige-test-worker"), "{text}");
    assert!(text.contains("panic_hook.rs:"), "{text}");
    assert!(
        !text.contains(SECRET),
        "текст паники попал в журнал: {text}"
    );
    assert!(
        !text.contains("/Users/"),
        "в журнале абсолютный путь с домом: {text}"
    );
}
