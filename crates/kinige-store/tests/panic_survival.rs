//! Паника вне MCP после снятия `panic = "abort"` (слайс 4a): чтение, проход копии и проход обслуживания не должны молча
//! ломаться навсегда. Паника — ошибка этого вызова или этого прохода, следующие работают.

mod common;

use common::*;
use kinige_domain::BackupError;
use kinige_domain::MirrorError;
use kinige_domain::groups::INBOX_ID;
use kinige_store::{BackgroundThread, Store, StoreError};

#[test]
fn a_panic_inside_a_read_fails_that_call_only_and_later_reads_work() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    store
        .send_message(INBOX_ID, "до паники", &moscow(T0))
        .unwrap();
    assert!(matches!(
        store.panic_in_read_for_test(),
        Err(StoreError::Internal(_))
    ));
    // Соединение чтения живо: чтения идут, запись видна.
    for _ in 0..3 {
        let page = store
            .list_messages(INBOX_ID, None, 10, &moscow(T0 + MINUTE))
            .unwrap();
        assert_eq!(page.messages.len(), 1);
        assert!(matches!(
            store.panic_in_read_for_test(),
            Err(StoreError::Internal(_))
        ));
    }
    assert!(store.get_settings().is_ok());
}

#[test]
fn a_panic_in_a_mirror_pass_is_caught_shown_in_settings_and_the_thread_keeps_going() {
    let dir = tempfile::tempdir().unwrap();
    // Повтор после паники — через 1,5 с: за это время ошибка в настройках успевает быть увиденной.
    let mut options = options(dir.path());
    options.mirror_retry = std::time::Duration::from_millis(1500);
    let store = Store::open_at(options, &moscow(T0)).unwrap();
    store.send_message(INBOX_ID, "первая", &moscow(T0)).unwrap();
    let file = mirror_dir(dir.path())
        .join("Inbox")
        .join("2026")
        .join("2026-10-21.md");
    wait_until("первая копия", || file.exists());
    store.inject_background_panic_for_test(BackgroundThread::Mirror);
    wait_until("ошибка копии в настройках", || {
        store.get_settings().unwrap().mirror.error == Some(MirrorError::WriteFailed)
    });
    // Поток жив: новая запись попадает в копию, ошибка снимается.
    store
        .send_message(INBOX_ID, "вторая после паники", &moscow(T0 + MINUTE))
        .unwrap();
    wait_until("копия после паники", || {
        std::fs::read_to_string(&file).is_ok_and(|text| text.contains("вторая после паники"))
    });
    wait_until("ошибка снята", || {
        store.get_settings().unwrap().mirror.error.is_none()
    });
}

#[test]
fn a_panic_in_a_maintenance_pass_is_caught_shown_in_settings_and_the_next_pass_runs() {
    let dir = tempfile::tempdir().unwrap();
    let mut options = options(dir.path());
    options.start_mirror = false;
    options.start_maintenance = true;
    let store = Store::open_at(options, &moscow(T0)).unwrap();
    // Первый проход при старте идёт на настоящих часах; ждём его конца (копия дня появилась).
    wait_until("первый проход обслуживания", || {
        store.get_settings().unwrap().backup.count >= 1
    });
    store.inject_background_panic_for_test(BackgroundThread::Maintenance);
    wait_until("ошибка копий в настройках", || {
        store.get_settings().unwrap().backup.error == Some(BackupError::WriteFailed)
    });
    // Поток жив: разбудим его ещё раз, следующий проход проходит и снимает ошибку.
    store.wake_maintenance_for_test();
    wait_until(
        "следующий проход снял ошибку",
        || store.get_settings().unwrap().backup.error.is_none(),
    );
}
