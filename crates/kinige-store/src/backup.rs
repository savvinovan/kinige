//! Копия файла базы и проверка целостности (ТЗ §2.1, §10.4, §12.3, слайс 3).
//!
//! Копия снимается через backup API SQLite (`rusqlite::backup`) **с отдельного соединения чтения**: простое `fs::copy` при
//! WAL даёт несогласованный файл (без `-wal`), а соединение писателя копия не блокирует. Копия пишется во временный файл,
//! проверяется (`PRAGMA quick_check`), переводится в режим журнала `DELETE` (один файл, без `-wal`), сбрасывается на диск и
//! переименовывается: читатель папки видит либо целую копию, либо ничего. Имена и ротация — `kinige_domain::backup`.

use kinige_domain::backup::{self, BackupInfo};
use kinige_domain::{BackupError, BackupStatus};
use rusqlite::{Connection, backup::Backup};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, UNIX_EPOCH};

/// Папка копий рядом с файлом базы.
pub fn backups_dir(db_path: &Path) -> PathBuf {
    db_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("backups")
}

/// `PRAGMA integrity_check` дал `ok`. Ошибка самой проверки (страницы настолько повреждены, что SQLite отказывает) — не `ok`.
pub(crate) fn integrity_ok(conn: &Connection) -> bool {
    conn.query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))
        .is_ok_and(|first| first == "ok")
}

/// Снимает копию `source` в `dir/name`. Существующий файл с тем же именем заменяется целиком.
pub(crate) fn snapshot(source: &Connection, dir: &Path, name: &str) -> Result<(), BackupError> {
    fs::create_dir_all(dir).map_err(|_| BackupError::FolderUnavailable)?;
    let temp = dir.join(format!(".{name}.tmp"));
    let _ = fs::remove_file(&temp);
    let result = write_snapshot(source, &temp).and_then(|()| {
        fs::rename(&temp, dir.join(name)).map_err(|_| BackupError::WriteFailed)?;
        // Само переименование тоже сбрасывается на диск. На Windows каталог открыть нельзя — ошибка игнорируется.
        if let Ok(directory) = fs::File::open(dir) {
            let _ = directory.sync_all();
        }
        Ok(())
    });
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

fn write_snapshot(source: &Connection, temp: &Path) -> Result<(), BackupError> {
    let failed = |_: rusqlite::Error| BackupError::WriteFailed;
    {
        let mut target = Connection::open(temp).map_err(failed)?;
        Backup::new(source, &mut target)
            .map_err(failed)?
            .run_to_completion(256, Duration::from_millis(2), None)
            .map_err(failed)?;
        // Копия — один файл: режим журнала WAL остался бы в её заголовке и требовал бы `-wal` рядом.
        target
            .pragma_update(None, "journal_mode", "DELETE")
            .map_err(failed)?;
        let verdict: String = target
            .query_row("PRAGMA quick_check", [], |row| row.get(0))
            .map_err(failed)?;
        if verdict != "ok" {
            return Err(BackupError::WriteFailed);
        }
    }
    fs::File::open(temp)
        .and_then(|file| file.sync_all())
        .map_err(|_| BackupError::WriteFailed)
}

/// Копии в папке, новые первыми (по времени изменения файла). Чужие и временные файлы не показываются.
pub fn list_backups(dir: &Path) -> Vec<BackupInfo> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<BackupInfo> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let kind = backup::classify(&name)?;
            let modified = entry.metadata().ok()?.modified().ok()?;
            let created_at = modified.duration_since(UNIX_EPOCH).ok()?.as_millis();
            Some(BackupInfo {
                name,
                kind,
                created_at: i64::try_from(created_at).ok()?,
            })
        })
        .collect();
    found.sort_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then_with(|| b.name.cmp(&a.name))
    });
    found
}

/// Удаляет копии старше ротации (7 копий дня, 3 перед миграцией); чужие файлы не трогает.
pub(crate) fn rotate(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let names: Vec<String> = entries
        .flatten()
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect();
    for stale in backup::expired(&names) {
        let _ = fs::remove_file(dir.join(stale));
    }
}

/// Копия перед миграцией существующей базы (ТЗ §10.4): имя несёт версию схемы, из которой мигрируем. Ошибка — миграция
/// не начинается.
pub(crate) fn before_migration(
    source: &Connection,
    db_path: &Path,
    from_version: i64,
    utc_stamp: &str,
) -> Result<(), BackupError> {
    let dir = backups_dir(db_path);
    snapshot(
        source,
        &dir,
        &backup::pre_migration_name(from_version, utc_stamp),
    )?;
    rotate(&dir);
    Ok(())
}

/// Состояние копий для «Настроек» и последняя ошибка (общая для потока обслуживания и окна).
#[derive(Default)]
pub(crate) struct BackupShared {
    error: Mutex<Option<BackupError>>,
}

impl BackupShared {
    pub(crate) fn set_error(&self, error: Option<BackupError>) {
        if let Ok(mut slot) = self.error.lock() {
            *slot = error;
        }
    }

    pub(crate) fn error(&self) -> Option<BackupError> {
        self.error.lock().ok().and_then(|slot| *slot)
    }

    pub(crate) fn status(&self, db_path: &Path) -> BackupStatus {
        let dir = backups_dir(db_path);
        let backups = list_backups(&dir);
        BackupStatus {
            folder: Some(dir.to_string_lossy().into_owned()),
            last_at: backups.first().map(|b| b.created_at),
            count: backups.len() as i64,
            error: self.error(),
        }
    }
}

/// Копия дня за эту локальную дату уже есть: вторую в тот же день не снимаем.
pub(crate) fn has_daily(dir: &Path, local_date: &str) -> bool {
    let name = backup::daily_name(local_date);
    dir.join(name).is_file()
}
