//! Настройки (`app_setting`, не синхронизируются): папка Markdown-копии и размер текста (ADR-0004).

use crate::send::mark_all_dirty;
use crate::{Store, StoreError};
use kinige_domain::{MirrorStatus, Settings, TextSize};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior};
use std::path::{Path, PathBuf};

const KEY_MIRROR_DIR: &str = "mirror_dir";
const KEY_TEXT_SIZE: &str = "text_size";

fn read_err(error: rusqlite::Error) -> StoreError {
    StoreError::Read(error.to_string())
}

fn write_err(error: rusqlite::Error) -> StoreError {
    StoreError::Write(error.to_string())
}

pub(crate) fn get_setting(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT value FROM app_setting WHERE key = ?1",
        [key],
        |row| row.get(0),
    )
    .optional()
}

/// Папка копии: выбранная пользователем или по умолчанию. `None` — копия выключена. Второе значение — «по умолчанию».
pub(crate) fn effective_mirror_dir(
    conn: &Connection,
    default: Option<&Path>,
) -> rusqlite::Result<(Option<PathBuf>, bool)> {
    Ok(match get_setting(conn, KEY_MIRROR_DIR)? {
        Some(chosen) => (Some(PathBuf::from(chosen)), false),
        None => (default.map(Path::to_path_buf), true),
    })
}

impl Store {
    /// Настройки и состояние копии для окна «Настройки».
    pub fn get_settings(&self) -> Result<Settings, StoreError> {
        let default = self.inner.default_mirror_dir.clone();
        let (text_size, folder, pending_days) = self.read(|conn| {
            let text_size = get_setting(conn, KEY_TEXT_SIZE).map_err(read_err)?;
            let folder = effective_mirror_dir(conn, default.as_deref()).map_err(read_err)?;
            let pending: i64 = conn
                .query_row("SELECT COUNT(*) FROM mirror_dirty", [], |row| row.get(0))
                .map_err(read_err)?;
            Ok((text_size, folder, pending))
        })?;
        Ok(Settings {
            text_size: text_size
                .as_deref()
                .map_or(TextSize::Normal, TextSize::from_setting),
            mirror: MirrorStatus {
                folder: folder.0.map(|path| path.to_string_lossy().into_owned()),
                is_default: folder.1,
                error: self.inner.mirror.error(),
                pending_days,
            },
            shortcuts: Vec::new(),
        })
    }

    /// «Размер текста»: хранится, переживает перезапуск.
    pub fn set_text_size(&self, size: TextSize) -> Result<(), StoreError> {
        self.write(move |conn| {
            conn.execute(
                "INSERT INTO app_setting(key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                (KEY_TEXT_SIZE, size.as_setting()),
            )
            .map_err(write_err)?;
            Ok(())
        })
    }

    /// Новая папка копии (путь пришёл из системного диалога в Rust). В той же транзакции все дни с записями помечаются
    /// для перезаписи: копия в новой папке получается полной, а не только с новых записей.
    pub fn set_mirror_dir(&self, dir: &Path) -> Result<(), StoreError> {
        let text = dir
            .to_str()
            .ok_or_else(|| StoreError::Internal("путь не в UTF-8".into()))?
            .to_string();
        self.write(move |conn| {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate).map_err(write_err)?;
            tx.execute(
                "INSERT INTO app_setting(key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                (KEY_MIRROR_DIR, text),
            )
            .map_err(write_err)?;
            mark_all_dirty(&tx).map_err(write_err)?;
            tx.commit().map_err(write_err)
        })?;
        self.inner.mirror.clear_error();
        self.wake_mirror();
        Ok(())
    }
}
