//! Ревью guardian, продолжение `safety.rs`: проверка целостности, запуск с отметкой, гонка и прерывание восстановления,
//! Markdown-копия после восстановления, копия под непрерывной записью.

mod common;

use common::ops::*;
use common::*;
use kinige_domain::backup::daily_name;
use kinige_domain::groups::INBOX_ID;
use kinige_domain::history::DAY_MS;
use kinige_store::{Integrity, Now, StartupCheck, Store, backups_dir, startup_check};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

fn at(ms: i64) -> Now {
    moscow(ms)
}

/// Портит страницы в хвосте файла (как в `backup.rs`): проверка целостности это видит, открытие базы проходит.
fn damage_pages(path: &Path) {
    let mut bytes = fs::read(path).unwrap();
    let len = bytes.len();
    assert!(len > 40 * 1024, "достаточно страниц: {len}");
    for byte in &mut bytes[len - 12 * 1024..len - 4 * 1024] {
        *byte = 0xFF;
    }
    fs::write(path, bytes).unwrap();
}

fn big_database(dir: &Path) -> Store {
    let store = open_without_mirror(dir);
    for i in 0..300 {
        store
            .send_message(INBOX_ID, &"текст ".repeat(100 + i), &at(T0 + i as i64))
            .unwrap();
    }
    store
}

#[test]
fn verification_tells_corruption_from_a_failed_check() {
    let dir = tempfile::tempdir().unwrap();
    drop(big_database(dir.path()));
    wait_closed(dir.path());
    let db = db_path(dir.path());
    assert_eq!(kinige_store::verify_database(&db), Integrity::Ok);

    // Файла нет: проверять нечего — повреждение; пустой файл при этом не создаётся.
    let missing = dir.path().join("nothing.db");
    assert_eq!(kinige_store::verify_database(&missing), Integrity::Corrupt);
    assert!(!missing.exists());

    // Вместо файла каталог: открыть нельзя, но это не вердикт о содержимом — «не удалось проверить».
    let unreadable = dir.path().join("dir.db");
    fs::create_dir(&unreadable).unwrap();
    assert_eq!(
        kinige_store::verify_database(&unreadable),
        Integrity::Unknown
    );

    // Испорчены страницы и испорчен заголовок — повреждение.
    damage_pages(&db);
    assert_eq!(kinige_store::verify_database(&db), Integrity::Corrupt);
    let mut bytes = fs::read(&db).unwrap();
    bytes[..64].copy_from_slice(&[0xAB; 64]);
    fs::write(&db, bytes).unwrap();
    assert_eq!(kinige_store::verify_database(&db), Integrity::Corrupt);
}

#[test]
fn a_marker_with_a_healthy_database_is_cleared_and_the_database_opens_normally() {
    let dir = tempfile::tempdir().unwrap();
    drop(big_database(dir.path()));
    wait_closed(dir.path());
    let db = db_path(dir.path());
    assert_eq!(startup_check(&db), StartupCheck::Normal, "отметки нет");

    kinige_store::mark_needs_recovery(&db).unwrap();
    assert_eq!(startup_check(&db), StartupCheck::Normal, "проверка ok");
    assert!(!kinige_store::needs_recovery(&db), "отметка снята");

    // Повреждение подтверждается — окно с копиями, отметка остаётся.
    kinige_store::mark_needs_recovery(&db).unwrap();
    damage_pages(&db);
    assert_eq!(startup_check(&db), StartupCheck::Recovery);
    assert!(kinige_store::needs_recovery(&db));

    // Проверка не состоялась (вместо файла каталог): открываем как обычно и отметку не трогаем.
    let dir2 = tempfile::tempdir().unwrap();
    let db2 = db_path(dir2.path());
    fs::create_dir(&db2).unwrap();
    kinige_store::mark_needs_recovery(&db2).unwrap();
    assert_eq!(startup_check(&db2), StartupCheck::Normal);
    assert!(kinige_store::needs_recovery(&db2));
}

#[test]
fn an_empty_folder_beside_backups_and_an_interrupted_restore_offer_the_copies() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    store.send_message(INBOX_ID, "из копии", &at(T0)).unwrap();
    store.run_maintenance(&at(T0)).unwrap();
    drop(store);
    wait_closed(dir.path());
    let db = db_path(dir.path());

    // Прерванное восстановление: файла базы нет, копии есть, а отметки «восстанавливается» хватает и при целой базе.
    fs::remove_file(&db).ok();
    let _ = fs::remove_file(dir.path().join("kinige.db-wal"));
    let _ = fs::remove_file(dir.path().join("kinige.db-shm"));
    assert!(kinige_store::database_missing_but_backups_exist(&db));
    assert_eq!(startup_check(&db), StartupCheck::Recovery);
    assert!(!db.exists(), "пустая база не создана");

    let name = kinige_store::backups(&db)[0].name.clone();
    kinige_store::restore_backup(&db, &name, T0, None).unwrap();
    assert_eq!(startup_check(&db), StartupCheck::Normal);

    // Отметка «восстановление не закончено» требует окна с копиями, даже если база цела.
    fs::write(dir.path().join("needs-recovery"), b"restoring").unwrap();
    assert_eq!(startup_check(&db), StartupCheck::Recovery);
    kinige_store::clear_needs_recovery(&db);
    assert_eq!(startup_check(&db), StartupCheck::Normal);

    // Новая установка: ни базы, ни копий — обычный запуск.
    let fresh = tempfile::tempdir().unwrap();
    assert_eq!(startup_check(&db_path(fresh.path())), StartupCheck::Normal);
}

// ---------- восстановление ----------

#[test]
fn two_simultaneous_restores_leave_a_readable_database_and_no_leftovers() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    store.send_message(INBOX_ID, "из копии", &at(T0)).unwrap();
    store.run_maintenance(&at(T0)).unwrap();
    drop(store);
    wait_closed(dir.path());
    let db = db_path(dir.path());
    let name = kinige_store::backups(&db)[0].name.clone();
    fs::write(&db, b"broken").unwrap();

    let results: Vec<_> = (0..4)
        .map(|i| {
            let (db, name) = (db.clone(), name.clone());
            std::thread::spawn(move || kinige_store::restore_backup(&db, &name, T0 + i, None))
        })
        .collect::<Vec<_>>()
        .into_iter()
        .map(|t| t.join().unwrap())
        .collect();
    // По очереди каждое восстановление завершается целиком (повторное просто заменяет базу копией).
    assert!(results.iter().all(Result::is_ok), "{results:?}");
    let store = open_without_mirror(dir.path());
    assert_eq!(
        store
            .list_messages(INBOX_ID, None, 10, &at(T0 + 1))
            .unwrap()
            .messages
            .len(),
        1
    );
    assert!(!kinige_store::needs_recovery(&db));
    let leftovers: Vec<String> = fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter(|n| n.starts_with(".restore-"))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}

#[test]
fn the_old_markdown_folder_moves_aside_and_the_restored_database_rewrites_the_copy() {
    let dir = tempfile::tempdir().unwrap();
    let mirror = mirror_dir(dir.path());
    let store = open(dir.path());
    store.send_message(INBOX_ID, "до копии", &at(T0)).unwrap();
    wait_until("файл", || {
        mirror.join("Inbox/2026/2026-10-21.md").exists()
    });
    store.run_maintenance(&at(T0)).unwrap();
    store
        .send_message(INBOX_ID, "после копии", &at(T0 + MINUTE))
        .unwrap();
    wait_until("вторая запись в копии", || {
        fs::read_to_string(mirror.join("Inbox/2026/2026-10-21.md"))
            .is_ok_and(|t| t.contains("после копии"))
    });
    drop(store);
    wait_closed(dir.path());
    let db = db_path(dir.path());
    let name = kinige_store::backups(&db)[0].name.clone();
    fs::write(&db, b"broken").unwrap();

    let report =
        kinige_store::restore_backup(&db, &name, 1_792_563_120_000, Some(&mirror)).unwrap();
    let moved = report.previous_mirror.expect("папка переехала");
    assert_eq!(
        moved.file_name().unwrap().to_str().unwrap(),
        "mirror-before-restore-20261021T061200Z"
    );
    assert!(!mirror.exists() && moved.join("Inbox/2026/2026-10-21.md").exists());
    assert_eq!(
        count_rows(dir.path(), "mirror_dirty"),
        1,
        "все дни помечены"
    );

    // Копия пишется заново из восстановленной базы: прежние файлы остались рядом нетронутыми.
    let _store = open(dir.path());
    wait_until("новая копия", || {
        mirror.join("Inbox/2026/2026-10-21.md").exists()
    });
    let fresh = fs::read_to_string(mirror.join("Inbox/2026/2026-10-21.md")).unwrap();
    assert!(
        fresh.contains("до копии") && !fresh.contains("после копии"),
        "{fresh}"
    );
    let old = fs::read_to_string(moved.join("Inbox/2026/2026-10-21.md")).unwrap();
    assert!(
        old.contains("после копии"),
        "записи после снимка не потеряны"
    );
}

#[test]
fn restoring_into_a_copy_without_a_device_id_writes_one() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    store.run_maintenance(&at(T0)).unwrap();
    drop(store);
    wait_closed(dir.path());
    let db = db_path(dir.path());
    let backup = backups_dir(&db).join(daily_name("2026-10-21"));
    rusqlite::Connection::open(&backup)
        .unwrap()
        .execute("DELETE FROM sync_state WHERE key = 'device_id'", [])
        .unwrap();
    kinige_store::restore_backup(&db, "daily-2026-10-21.db", T0, None).unwrap();
    let id: String = raw(dir.path())
        .query_row(
            "SELECT value FROM sync_state WHERE key = 'device_id'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(kinige_domain::hlc::is_uuid(&id));
}

// ---------- копия под нагрузкой ----------

#[test]
fn the_copy_of_a_database_over_256_pages_completes_under_continuous_writes() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    for i in 0..400 {
        store
            .send_message(INBOX_ID, &"я".repeat(3500 + i), &at(T0 + i as i64))
            .unwrap();
    }
    let pages: i64 = raw(dir.path())
        .query_row("PRAGMA page_count", [], |r| r.get(0))
        .unwrap();
    assert!(
        pages > 256,
        "больше одного шага постраничного копирования: {pages}"
    );

    let stop = Arc::new(AtomicBool::new(false));
    let writer = {
        let (store, stop) = (store.clone(), stop.clone());
        std::thread::spawn(move || {
            let mut i = 0;
            while !stop.load(Ordering::SeqCst) {
                store
                    .send_message(INBOX_ID, &format!("пишу {i}"), &at(T0 + 10_000 + i))
                    .unwrap();
                i += 1;
            }
            i
        })
    };
    let report = store.run_maintenance(&at(T0 + DAY_MS)).unwrap();
    stop.store(true, Ordering::SeqCst);
    let written = writer.join().unwrap();
    assert!(written > 0);
    assert!(report.backup_made, "копия снята, а не зациклилась");
    let copy = backups_dir(&db_path(dir.path())).join(daily_name("2026-10-22"));
    let conn = rusqlite::Connection::open(&copy).unwrap();
    let verdict: String = conn
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(verdict, "ok");
    let n: i64 = conn
        .query_row("SELECT COUNT(*) FROM message", [], |r| r.get(0))
        .unwrap();
    assert!(n >= 400, "{n}");
}
