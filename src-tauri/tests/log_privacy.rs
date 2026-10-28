//! Приватность журнала: ошибки команд пишутся в `log`, но ни адрес, ни id, ни текст записи в журнал не попадают.

mod common;

use common::*;
use log::{LevelFilter, Log, Metadata, Record};
use serde_json::json;
use std::sync::Mutex;

/// Уникальная метка, которая есть во всех «пользовательских» аргументах теста.
const MARK: &str = "ZEBRAUNIQ";

static CAPTURED: Mutex<Vec<String>> = Mutex::new(Vec::new());

struct Capture;

impl Log for Capture {
    fn enabled(&self, _: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        CAPTURED
            .lock()
            .unwrap()
            .push(format!("[{}] {}", record.level(), record.args()));
    }

    fn flush(&self) {}
}

#[test]
fn failed_commands_log_only_categories_never_user_input() {
    log::set_logger(&Capture).expect("в этом тестовом бинарнике логгер ставится один раз");
    log::set_max_level(LevelFilter::Trace);
    let fixture = fixture();
    let window = window(&fixture, "main");

    for url in [
        format!("javascript:{MARK}"),
        format!(" https://{MARK}.example/"),
        format!("{MARK}/relative"),
    ] {
        call(&window, "open_external_url", json!({ "url": url })).expect_err("ссылка отвергнута");
    }
    call(
        &window,
        "send_message",
        json!({"chatId": format!("{MARK}-chat"), "body": format!("{MARK} текст записи")}),
    )
    .expect_err("нет такого чата");
    call(
        &window,
        "edit_message",
        json!({"id": format!("{MARK}-id"), "body": format!("{MARK} правка")}),
    )
    .expect_err("нет такой записи");

    let lines = CAPTURED.lock().unwrap().clone();
    assert!(
        lines.iter().any(|line| line.contains("ссылка отвергнута")),
        "отказ по ссылке должен попасть в журнал: {lines:?}"
    );
    assert!(
        lines.iter().any(|line| line.contains("ошибка хранилища")),
        "ошибка хранилища должна попасть в журнал: {lines:?}"
    );
    for line in &lines {
        assert!(
            !line.to_uppercase().contains(MARK),
            "в журнале пользовательский ввод: {line}"
        );
    }
}
