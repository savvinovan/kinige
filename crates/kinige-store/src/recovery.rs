//! Восстановление из копии (слайс 3, ТЗ §10.4, §12.3; ADR-0001 §1). Работает с файлами, пока база не открыта в этом
//! процессе: повреждённый файл не удаляется, а откладывается рядом как `corrupt-<время>.db`, копия встаёт на его место, а
//! `device_id` создаётся новый — база восстановлена из копии, это новый экземпляр (иначе два экземпляра с одним `device_id`
//! выдавали бы одинаковые метки HLC).
//!
//! Отметка `needs-recovery` ставится в начале восстановления и снимается в конце: сбой между двумя `rename` оставил бы при
//! следующем запуске папку без файла базы, и приложение молча создало бы пустую базу. Папка Markdown-копии переименовывается
//! в `<имя>-before-restore-<метка>` и все дни помечаются для перезаписи: иначе первая правка дня затёрла бы файл с записями,
//! сделанными после снимка копии.

use crate::backup::{backups_dir, list_backups};
use crate::error::{read_err, write_err};
use crate::schema::LATEST_VERSION;
use crate::send::mark_all_dirty;
use crate::settings::effective_mirror_dir;
use crate::settings_file;
use crate::{StoreError, time::utc_stamp};
use kinige_domain::BackupInfo;
use kinige_domain::backup::classify;
use rusqlite::{Connection, OpenFlags, TransactionBehavior};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use uuid::Uuid;

const MARKER: &str = "needs-recovery";

/// Восстановления в одном процессе идут по очереди: два одновременных вызова не должны делить промежуточные файлы.
static RESTORE_LOCK: Mutex<()> = Mutex::new(());

fn marker_path(db_path: &Path) -> PathBuf {
    db_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(MARKER)
}

/// Проверка целостности нашла повреждение работающей базы: следующий запуск не открывает её, а предлагает копии.
pub fn mark_needs_recovery(db_path: &Path) -> std::io::Result<()> {
    fs::write(marker_path(db_path), b"")
}

/// Стоит ли отметка «нужно восстановление» (см. [`mark_needs_recovery`]).
pub fn needs_recovery(db_path: &Path) -> bool {
    marker_path(db_path).exists()
}

/// Снимает отметку (база проверена и цела, либо восстановление закончено).
pub fn clear_needs_recovery(db_path: &Path) {
    let _ = fs::remove_file(marker_path(db_path));
}

/// Восстановление началось и не закончено: в отметке написано `restoring`.
const RESTORING: &[u8] = b"restoring";

fn restore_interrupted(db_path: &Path) -> bool {
    fs::read(marker_path(db_path)).is_ok_and(|content| content == RESTORING)
}

/// С чего начинать запуск.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupCheck {
    /// Открыть базу как обычно.
    Normal,
    /// Журнал не открывать, показать окно «База повреждена» с копиями.
    Recovery,
}

/// Решение при запуске до открытия базы:
/// - отметка «восстановление не закончено» — всегда окно с копиями (база могла остаться без нового `device_id`);
/// - отметка «нужно восстановление» от фоновой проверки — повторная синхронная проверка: `ok` — отметка снимается, база
///   открывается как обычно (проверка прошлого запуска могла ошибиться или повреждение было временным); вердикт «повреждена»
///   или нет файла — окно с копиями; проверка не состоялась (занято, ввод-вывод) — открываем как обычно, отметку оставляем;
/// - отметки нет, файла базы нет, а копии есть — окно с копиями: молча создать пустую базу рядом с копиями нельзя.
pub fn startup_check(db_path: &Path) -> StartupCheck {
    if restore_interrupted(db_path) {
        return StartupCheck::Recovery;
    }
    if needs_recovery(db_path) {
        return match crate::backup::verify_database(db_path) {
            // Порча только индекса поиска — не повод к восстановлению: отметка снимается, индекс вылечит первый запрос
            // или проход обслуживания.
            crate::backup::Integrity::Ok | crate::backup::Integrity::SearchIndex => {
                clear_needs_recovery(db_path);
                StartupCheck::Normal
            }
            crate::backup::Integrity::Corrupt => StartupCheck::Recovery,
            crate::backup::Integrity::Unknown => StartupCheck::Normal,
        };
    }
    if database_missing_but_backups_exist(db_path) {
        StartupCheck::Recovery
    } else {
        StartupCheck::Normal
    }
}

/// Файла базы нет, а копии в папке есть: так выглядит прерванное восстановление (или потерянный файл). Молча создать
/// пустую базу рядом с копиями нельзя — предлагаются копии.
pub fn database_missing_but_backups_exist(db_path: &Path) -> bool {
    !db_path.exists() && !list_backups(&backups_dir(db_path)).is_empty()
}

/// Копии, из которых можно восстановить, новые первыми.
pub fn backups(db_path: &Path) -> Vec<BackupInfo> {
    list_backups(&backups_dir(db_path))
}

/// Копия открывается, проходит `PRAGMA quick_check` и сделана схемой, которую это приложение понимает.
fn usable(path: &Path) -> bool {
    let Ok(conn) = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY) else {
        return false;
    };
    let version: Result<i64, _> = conn.pragma_query_value(None, "user_version", |row| row.get(0));
    // Копия с повреждённым индексом поиска годится: он производный и перестроится при первом же запросе или записи.
    matches!(version, Ok(v) if (1..=LATEST_VERSION).contains(&v))
        && matches!(crate::backup::quick_check_fine(&conn), Ok(true))
}

/// Что сделало восстановление.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreReport {
    /// Куда переехала прежняя папка Markdown-копии; `None` — копия выключена или папки не было.
    pub previous_mirror: Option<PathBuf>,
    /// Куда переехала папка копии, названная в `settings.json` (ADR-0010), если она другая; `None` — такой папки не было.
    pub settings_file_mirror: Option<PathBuf>,
}

/// Восстанавливает базу из копии `name` (имя файла из [`backups`]). Повреждённая база (если файл есть) уходит в
/// `corrupt-<now_ms>.db` вместе с `-wal` и `-shm`; копия встаёт на место; `device_id` создаётся новый. Прежняя папка
/// Markdown-копии (`default_mirror` — папка по умолчанию на случай, когда пользователь её не менял) переименовывается в
/// `<имя>-before-restore-<метка>` (имя уникально) ПЕРВЫМ делом, до любого переноса базы; все дни помечаются для перезаписи.
/// Отметка «нужно восстановление» снимается в конце. Копия не найдена или не проходит проверку — `BackupNotFound`, папку
/// переименовать не удалось — `MirrorFolderBusy`; в обоих случаях ничего не тронуто.
pub fn restore_backup(
    db_path: &Path,
    name: &str,
    now_ms: i64,
    default_mirror: Option<&Path>,
) -> Result<RestoreReport, StoreError> {
    restore_backup_with(
        db_path,
        name,
        now_ms,
        default_mirror,
        &|| Ok(()),
        &|| Ok(()),
    )
}

/// То же с проверочными шагами: `before_place` выполняется после копирования и до первого `rename` базы, `after_first_rename`
/// — сразу после первого `rename` (в них тесты имитируют сбой копирования, записи на диск или переименования). Не часть
/// API приложения.
#[doc(hidden)]
pub fn restore_backup_with(
    db_path: &Path,
    name: &str,
    now_ms: i64,
    default_mirror: Option<&Path>,
    before_place: &dyn Fn() -> Result<(), StoreError>,
    after_first_rename: &dyn Fn() -> Result<(), StoreError>,
) -> Result<RestoreReport, StoreError> {
    if classify(name).is_none() {
        return Err(StoreError::BackupNotFound);
    }
    let _guard = RESTORE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let source = backups_dir(db_path).join(name);
    if !source.is_file() || !usable(&source) {
        return Err(StoreError::BackupNotFound);
    }
    let io = |error: std::io::Error| StoreError::Write(error.to_string());
    let dir = db_path.parent().unwrap_or_else(|| Path::new("."));

    // 1. Папка Markdown-копии уходит в сторону ПЕРВОЙ, пока ничего больше не тронуто: не удалось переименовать (занята,
    // нет прав) — `MirrorFolderBusy`, всё на месте, человек закрывает папку и повторяет. Путь берётся из самой копии: именно
    // её настройка станет настройкой восстановленной базы.
    let mirror = mirror_folder_in(&source, default_mirror);
    // Повтор после сбоя восстановления (отметка `restoring`): папка уже переехала в прошлой попытке — окно должно узнать
    // про `…-before-restore-…`, поэтому берётся самая поздняя такая папка рядом.
    let earlier = if restore_interrupted(db_path) {
        mirror.as_deref().and_then(latest_before_restore)
    } else {
        None
    };
    // Папка из `settings.json` (ADR-0010) тоже уходит в сторону, если она другая: файл применится поверх восстановленной
    // базы, поставит эту папку копией и пометит все дни на перезапись, а писатель копии удаляет в папке файлы дней, которых
    // нет в базе, то есть дни, появившиеся после снимка. Файл читается только как чтение, с теми же проверками безопасности.
    let from_file = db_path
        .parent()
        .and_then(|data_dir| settings_file::mirror_folder_from_file(data_dir, default_mirror));
    let mut moved: Vec<(PathBuf, PathBuf)> = Vec::new();
    let undo_move = |moved: &[(PathBuf, PathBuf)]| {
        for (from, to) in moved.iter().rev() {
            let _ = fs::rename(to, from);
        }
    };
    for folder in mirror.iter().chain(from_file.iter()) {
        if !folder.is_dir() || moved.iter().any(|(from, _)| from == folder) {
            continue;
        }
        match move_aside(folder, now_ms) {
            Ok(target) => moved.push((folder.clone(), target)),
            Err(error) => {
                undo_move(&moved);
                return Err(error);
            }
        }
    }

    // 2. С этого момента база в неопределённом состоянии: упасть можно, а пустую базу молча создать — нет. Прежняя отметка
    // (например, «нужно восстановление» от фоновой проверки) запоминается: если до первого `rename` что-то не удалось,
    // база цела и отметка возвращается как была.
    let prior_marker = fs::read(marker_path(db_path)).ok();
    let restore_marker = || match &prior_marker {
        Some(content) => {
            let _ = fs::write(marker_path(db_path), content);
        }
        None => clear_needs_recovery(db_path),
    };
    if let Err(error) = fs::write(marker_path(db_path), RESTORING) {
        restore_marker();
        undo_move(&moved);
        return Err(io(error));
    }

    // 3. Копия сначала целиком рядом с базой (имя уникально), и только потом занимает её место.
    let staged = dir.join(format!(".restore-{}.tmp", Uuid::new_v4()));
    let mut base_touched = false;
    let placed = (|| -> Result<(), StoreError> {
        fs::copy(&source, &staged).map_err(io)?;
        fs::File::open(&staged)
            .and_then(|f| f.sync_all())
            .map_err(io)?;
        before_place()?;
        let corrupt = |suffix: &str| dir.join(format!("corrupt-{now_ms}.db{suffix}"));
        for (from, to) in [
            (db_path.to_path_buf(), corrupt("")),
            (sidecar(db_path, "-wal"), corrupt("-wal")),
            (sidecar(db_path, "-shm"), corrupt("-shm")),
        ] {
            if from.exists() {
                fs::rename(&from, &to).map_err(io)?;
                base_touched = true;
                after_first_rename()?;
            }
        }
        fs::rename(&staged, db_path).map_err(io)?;
        if let Ok(directory) = fs::File::open(dir) {
            let _ = directory.sync_all();
        }
        Ok(())
    })();
    if let Err(error) = placed {
        let _ = fs::remove_file(&staged);
        // Сбой до первого `rename` (копирование, сброс на диск): база и папка на месте, всё возвращается как было. После
        // первого `rename` отметка `restoring` остаётся: при запуске окно предложит копии.
        if !base_touched {
            restore_marker();
            undo_move(&moved);
        }
        return Err(error);
    }

    let mut conn = Connection::open(db_path).map_err(write_err)?;
    // Новый `device_id` — в одной транзакции с отметкой «все дни на перезапись».
    {
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(write_err)?;
        tx.execute(
            "INSERT INTO sync_state(key, value) VALUES ('device_id', ?1) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [Uuid::new_v4().to_string()],
        )
        .map_err(write_err)?;
        mark_all_dirty(&tx).map_err(write_err)?;
        // Доступ агентов из копии не переносится (ADR-0010): итог — ровно как в файле настроек, а если файл небезопасен или
        // не разобран — выключено. Хэша файла в копии нет: файл применится как внешняя правка.
        settings_file::reset_access_after_restore(&tx).map_err(write_err)?;
        tx.commit().map_err(write_err)?;
    }
    // Контроль: после замены база читается.
    conn.query_row("SELECT COUNT(*) FROM chat", [], |row| row.get::<_, i64>(0))
        .map_err(read_err)?;
    drop(conn);
    clear_needs_recovery(db_path);
    let target_of = |folder: &Option<PathBuf>| {
        let folder = folder.as_ref()?;
        moved
            .iter()
            .find(|(from, _)| from == folder)
            .map(|(_, target)| target.clone())
    };
    let previous_mirror = target_of(&mirror).or(earlier);
    let settings_file_mirror = if from_file == mirror {
        None
    } else {
        target_of(&from_file)
    };
    Ok(RestoreReport {
        previous_mirror,
        settings_file_mirror,
    })
}

/// Самая поздняя папка `<имя>-before-restore-…` рядом с `folder` (имена с меткой UTC сортируются по времени, суффикс
/// `-2` идёт позже того же момента). Папки самой `folder` может не быть: прошлая попытка её уже переименовала.
fn latest_before_restore(folder: &Path) -> Option<PathBuf> {
    let name = folder.file_name()?.to_string_lossy().into_owned();
    let prefix = format!("{name}-before-restore-");
    let parent = folder.parent().unwrap_or_else(|| Path::new("."));
    fs::read_dir(parent)
        .ok()?
        .flatten()
        .filter(|entry| {
            entry.path().is_dir() && entry.file_name().to_string_lossy().starts_with(&prefix)
        })
        .map(|entry| entry.path())
        .max()
}

/// Папка Markdown-копии по настройке самой копии (или по умолчанию, если пользователь её не менял); `None` — копия
/// выключена или настройка не читается (тогда ничего не переносится).
fn mirror_folder_in(backup: &Path, default_mirror: Option<&Path>) -> Option<PathBuf> {
    let conn = Connection::open_with_flags(backup, OpenFlags::SQLITE_OPEN_READ_ONLY).ok()?;
    effective_mirror_dir(&conn, default_mirror).ok()?.0
}

/// Папка копии → `<имя>-before-restore-<метка>` рядом; если такое имя занято (два восстановления в одну секунду) —
/// `…-2`, `…-3`. Переименовать не удалось — [`StoreError::MirrorFolderBusy`].
fn move_aside(folder: &Path, now_ms: i64) -> Result<PathBuf, StoreError> {
    let name = folder
        .file_name()
        .ok_or(StoreError::MirrorFolderBusy)?
        .to_string_lossy()
        .into_owned();
    let stamp = utc_stamp(now_ms);
    let target = (1..=999)
        .map(|n| {
            let suffix = if n == 1 {
                String::new()
            } else {
                format!("-{n}")
            };
            folder.with_file_name(format!("{name}-before-restore-{stamp}{suffix}"))
        })
        .find(|candidate| !candidate.exists())
        .ok_or(StoreError::MirrorFolderBusy)?;
    fs::rename(folder, &target).map_err(|_| StoreError::MirrorFolderBusy)?;
    Ok(target)
}

fn sidecar(db_path: &Path, suffix: &str) -> PathBuf {
    let mut name = db_path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}
