//! Повторное ревью guardian: папка Markdown-копии уходит в сторону до любого переноса базы, сбой до первого `rename` всё
//! возвращает, проверка целостности идёт раз в сутки и после находки в базу ничего не пишется, `verify_database` только
//! читает, автоудаление не блокируется пропущенными записями, восстановление из копии схемы 2 мигрирует базу.

mod common;

use common::ops::*;
use common::*;
use kinige_domain::ErrorCode;
use kinige_domain::groups::INBOX_ID;
use kinige_store::{Now, StoreError, backups, backups_dir};
use std::fs;
use std::path::Path;

fn at(ms: i64) -> Now {
    moscow(ms)
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
