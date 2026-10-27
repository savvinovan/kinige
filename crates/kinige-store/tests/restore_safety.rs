//! Повторное ревью guardian: папка Markdown-копии уходит в сторону до любого переноса базы, сбой до первого `rename` всё
//! возвращает, проверка целостности идёт раз в сутки и после находки в базу ничего не пишется, `verify_database` только
//! читает, автоудаление не блокируется пропущенными записями, восстановление из копии схемы 2 мигрирует базу.

mod common;

use common::ops::*;
use common::*;
use kinige_domain::ErrorCode;
use kinige_domain::backup::pre_migration_name;
use kinige_domain::groups::INBOX_ID;
use kinige_domain::history::DAY_MS;
use kinige_store::{
    CorruptionHook, Integrity, Now, Store, StoreError, backups, backups_dir, verify_database,
};
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

/// База с одной записью и копией дня; возвращает имя копии. Хранилище закрыто.
fn database_with_copy(dir: &Path) -> String {
    let store = open_without_mirror(dir);
    store.send_message(INBOX_ID, "из копии", &at(T0)).unwrap();
    store.run_maintenance(&at(T0)).unwrap();
    drop(store);
    wait_closed(dir);
    backups(&db_path(dir))[0].name.clone()
}

/// Всё, что лежит рядом с базой (имена), без папки копий и самой базы: следы незаконченного восстановления.
fn leftovers(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().into_string().unwrap())
        .filter(|n| n.starts_with(".restore-") || n.starts_with("corrupt-"))
        .collect();
    names.sort();
    names
}

fn marker(dir: &Path) -> Option<Vec<u8>> {
    fs::read(dir.join("needs-recovery")).ok()
}

// ---------- папка Markdown-копии: первой, до переноса базы ----------

#[test]
fn a_failure_before_the_first_rename_puts_everything_back() {
    for prior in [None, Some(b"".to_vec())] {
        let dir = tempfile::tempdir().unwrap();
        let name = database_with_copy(dir.path());
        let mirror = mirror_dir(dir.path());
        fs::create_dir_all(mirror.join("Inbox")).unwrap();
        fs::write(mirror.join("Inbox/keep.md"), "моё").unwrap();
        if prior.is_some() {
            kinige_store::mark_needs_recovery(&db_path(dir.path())).unwrap();
        }
        let before = fs::read(db_path(dir.path())).unwrap();

        let error = kinige_store::restore_backup_with(
            &db_path(dir.path()),
            &name,
            T0,
            Some(&mirror),
            &|| Err(StoreError::Write("диск полон".into())),
            &|| Ok(()),
        )
        .unwrap_err();

        assert!(matches!(error, StoreError::Write(_)), "{error}");
        assert_eq!(fs::read(db_path(dir.path())).unwrap(), before, "база цела");
        assert_eq!(
            marker(dir.path()),
            prior,
            "отметка как была (стояла: {})",
            prior.is_some()
        );
        assert!(
            mirror.join("Inbox/keep.md").exists(),
            "папка копии вернулась на место"
        );
        assert!(
            !fs::read_dir(dir.path())
                .unwrap()
                .flatten()
                .any(|e| e.file_name().to_string_lossy().contains("before-restore")),
            "переименованной папки не осталось"
        );
        assert!(leftovers(dir.path()).is_empty(), "временных файлов нет");
        // И повторная попытка проходит.
        kinige_store::restore_backup(&db_path(dir.path()), &name, T0, Some(&mirror)).unwrap();
    }
}

#[test]
fn a_failure_after_the_first_rename_keeps_the_mark_and_the_retry_reports_the_folder_moved_before() {
    let dir = tempfile::tempdir().unwrap();
    let name = database_with_copy(dir.path());
    let mirror = mirror_dir(dir.path());
    fs::create_dir_all(mirror.join("Inbox")).unwrap();
    fs::write(mirror.join("Inbox/keep.md"), "моё").unwrap();

    // Сбой сразу после того, как файл базы ушёл в `corrupt-…`: папка уже переехала, базы на месте нет.
    let error = kinige_store::restore_backup_with(
        &db_path(dir.path()),
        &name,
        T0,
        Some(&mirror),
        &|| Ok(()),
        &|| Err(StoreError::Write("сбой посреди переименования".into())),
    )
    .unwrap_err();
    assert!(matches!(error, StoreError::Write(_)), "{error}");
    assert_eq!(
        marker(dir.path()),
        Some(b"restoring".to_vec()),
        "отметка осталась"
    );
    assert!(!db_path(dir.path()).exists(), "базы на месте нет");
    assert!(!mirror.exists(), "папка уже переехала");
    let moved: Vec<String> = fs::read_dir(dir.path())
        .unwrap()
        .flatten()
        .map(|e| e.file_name().into_string().unwrap())
        .filter(|n| n.contains("before-restore"))
        .collect();
    assert_eq!(moved.len(), 1, "{moved:?}");
    // При таком состоянии запуск предлагает копии, а не создаёт пустую базу.
    assert_eq!(
        kinige_store::startup_check(&db_path(dir.path())),
        kinige_store::StartupCheck::Recovery
    );

    // Повтор: проходит, папку второй раз не переносит, но окно узнаёт про ту, что переехала в прошлой попытке.
    let report =
        kinige_store::restore_backup(&db_path(dir.path()), &name, T0 + MINUTE, Some(&mirror))
            .unwrap();
    assert_eq!(
        report.previous_mirror.as_deref(),
        Some(dir.path().join(&moved[0]).as_path())
    );
    let after: Vec<String> = fs::read_dir(dir.path())
        .unwrap()
        .flatten()
        .map(|e| e.file_name().into_string().unwrap())
        .filter(|n| n.contains("before-restore"))
        .collect();
    assert_eq!(after, moved, "второй папки не появилось");
    assert!(dir.path().join(&moved[0]).join("Inbox/keep.md").exists());
    assert_eq!(marker(dir.path()), None, "отметка снята");
    assert_eq!(count_rows(dir.path(), "message"), 1, "база восстановлена");
}

#[test]
fn a_restore_that_was_not_interrupted_does_not_borrow_an_old_before_restore_folder() {
    let dir = tempfile::tempdir().unwrap();
    let name = database_with_copy(dir.path());
    let mirror = mirror_dir(dir.path());
    // Папка от давнего восстановления лежит рядом, а самой папки копии нет (копия выключена или пуста).
    fs::create_dir_all(dir.path().join("mirror-before-restore-20200101T000000Z")).unwrap();

    let report =
        kinige_store::restore_backup(&db_path(dir.path()), &name, T0, Some(&mirror)).unwrap();

    assert_eq!(report.previous_mirror, None);
}

#[test]
fn a_mirror_folder_that_cannot_be_renamed_stops_the_restore_before_anything_is_touched() {
    let dir = tempfile::tempdir().unwrap();
    let name = database_with_copy(dir.path());
    let mirror = mirror_dir(dir.path());
    fs::create_dir_all(mirror.join("Inbox")).unwrap();
    // Все допустимые имена «…-before-restore-…» заняты: переименовывать некуда (то же, что отказ ОС).
    let stamp = kinige_store::utc_stamp(T0);
    fs::create_dir(dir.path().join(format!("mirror-before-restore-{stamp}"))).unwrap();
    for n in 2..=999 {
        fs::create_dir(
            dir.path()
                .join(format!("mirror-before-restore-{stamp}-{n}")),
        )
        .unwrap();
    }
    let before = fs::read(db_path(dir.path())).unwrap();

    let error =
        kinige_store::restore_backup(&db_path(dir.path()), &name, T0, Some(&mirror)).unwrap_err();

    assert!(matches!(error, StoreError::MirrorFolderBusy), "{error}");
    assert_eq!(error.code(), ErrorCode::MirrorFolderBusy);
    assert_eq!(fs::read(db_path(dir.path())).unwrap(), before, "база цела");
    assert_eq!(marker(dir.path()), None, "отметку не ставили");
    assert!(mirror.join("Inbox").is_dir(), "папка на месте");
    assert!(leftovers(dir.path()).is_empty());
}

#[cfg(unix)]
#[test]
fn an_os_refusal_to_rename_the_mirror_folder_is_the_same_error() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let name = database_with_copy(dir.path());
    let parent = dir.path().join("mirrors");
    let mirror = parent.join("Kinige");
    fs::create_dir_all(mirror.join("Inbox")).unwrap();
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o555)).unwrap();
    // Под root права не действуют (контейнер CI): тогда проверять нечего.
    let enforced = fs::write(parent.join("probe"), b"").is_err();
    let before = fs::read(db_path(dir.path())).unwrap();

    let result = kinige_store::restore_backup(&db_path(dir.path()), &name, T0, Some(&mirror));

    fs::set_permissions(&parent, fs::Permissions::from_mode(0o755)).unwrap();
    if !enforced {
        return;
    }
    assert!(matches!(result, Err(StoreError::MirrorFolderBusy)));
    assert_eq!(fs::read(db_path(dir.path())).unwrap(), before);
    assert_eq!(marker(dir.path()), None);
    assert!(mirror.join("Inbox").is_dir());
}

#[test]
fn two_restores_in_one_second_get_different_folder_names() {
    let dir = tempfile::tempdir().unwrap();
    let name = database_with_copy(dir.path());
    let mirror = mirror_dir(dir.path());
    fs::create_dir_all(mirror.join("Inbox")).unwrap();
    let first = kinige_store::restore_backup(&db_path(dir.path()), &name, T0, Some(&mirror))
        .unwrap()
        .previous_mirror
        .unwrap();
    fs::create_dir_all(mirror.join("Inbox")).unwrap();
    let second = kinige_store::restore_backup(&db_path(dir.path()), &name, T0, Some(&mirror))
        .unwrap()
        .previous_mirror
        .unwrap();
    assert_ne!(first, second);
    assert!(first.is_dir() && second.is_dir(), "обе папки сохранены");
    assert!(
        second
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .ends_with("-2"),
        "{second:?}"
    );
}

#[test]
fn the_folder_to_move_is_the_one_the_backup_itself_names() {
    let dir = tempfile::tempdir().unwrap();
    let chosen = dir.path().join("my-notes");
    let default = mirror_dir(dir.path());
    fs::create_dir_all(chosen.join("Inbox")).unwrap();
    fs::create_dir_all(default.join("Inbox")).unwrap();
    let store = open_without_mirror(dir.path());
    store.run_maintenance(&at(T0)).unwrap();
    drop(store);
    wait_closed(dir.path());
    // Пользователь выбрал свою папку: она записана в настройках, копия снимается уже с ней.
    raw(dir.path())
        .execute(
            "INSERT INTO app_setting(key, value) VALUES ('mirror_dir', ?1)",
            [chosen.to_string_lossy().into_owned()],
        )
        .unwrap();
    let store = open_without_mirror(dir.path());
    fs::remove_file(backups_dir(&db_path(dir.path())).join(&backups(&db_path(dir.path()))[0].name))
        .unwrap();
    store.run_maintenance(&at(T0 + MINUTE)).unwrap();
    drop(store);
    wait_closed(dir.path());
    let name = backups(&db_path(dir.path()))[0].name.clone();

    let report =
        kinige_store::restore_backup(&db_path(dir.path()), &name, T0, Some(&default)).unwrap();

    let moved = report.previous_mirror.unwrap();
    assert!(
        moved
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("my-notes-before-restore-"),
        "{moved:?}"
    );
    assert!(
        !chosen.exists() && default.join("Inbox").is_dir(),
        "чужая по умолчанию папка не тронута"
    );
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
         DROP TABLE mirror_chat;",
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
    // Что делает `resume_after_restore`: открывает восстановленную базу (схема 2) — она мигрирует до 3.
    let store = Store::open_at(options(dir.path()), &at(T0 + 10 * MINUTE)).unwrap();

    let version: i64 = raw(dir.path())
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, 3);
    assert_eq!(count_rows(dir.path(), "message"), 1);
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
