//! Обслуживание и целостность (повторное ревью guardian, продолжение `restore_safety.rs`): проверка раз в сутки, после
//! находки в базу ничего не пишется, `verify_database` только читает, автоудаление не блокируется пропущенными записями,
//! восстановление из копии схемы 2 мигрирует базу.

mod common;

use common::ops::*;
use common::*;
use kinige_domain::backup::pre_migration_name;
use kinige_domain::groups::INBOX_ID;
use kinige_domain::history::DAY_MS;
use kinige_store::{CorruptionHook, Integrity, Now, Store, backups, backups_dir, verify_database};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

fn at(ms: i64) -> Now {
    moscow(ms)
}

/// Портит страницы в хвосте файла: проверка целостности это видит, открытие базы проходит.
fn damage_pages(path: &Path) {
    let mut bytes = fs::read(path).unwrap();
    let len = bytes.len();
    assert!(len > 40 * 1024, "достаточно страниц: {len}");
    for byte in &mut bytes[len - 12 * 1024..len - 4 * 1024] {
        *byte = 0xFF;
    }
    fs::write(path, bytes).unwrap();
}

/// База больше 256 страниц: у неё есть «хвост», который можно испортить.
fn big_database(dir: &Path, options: kinige_store::StoreOptions) -> Store {
    let store = Store::open_at(options, &at(T0)).unwrap();
    for i in 0..300 {
        store
            .send_message(INBOX_ID, &"текст ".repeat(100 + i), &at(T0 + i as i64))
            .unwrap();
    }
    let _ = dir;
    store
}

// ---------- проверка целостности и обслуживание ----------

#[test]
fn the_second_run_of_a_day_does_not_check_integrity_again() {
    let dir = tempfile::tempdir().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let mut opts = options(dir.path());
    let counter = calls.clone();
    opts.on_corruption = Some(CorruptionHook(Arc::new(move || {
        counter.fetch_add(1, Ordering::SeqCst);
    })));
    let store = big_database(dir.path(), opts);
    assert!(store.run_maintenance(&at(T0)).unwrap().integrity_ok);
    wait_closed_keep_open(dir.path());
    damage_pages(&db_path(dir.path()));

    // Тот же день: проверка не повторяется (иначе нашла бы порчу), сигнала нет.
    let report = store.run_maintenance(&at(T0 + MINUTE)).unwrap();
    assert!(
        report.integrity_ok,
        "проверка раз в сутки: сегодня уже была"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    // Следующие сутки: проверка идёт и находит.
    let next = store.run_maintenance(&at(T0 + DAY_MS)).unwrap();
    assert!(!next.integrity_ok);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

/// Контрольная точка WAL без закрытия хранилища: страницы базы лежат в основном файле.
fn wait_closed_keep_open(dir: &Path) {
    let conn = raw(dir);
    let _ = conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()));
}

#[test]
fn after_corruption_is_found_nothing_is_written_not_even_the_trash_cleanup() {
    let dir = tempfile::tempdir().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let mut opts = options(dir.path());
    opts.start_mirror = false;
    let counter = calls.clone();
    opts.on_corruption = Some(CorruptionHook(Arc::new(move || {
        counter.fetch_add(1, Ordering::SeqCst);
    })));
    let store = big_database(dir.path(), opts);
    let sent = store.send_message(INBOX_ID, "в корзину", &at(T0)).unwrap();
    store.delete_message(&sent.id, &at(T0 + 1)).unwrap();
    wait_closed_keep_open(dir.path());
    damage_pages(&db_path(dir.path()));
    let later = T0 + 40 * DAY_MS;

    let first = store.run_maintenance(&at(later)).unwrap();
    assert!(!first.integrity_ok);
    assert_eq!(
        first.purged, 0,
        "автоудаление идёт только после вердикта «цела»"
    );
    let second = store.run_maintenance(&at(later + MINUTE)).unwrap();
    assert!(!second.integrity_ok && second.purged == 0 && !second.backup_made);

    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(count_rows(dir.path(), "tombstone"), 0);
    let in_trash: i64 = raw(dir.path())
        .query_row(
            "SELECT COUNT(*) FROM message WHERE deleted_at IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(in_trash, 1, "запись осталась в корзине");
}

#[test]
fn verifying_a_database_only_reads_it() {
    let dir = tempfile::tempdir().unwrap();
    drop(open_without_mirror(dir.path()));
    wait_closed(dir.path());
    let db = db_path(dir.path());
    let bytes = fs::read(&db).unwrap();
    let names = || {
        let mut n: Vec<String> = fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .map(|e| e.file_name().into_string().unwrap())
            .collect();
        n.sort();
        n
    };
    let before = names();

    assert_eq!(verify_database(&db), Integrity::Ok);

    assert_eq!(fs::read(&db).unwrap(), bytes, "файл не изменён");
    // Читателю базы в режиме WAL SQLite сама заводит `-shm` и `-wal`; больше ничего появляться не должно.
    let created: Vec<String> = names()
        .into_iter()
        .filter(|n| !before.contains(n) && !n.ends_with("-shm") && !n.ends_with("-wal"))
        .collect();
    assert!(created.is_empty(), "{created:?}");
    // Нет файла: проверка не создаёт пустую базу.
    let missing = dir.path().join("none.db");
    assert_eq!(verify_database(&missing), Integrity::Corrupt);
    assert!(!missing.exists());
    // Не база данных.
    let junk: PathBuf = dir.path().join("junk.db");
    fs::write(&junk, vec![0xAB; 8192]).unwrap();
    assert_eq!(verify_database(&junk), Integrity::Corrupt);
}

// ---------- автоудаление: пропущенные записи не мешают здоровым ----------

#[test]
fn more_than_a_page_of_skipped_entries_does_not_block_the_healthy_ones_behind_them() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let mut skipped = Vec::new();
    for i in 0..230 {
        let sent = store
            .send_message(INBOX_ID, &format!("новее {i}"), &at(T0 + i))
            .unwrap();
        store.delete_message(&sent.id, &at(T0 + 1000 + i)).unwrap();
        skipped.push(sent.id);
    }
    let mut healthy = Vec::new();
    for i in 0..5 {
        let sent = store
            .send_message(INBOX_ID, &format!("здоровая {i}"), &at(T0 + 500 + i))
            .unwrap();
        // Удалены позже всех пропускаемых: в выборке стоят за ними.
        store.delete_message(&sent.id, &at(T0 + 5000 + i)).unwrap();
        healthy.push(sent.id);
    }
    drop(store);
    wait_closed(dir.path());
    // У первых 230 группа `deleted` из будущей версии: автоудаление их пропускает.
    let conn = raw(dir.path());
    for id in &skipped {
        let hlc: String = conn
            .query_row("SELECT hlc_json FROM message WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .unwrap();
        let mut value: serde_json::Value = serde_json::from_str(&hlc).unwrap();
        value["deleted"]["sv"] = serde_json::json!(2);
        conn.execute(
            "UPDATE message SET hlc_json = ?1 WHERE id = ?2",
            (value.to_string(), id),
        )
        .unwrap();
    }
    drop(conn);

    let store = open_without_mirror(dir.path());
    let purged = store.purge_expired(&at(T0 + 60 * DAY_MS)).unwrap();

    assert_eq!(purged, 5, "здоровые записи за 230 пропущенными удалены");
    assert_eq!(count_rows(dir.path(), "tombstone"), 5);
    assert_eq!(count_rows(dir.path(), "message"), 230);
    let _ = healthy;
}

// ---------- восстановление из копии схемы 2 ----------

#[test]
fn restoring_a_pre_migration_copy_of_schema_two_migrates_the_database_on_open() {
    let dir = tempfile::tempdir().unwrap();
    {
        let store = open_without_mirror(dir.path());
        store
            .send_message(INBOX_ID, "до миграции #старая", &at(T0))
            .unwrap();
    }
    wait_closed(dir.path());
    // База слайса 2.
    let conn = raw(dir.path());
    conn.execute_batch(
        "DROP INDEX message_deleted; DROP TABLE message_revision; DROP TABLE tombstone; DROP TABLE message_tag; \
         DROP TABLE mirror_chat; DROP TABLE message_ext; DROP TABLE search_fts; DROP TABLE search_map; \
         DELETE FROM app_setting WHERE key = 'search_index_version';",
    )
    .unwrap();
    conn.pragma_update(None, "user_version", 2).unwrap();
    drop(conn);
    // Миграция снимает копию «перед обновлением», после чего с базой «случается беда».
    drop(Store::open_at(options(dir.path()), &at(T0 + 5 * MINUTE)).unwrap());
    wait_closed(dir.path());
    let copy = pre_migration_name(2, &kinige_store::utc_stamp(T0 + 5 * MINUTE));
    assert!(backups_dir(&db_path(dir.path())).join(&copy).is_file());
    fs::write(db_path(dir.path()), b"broken").unwrap();

    kinige_store::restore_backup(&db_path(dir.path()), &copy, T0 + 10 * MINUTE, None).unwrap();
    // Что делает `resume_after_restore`: открывает восстановленную базу (схема 2) — она мигрирует до последней.
    let store = Store::open_at(options(dir.path()), &at(T0 + 10 * MINUTE)).unwrap();

    let version: i64 = raw(dir.path())
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, kinige_store::LATEST_VERSION);
    assert_eq!(count_rows(dir.path(), "message"), 1);
    assert_eq!(count_rows(dir.path(), "search_map"), 1, "индекс построен");
    assert_eq!(count_rows(dir.path(), "message_tag"), 1, "теги пересчитаны");
    assert_eq!(store.list_chats().unwrap()[0].count, 1);
    assert!(
        backups(&db_path(dir.path()))
            .iter()
            .filter(|b| b.name.starts_with("pre-migration-v2-"))
            .count()
            >= 1
    );
}

// ---------- повреждение, найденное командой, и копия с порчей, которой не видит `quick_check` ----------

fn counting_options(dir: &Path, calls: &Arc<AtomicUsize>) -> kinige_store::StoreOptions {
    let mut opts = options(dir);
    opts.start_mirror = false;
    let counter = calls.clone();
    opts.on_corruption = Some(CorruptionHook(Arc::new(move || {
        counter.fetch_add(1, Ordering::SeqCst);
    })));
    opts
}

#[test]
fn a_read_that_hits_a_damaged_page_raises_the_hook_once_and_stops_the_maintenance_writes() {
    let dir = tempfile::tempdir().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let store = Store::open_at(counting_options(dir.path(), &calls), &at(T0)).unwrap();
    let sent = store.send_message(INBOX_ID, "в корзину", &at(T0)).unwrap();
    store.delete_message(&sent.id, &at(T0 + 1)).unwrap();
    wait_closed_keep_open(dir.path());
    let mut bytes = fs::read(db_path(dir.path())).unwrap();
    for byte in &mut bytes[4096..] {
        *byte = 0xFF;
    }
    fs::write(db_path(dir.path()), bytes).unwrap();

    let error = store.get_settings().unwrap_err();
    assert!(
        matches!(error, kinige_store::StoreError::Corrupt(_)),
        "{error}"
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "тот же сигнал, что у фоновой проверки"
    );
    assert!(store.get_settings().is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 1, "один раз за запуск");
    // После находки обслуживание в базу ничего не пишет (автоудаление корзины, копия дня).
    let report = store.run_maintenance(&at(T0 + 40 * DAY_MS)).unwrap();
    assert!(!report.integrity_ok && report.purged == 0 && !report.backup_made);
}

#[test]
fn a_restored_copy_with_a_damaged_index_passes_quick_check_and_the_first_pass_raises_the_hook() {
    let dir = tempfile::tempdir().unwrap();
    {
        let store = open_without_mirror(dir.path());
        for i in 0..5 {
            store
                .send_message(INBOX_ID, &format!("запись {i}"), &at(T0 + i))
                .unwrap();
        }
        store.run_maintenance(&at(T0)).unwrap();
    }
    wait_closed(dir.path());
    let name = backups(&db_path(dir.path()))[0].name.clone();
    // Порча индекса `message_day` прямо в файле копии: ключ индекса расходится с таблицей. `quick_check` индексы
    // с таблицей не сверяет и проходит, `integrity_check` — нет.
    let copy = backups_dir(&db_path(dir.path())).join(&name);
    let root: i64 = rusqlite::Connection::open(&copy)
        .unwrap()
        .query_row(
            "SELECT rootpage FROM sqlite_master WHERE name = 'message_day'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let mut bytes = fs::read(&copy).unwrap();
    let page = (root as usize - 1) * 4096;
    let at_key = bytes[page..page + 4096]
        .windows(8)
        .position(|w| w == b"2026-10-")
        .expect("ключ дня в странице индекса");
    bytes[page + at_key + 9] = b'9';
    fs::write(&copy, bytes).unwrap();
    let verdict: String = rusqlite::Connection::open(&copy)
        .unwrap()
        .query_row("PRAGMA quick_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(verdict, "ok", "копия проходит быструю проверку");
    fs::write(db_path(dir.path()), b"broken").unwrap();

    kinige_store::restore_backup(&db_path(dir.path()), &name, T0 + MINUTE, None).unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let store = Store::open_at(counting_options(dir.path(), &calls), &at(T0 + MINUTE)).unwrap();
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "при открытии порчи не видно"
    );

    let report = store.run_maintenance(&at(T0 + MINUTE)).unwrap();

    assert!(
        !report.integrity_ok,
        "первый проход обслуживания находит порчу"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn a_command_that_finds_corruption_also_shows_it_in_the_settings_row() {
    let dir = tempfile::tempdir().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let store = Store::open_at(counting_options(dir.path(), &calls), &at(T0)).unwrap();
    for i in 0..5 {
        store
            .send_message(INBOX_ID, &format!("запись {i}"), &at(T0 + i))
            .unwrap();
    }
    wait_closed_keep_open(dir.path());
    // Портится только корневая страница таблицы `message`: настройки читаются, записи — нет.
    let root: i64 = raw(dir.path())
        .query_row(
            "SELECT rootpage FROM sqlite_master WHERE name = 'message'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let mut bytes = fs::read(db_path(dir.path())).unwrap();
    let page = (root as usize - 1) * 4096;
    for byte in &mut bytes[page..page + 4096] {
        *byte = 0xFF;
    }
    fs::write(db_path(dir.path()), bytes).unwrap();

    let error = store
        .list_messages(INBOX_ID, None, 50, &at(T0))
        .unwrap_err();
    assert!(
        matches!(error, kinige_store::StoreError::Corrupt(_)),
        "{error}"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    let settings = store.get_settings().unwrap();
    assert_eq!(
        settings.backup.error,
        Some(kinige_domain::BackupError::IntegrityFailed),
        "строка в «Настройках» совпадает с окном «База повреждена»"
    );
}
