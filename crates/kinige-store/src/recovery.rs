//! Восстановление из копии (слайс 3, ТЗ §10.4, §12.3; ADR-0001 §1). Работает с файлами, пока база не открыта в этом
//! процессе: повреждённый файл не удаляется, а откладывается рядом как `corrupt-<время>.db`, копия встаёт на его место, а
//! `device_id` создаётся новый — база восстановлена из копии, это новый экземпляр (иначе два экземпляра с одним `device_id`
//! выдавали бы одинаковые метки HLC).

use crate::StoreError;
use crate::backup::{backups_dir, list_backups};
use crate::error::{read_err, write_err};
use crate::schema::LATEST_VERSION;
use kinige_domain::BackupInfo;
use kinige_domain::backup::classify;
use rusqlite::{Connection, OpenFlags};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const MARKER: &str = "needs-recovery";

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

/// Восстанавливает базу из копии `name` (имя файла из [`backups`]). Повреждённая база (если файл есть) уходит в
/// `corrupt-<now_ms>.db` вместе с `-wal` и `-shm`; копия встаёт на место; `device_id` создаётся новый. Отметка
/// «нужно восстановление» снимается. Копия не найдена или не проходит проверку — `BackupNotFound`, ничего не тронуто.
pub fn restore_backup(db_path: &Path, name: &str, now_ms: i64) -> Result<(), StoreError> {
    if classify(name).is_none() {
        return Err(StoreError::BackupNotFound);
    }
    let source = backups_dir(db_path).join(name);
    if !source.is_file() || !usable(&source) {
        return Err(StoreError::BackupNotFound);
    }
    let io = |error: std::io::Error| StoreError::Write(error.to_string());
    let dir = db_path.parent().unwrap_or_else(|| Path::new("."));

    // Копия сначала целиком рядом с базой, и только потом занимает её место.
    let staged = dir.join(".restore.tmp");
    let _ = fs::remove_file(&staged);
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

    let conn = Connection::open(db_path).map_err(write_err)?;
    conn.execute(
        "UPDATE sync_state SET value = ?1 WHERE key = 'device_id'",
        [Uuid::new_v4().to_string()],
    )
    .map_err(write_err)?;
    // Контроль: после замены база читается.
    conn.query_row("SELECT COUNT(*) FROM chat", [], |row| row.get::<_, i64>(0))
        .map_err(read_err)?;
    let _ = fs::remove_file(marker_path(db_path));
    Ok(())
}

fn sidecar(db_path: &Path, suffix: &str) -> PathBuf {
    let mut name = db_path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}
