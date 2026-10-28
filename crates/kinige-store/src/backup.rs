//! Копия файла базы и проверка целостности (ТЗ §2.1, §10.4, §12.3, слайс 3).
//!
//! Копия снимается через backup API SQLite (`rusqlite::backup`) **с отдельного соединения чтения**: простое `fs::copy` при
//! WAL даёт несогласованный файл (без `-wal`), а соединение писателя копия не блокирует. Копия пишется во временный файл,
//! проверяется (`PRAGMA quick_check`), переводится в режим журнала `DELETE` (один файл, без `-wal`), сбрасывается на диск и
//! переименовывается: читатель папки видит либо целую копию, либо ничего. Имена и ротация — `kinige_domain::backup`.

use crate::StoreError;
use kinige_domain::backup::{self, BackupInfo};
use kinige_domain::{BackupError, BackupStatus};
use rusqlite::{Connection, OpenFlags, backup::Backup};
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

/// Итог проверки целостности. «Не удалось проверить» (база занята, ошибка ввода-вывода, пропавший том) — не повреждение:
/// ложное «база повреждена» отправило бы человека восстанавливать здоровую базу.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Integrity {
    /// `PRAGMA integrity_check` дал `ok`.
    Ok,
    /// Вердикт не `ok`, либо SQLite отвечает «база повреждена» / «это не база данных».
    Corrupt,
    /// Проверка не состоялась по другой причине: повторить позже, отметку не ставить.
    Unknown,
}

/// Вердикт `PRAGMA integrity_check`.
pub(crate) fn check_integrity(conn: &Connection) -> Integrity {
    match conn.query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0)) {
        Ok(first) if first == "ok" => Integrity::Ok,
        Ok(_) => Integrity::Corrupt,
        Err(error) => match error.sqlite_error_code() {
            Some(rusqlite::ErrorCode::DatabaseCorrupt | rusqlite::ErrorCode::NotADatabase) => {
                Integrity::Corrupt
            }
            _ => Integrity::Unknown,
        },
    }
}

/// Проверка целостности файла базы отдельным соединением (при старте с отметкой «нужно восстановление» и перед миграцией).
pub fn verify_database(db_path: &Path) -> Integrity {
    // Файла нет — это не «проверка не удалась»: открытие создало бы пустую базу, а проверять нечего.
    if !db_path.exists() {
        return Integrity::Corrupt;
    }
    // Только чтение и без `CREATE`: проверка не должна ни создать файл, ни изменить его (ни `-wal`, ни режим журнала).
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let conn = match Connection::open_with_flags(db_path, flags) {
        Ok(conn) => conn,
        Err(error) => {
            return match crate::error::read_err(error) {
                StoreError::Corrupt(_) => Integrity::Corrupt,
                _ => Integrity::Unknown,
            };
        }
    };
    if conn.busy_timeout(Duration::from_secs(5)).is_err() {
        return Integrity::Unknown;
    }
    check_integrity(&conn)
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
        copy_all_pages(source, &mut target)?;
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

/// Все страницы одним шагом. Постраничное копирование (`run_to_completion(256, …)`) при непрерывной записи в базу
/// перезапускалось бы с первой страницы после каждого изменения и могло не закончиться; один шаг держит согласованный
/// снимок под блокировкой чтения (в WAL писатель при этом не ждёт). Занятость (`Busy`/`Locked`) — несколько повторов.
fn copy_all_pages(source: &Connection, target: &mut Connection) -> Result<(), BackupError> {
    use rusqlite::backup::StepResult;
    let failed = |_: rusqlite::Error| BackupError::WriteFailed;
    let backup = Backup::new(source, target).map_err(failed)?;
    for _ in 0..50 {
        match backup.step(-1).map_err(failed)? {
            StepResult::Done => return Ok(()),
            _ => std::thread::sleep(Duration::from_millis(100)),
        }
    }
    Err(BackupError::WriteFailed)
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

impl crate::Store {
    /// Копия перед импортом из Telegram (слайс 4): `backups/pre-import-<UTC>.db`, хранятся 3 последних. Снимается тем же
    /// способом, что копия дня: отдельное соединение чтения, согласованный снимок при работающем писателе. Возвращает имя
    /// файла. Ошибка (папка недоступна, не записалось) — [`StoreError::ImportBackupFailed`]: импорт не начинается.
    pub fn backup_before_import(&self, now: &crate::Now) -> Result<String, StoreError> {
        let db_path = &self.inner.db_path;
        let reader =
            crate::open_connection(db_path, true).map_err(|_| StoreError::ImportBackupFailed)?;
        let dir = backups_dir(db_path);
        let name = backup::pre_import_name(&crate::time::utc_stamp(now.ms));
        snapshot(&reader, &dir, &name).map_err(|_| StoreError::ImportBackupFailed)?;
        rotate(&dir);
        Ok(name)
    }
}

/// Состояние копий для «Настроек» и последняя ошибка (общая для потока обслуживания и окна).
#[derive(Default)]
pub(crate) struct BackupShared {
    error: Mutex<Option<BackupError>>,
    /// Автоудаление корзины не смогло убрать запись: видно в «Настройках», но проверку и копию не отменяет.
    purge_failed: std::sync::atomic::AtomicBool,
    /// Локальная дата последней успешной проверки целостности: проверка раз в сутки, а не каждый час.
    checked_on: Mutex<Option<String>>,
}

impl BackupShared {
    pub(crate) fn set_error(&self, error: Option<BackupError>) {
        if let Ok(mut slot) = self.error.lock() {
            *slot = error;
        }
    }

    /// Ошибка для окна: сбой копии или проверки важнее сбоя автоудаления.
    pub(crate) fn error(&self) -> Option<BackupError> {
        let main = self.error.lock().ok().and_then(|slot| *slot);
        main.or_else(|| {
            self.purge_failed
                .load(std::sync::atomic::Ordering::SeqCst)
                .then_some(BackupError::PurgeFailed)
        })
    }

    pub(crate) fn set_purge_failed(&self, failed: bool) {
        self.purge_failed
            .store(failed, std::sync::atomic::Ordering::SeqCst);
    }

    pub(crate) fn checked_on(&self) -> Option<String> {
        self.checked_on.lock().ok().and_then(|slot| slot.clone())
    }

    pub(crate) fn set_checked_on(&self, date: Option<String>) {
        if let Ok(mut slot) = self.checked_on.lock() {
            *slot = date;
        }
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
