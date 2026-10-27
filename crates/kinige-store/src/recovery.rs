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
            crate::backup::Integrity::Ok => {
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

/// Можно ли по просьбе окна перезапустить приложение для восстановления: только если повреждение действительно найдено
/// (отметка стоит). Просьба без отметки отвергается — отметку она не ставит.
pub fn restart_for_recovery_allowed(db_path: &Path) -> bool {
    needs_recovery(db_path)
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
    let verdict: Result<String, _> = conn.query_row("PRAGMA quick_check", [], |row| row.get(0));
    matches!(version, Ok(v) if (1..=LATEST_VERSION).contains(&v))
        && matches!(verdict, Ok(v) if v == "ok")
}

/// Что сделало восстановление.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreReport {
    /// Куда переехала прежняя папка Markdown-копии; `None` — копия выключена или папки не было.
    pub previous_mirror: Option<PathBuf>,
}

/// Восстанавливает базу из копии `name` (имя файла из [`backups`]). Повреждённая база (если файл есть) уходит в
/// `corrupt-<now_ms>.db` вместе с `-wal` и `-shm`; копия встаёт на место; `device_id` создаётся новый. Прежняя папка
/// Markdown-копии (`default_mirror` — папка по умолчанию на случай, когда пользователь её не менял) переименовывается в
/// `<имя>-before-restore-<метка>`, все дни помечаются для перезаписи. Отметка «нужно восстановление» снимается в конце.
/// Копия не найдена или не проходит проверку — `BackupNotFound`, ничего не тронуто.
pub fn restore_backup(
    db_path: &Path,
    name: &str,
    now_ms: i64,
    default_mirror: Option<&Path>,
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

    // С этого момента и до конца база в неопределённом состоянии: упасть можно, а пустую базу молча создать — нет.
    fs::write(marker_path(db_path), RESTORING).map_err(io)?;

    // Копия сначала целиком рядом с базой (имя уникально), и только потом занимает её место.
    let staged = dir.join(format!(".restore-{}.tmp", Uuid::new_v4()));
    let placed = (|| -> Result<(), StoreError> {
        fs::copy(&source, &staged).map_err(io)?;
        fs::File::open(&staged)
            .and_then(|f| f.sync_all())
            .map_err(io)?;
        let corrupt = |suffix: &str| dir.join(format!("corrupt-{now_ms}.db{suffix}"));
        for (from, to) in [
            (db_path.to_path_buf(), corrupt("")),
            (sidecar(db_path, "-wal"), corrupt("-wal")),
            (sidecar(db_path, "-shm"), corrupt("-shm")),
        ] {
            if from.exists() {
                fs::rename(&from, &to).map_err(io)?;
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
        return Err(error);
    }

    let mut conn = Connection::open(db_path).map_err(write_err)?;
    // Новый `device_id` и прежняя папка копии — в одной транзакции с отметкой «все дни на перезапись».
    let previous_mirror = {
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(write_err)?;
        tx.execute(
            "INSERT INTO sync_state(key, value) VALUES ('device_id', ?1) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [Uuid::new_v4().to_string()],
        )
        .map_err(write_err)?;
        let mirror = effective_mirror_dir(&tx, default_mirror)
            .map_err(read_err)?
            .0;
        mark_all_dirty(&tx).map_err(write_err)?;
        tx.commit().map_err(write_err)?;
        mirror
    };
    // Контроль: после замены база читается.
    conn.query_row("SELECT COUNT(*) FROM chat", [], |row| row.get::<_, i64>(0))
        .map_err(read_err)?;
    drop(conn);
    let moved = previous_mirror.and_then(|folder| move_aside(&folder, now_ms));
    clear_needs_recovery(db_path);
    Ok(RestoreReport {
        previous_mirror: moved,
    })
}

/// Папка копии → `<имя>-before-restore-<метка>` рядом. Нет папки или переименование не удалось — `None`: восстановление
/// из-за этого не отменяется (копия в новой базе пишется заново, а прежние файлы остаются на месте).
fn move_aside(folder: &Path, now_ms: i64) -> Option<PathBuf> {
    if !folder.is_dir() {
        return None;
    }
    let name = folder.file_name()?.to_string_lossy().into_owned();
    let target = folder.with_file_name(format!("{name}-before-restore-{}", utc_stamp(now_ms)));
    if target.exists() {
        return None;
    }
    fs::rename(folder, &target).ok()?;
    Some(target)
}

fn sidecar(db_path: &Path, suffix: &str) -> PathBuf {
    let mut name = db_path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}
