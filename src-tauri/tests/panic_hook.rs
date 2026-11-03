//! The panic hook writes the location and the thread to the log, but not the message text (it can hold user data). A separate binary:
//! the process logger and hook are global.

// The whole root of the integration tests is test code: `cfg(test)` gives helpers outside #[test] the same relaxations
// of clippy.toml (unwrap/expect/panic/indexing) as the tests in src/.
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
    assert!(caught.is_err(), "the panic was caught, the thread is alive");

    let records = RECORDS.lock().unwrap();
    assert_eq!(records.len(), 1, "{records:?}");
    let (level, text) = &records[0];
    assert_eq!(*level, Level::Error);
    assert!(text.contains("kinige-test-worker"), "{text}");
    assert!(text.contains("panic_hook.rs:"), "{text}");
    assert!(
        !text.contains(SECRET),
        "the panic text got into the log: {text}"
    );
    assert!(
        !text.contains("/Users/"),
        "an absolute path with the home directory in the log: {text}"
    );
}
