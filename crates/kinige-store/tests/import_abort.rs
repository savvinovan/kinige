//! Импорт и сбои (слайс 4, задача 4.8): остановка между пачками, отказ записи пачки, сбой копии, `abort` между пачками и
//! посреди пачки. Полупачек нет, повторный импорт доносит остаток без дублей.

mod common;

use common::import::*;
use common::*;
use kinige_domain::ErrorCode;
use kinige_domain::Language;
use kinige_domain::groups::INBOX_ID;
use kinige_domain::import::ImportProgress;
use kinige_store::{StoreError, backups_dir};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Записей, строк `message_ext` и строк индекса поиска.
fn counts(dir: &std::path::Path) -> (i64, i64, i64) {
    let conn = raw(dir);
    let n = |sql: &str| -> i64 { conn.query_row(sql, [], |r| r.get(0)).unwrap() };
    (
        n("SELECT COUNT(*) FROM message"),
        n("SELECT COUNT(*) FROM message_ext"),
        n("SELECT COUNT(*) FROM search_map"),
    )
}

fn assert_consistent(dir: &std::path::Path, expected: i64) {
    let (messages, ext, indexed) = counts(dir);
    assert_eq!((messages, ext, indexed), (expected, expected, expected));
    let integrity: String = raw(dir)
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(integrity, "ok");
    let duplicates: i64 = raw(dir)
        .query_row(
            "SELECT COUNT(*) FROM (SELECT ext_json FROM message GROUP BY ext_json HAVING COUNT(*) > 1)",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(duplicates, 0, "дублей нет");
}

#[test]
fn stop_after_the_first_batch_then_the_rest_without_duplicates() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let export = parse(&saved_n(1_200));
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let report = store
        .import_telegram(
            export.clone(),
            all_chats(&export),
            Language::Ru,
            &now(),
            move |p: ImportProgress| {
                if p.done >= 500 {
                    flag.store(true, Ordering::SeqCst);
                }
            },
            stop,
        )
        .unwrap();
    assert!(report.stopped);
    assert_eq!(report.imported, 500);
    assert_consistent(dir.path(), 500);
    let rest = run(&store, &export, &[]).unwrap();
    assert!(!rest.stopped);
    assert_eq!((rest.imported, rest.skipped_existing), (700, 500));
    assert_consistent(dir.path(), 1_200);
}

#[test]
fn stop_before_anything_writes_nothing_and_takes_no_copy() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let export = parse(&saved_n(10));
    let report = store
        .import_telegram(
            export.clone(),
            all_chats(&export),
            Language::Ru,
            &now(),
            |_| {},
            Arc::new(AtomicBool::new(true)),
        )
        .unwrap();
    assert!(report.stopped);
    assert_eq!(report.imported, 0);
    assert_eq!(report.backup_name, None);
    assert_eq!(counts(dir.path()).0, 0);
}

#[test]
fn a_failing_batch_is_not_written_at_all_and_the_next_run_finishes() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let export = parse(&saved_n(1_200));
    // Отказ посреди второй пачки (сообщение 700): пачка откатывается целиком, с индексом, тегами и `message_ext`.
    raw(dir.path())
        .execute_batch(
            "CREATE TRIGGER fail_batch BEFORE INSERT ON message_ext WHEN NEW.ext_msg = '700' \
             BEGIN SELECT RAISE(ABORT, 'disk full'); END;",
        )
        .unwrap();
    let error = run(&store, &export, &[]).unwrap_err();
    assert_eq!(error.code(), ErrorCode::StoreWriteFailed);
    assert_consistent(dir.path(), 500);
    raw(dir.path())
        .execute_batch("DROP TRIGGER fail_batch;")
        .unwrap();
    let rest = run(&store, &export, &[]).unwrap();
    assert_eq!((rest.imported, rest.skipped_existing), (700, 500));
    assert_consistent(dir.path(), 1_200);
}

#[test]
fn a_failed_copy_cancels_the_import_before_any_write() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    store.send_message(INBOX_ID, "своя", &moscow(T0)).unwrap();
    // Вместо папки копий лежит файл: копию снять нельзя.
    std::fs::write(backups_dir(&db_path(dir.path())), b"not a folder").unwrap();
    let error = run(&store, &parse(&saved_n(20)), &[]).unwrap_err();
    assert!(matches!(error, StoreError::ImportBackupFailed));
    assert_eq!(error.code(), ErrorCode::ImportBackupFailed);
    assert_eq!(counts(dir.path()).0, 1, "ни одной импортированной записи");
    assert_eq!(counts(dir.path()).1, 0);
}

const CHILD_ENV: &str = "KINIGE_IMPORT_ABORT_DIR";
const MODE_ENV: &str = "KINIGE_IMPORT_ABORT_MODE";

/// Дочерний процесс: импорт 1200 сообщений; `between` — `abort` сразу после фиксации первой пачки, `inside` — `abort`
/// посреди второй (её запись застревает в триггере).
fn child(dir: &std::path::Path, mode: &str) -> ! {
    let store = open_without_mirror(dir);
    let export = parse(&saved_n(1_200));
    if mode == "inside" {
        raw(dir)
            .execute_batch(
                "CREATE TRIGGER stall_batch BEFORE INSERT ON message_ext WHEN NEW.ext_msg = '700' BEGIN \
                 SELECT count(*) FROM (WITH RECURSIVE c(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM c WHERE x < 4000000000) \
                 SELECT x FROM c); END;",
            )
            .unwrap();
        std::thread::spawn(|| {
            // Первая пачка фиксируется за доли секунды, вторая «считает» дальше: через секунду писатель посередине.
            std::thread::sleep(std::time::Duration::from_millis(1500));
            std::process::abort();
        });
    }
    let abort_after_first = mode == "between";
    let _ = store.import_telegram(
        export.clone(),
        all_chats(&export),
        Language::Ru,
        &now(),
        move |p| {
            if abort_after_first && p.done >= 500 {
                std::process::abort();
            }
        },
        Arc::new(AtomicBool::new(false)),
    );
    std::process::exit(0);
}

fn killed_import(mode: &str, test: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let status = Command::new(std::env::current_exe().unwrap())
        .args([test, "--exact", "--nocapture", "--test-threads=1"])
        .env(CHILD_ENV, dir.path())
        .env(MODE_ENV, mode)
        .status()
        .unwrap();
    assert!(!status.success(), "дочерний процесс должен был упасть");
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(status.signal(), Some(6), "SIGABRT");
    }
    dir
}

fn finish_after_kill(dir: &std::path::Path) {
    // Следующий запуск: база цела, пачка либо есть целиком, либо нет совсем.
    assert_consistent(dir, 500);
    let store = open_without_mirror(dir);
    let export = parse(&saved_n(1_200));
    let rest = run(&store, &export, &[]).unwrap();
    assert_eq!((rest.imported, rest.skipped_existing), (700, 500));
    assert_consistent(dir, 1_200);
    // Отметки Markdown-копии пережили: каждый импортированный день помечен или уже записан.
    let days: i64 = raw(dir)
        .query_row(
            "SELECT COUNT(DISTINCT journal_date) FROM message",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let marked: i64 = raw(dir)
        .query_row("SELECT COUNT(*) FROM mirror_dirty", [], |r| r.get(0))
        .unwrap();
    assert_eq!(marked, days);
}

#[test]
fn abort_between_batches_leaves_whole_batches_and_the_next_run_finishes() {
    let name = "abort_between_batches_leaves_whole_batches_and_the_next_run_finishes";
    if let Ok(dir) = std::env::var(CHILD_ENV) {
        child(dir.as_ref(), &std::env::var(MODE_ENV).unwrap());
    }
    let dir = killed_import("between", name);
    finish_after_kill(dir.path());
}

#[test]
fn abort_in_the_middle_of_a_batch_leaves_no_half_batch() {
    let name = "abort_in_the_middle_of_a_batch_leaves_no_half_batch";
    if let Ok(dir) = std::env::var(CHILD_ENV) {
        child(dir.as_ref(), &std::env::var(MODE_ENV).unwrap());
    }
    let dir = killed_import("inside", name);
    // Триггер-задержка остался в базе дочернего процесса: перед повторным импортом его убирают.
    raw(dir.path())
        .execute_batch("DROP TRIGGER IF EXISTS stall_batch;")
        .unwrap();
    finish_after_kill(dir.path());
}
