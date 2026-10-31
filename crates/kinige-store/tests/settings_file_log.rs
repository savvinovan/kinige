//! Журнал процесса при битом файле настроек (ADR-0010): предупреждение один раз на смену состояния, а не на каждый опрос.
//! Отдельный бинарник: логгер один на процесс, другие тесты его строки не засоряют.

mod common;

use common::settings_file::*;
use kinige_domain::TextSize;
use log::{LevelFilter, Log, Metadata, Record};
use std::sync::Mutex;
use std::time::Duration;

static CAPTURED: Mutex<Vec<String>> = Mutex::new(Vec::new());
/// Тесты бинарника идут по очереди: у них общий логгер.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

struct Capture;

impl Log for Capture {
    fn enabled(&self, _: &Metadata) -> bool {
        true
    }
    fn log(&self, record: &Record) {
        CAPTURED.lock().unwrap().push(record.args().to_string());
    }
    fn flush(&self) {}
}

fn install() -> std::sync::MutexGuard<'static, ()> {
    static ONCE: std::sync::Once = std::sync::Once::new();
    let guard = ONE_AT_A_TIME
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    ONCE.call_once(|| {
        log::set_logger(&Capture).unwrap();
        log::set_max_level(LevelFilter::Trace);
    });
    CAPTURED.lock().unwrap().clear();
    guard
}

fn file_lines() -> Vec<String> {
    CAPTURED
        .lock()
        .unwrap()
        .iter()
        .filter(|line| line.contains("файл настроек"))
        .cloned()
        .collect()
}

#[test]
fn a_broken_file_after_a_window_edit_is_logged_once_not_on_every_poll() {
    let _guard = install();
    let rig = rig_with(|options| options.poll = Duration::from_millis(20));
    rig.write(r#"{"text_size": "#);
    // Правка из окна при битом файле: запись откладывается; опросы идут каждые 20 мс.
    rig.file
        .edit(|store| store.set_text_size(TextSize::Large))
        .unwrap()
        .unwrap();
    let polls_before = file_lines().len();
    std::thread::sleep(Duration::from_millis(500));
    let lines = file_lines();
    assert_eq!(
        lines.len(),
        polls_before,
        "опросы не пишут повторов: {lines:?}"
    );
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].contains("не разобран") && lines[0].contains("parse"));
    // Файл не перечитывался на каждом опросе: после починки он применяется, и состояние «битый» снова логируется заново.
    rig.write(r#"{"text_size": "normal"}"#);
    rig.write(r#"{"text_size": "#);
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(file_lines().len(), 2);
}

#[test]
fn an_unsafe_file_and_a_fifo_do_not_flood_the_log_either() {
    let _guard = install();
    let before = file_lines().len();
    let rig = rig_with(|options| options.poll = Duration::from_millis(20));
    let path = rig.path();
    std::fs::remove_file(&path).unwrap();
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&path)
            .status()
            .unwrap()
            .success()
    );
    rig.file.sync_now();
    rig.file
        .edit(|store| store.set_text_size(TextSize::Large))
        .unwrap()
        .unwrap();
    std::thread::sleep(Duration::from_millis(400));
    assert!(file_lines().len() - before <= 1, "{:?}", file_lines());
}
