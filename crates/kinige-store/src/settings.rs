//! Настройки (`app_setting`, не синхронизируются): папка Markdown-копии и размер текста (ADR-0004).

use crate::error::{read_err, write_err};
use crate::send::mark_all_dirty;
use crate::{Store, StoreError};
use kinige_domain::shortcuts::canonical_accelerator;
use kinige_domain::{
    MirrorStatus, Settings, ShortcutKind, ShortcutState, ShortcutStatus, TextSize,
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior};
use std::path::{Path, PathBuf};

const KEY_MIRROR_DIR: &str = "mirror_dir";
const KEY_TEXT_SIZE: &str = "text_size";
const KEY_SIDEBAR_COLLAPSED: &str = "sidebar_collapsed";
const KEY_RECENT_EMOJI: &str = "recent_emoji";
/// Сколько недавних эмодзи помнит пикер.
pub const MAX_RECENT_EMOJI: usize = 24;

/// Недавние эмодзи, самые свежие первыми. Повреждённое значение — пустой список: недавние не стоят ошибки.
fn recent_emoji(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    Ok(get_setting(conn, KEY_RECENT_EMOJI)?
        .and_then(|text| serde_json::from_str::<Vec<String>>(&text).ok())
        .unwrap_or_default())
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

/// Сохранённое сочетание вида `kind`; нет записи, незнакомое или повреждённое значение — сочетание по умолчанию.
fn stored_accelerator(conn: &Connection, kind: ShortcutKind) -> rusqlite::Result<String> {
    Ok(get_setting(conn, kind.setting_key())?
        .and_then(|text| canonical_accelerator(&text).ok())
        .unwrap_or_else(|| kind.default_accelerator().to_string()))
}

fn shortcut_statuses(conn: &Connection) -> rusqlite::Result<Vec<ShortcutStatus>> {
    ShortcutKind::ALL
        .into_iter()
        .map(|kind| {
            let accelerator = stored_accelerator(conn, kind)?;
            Ok(ShortcutStatus {
                kind,
                is_default: accelerator == kind.default_accelerator(),
                accelerator,
                state: ShortcutState::Assigned,
            })
        })
        .collect()
}

impl Store {
    /// Настройки и состояние копии для окна «Настройки». Состояние сочетаний здесь — «назначено»: занятость определяет
    /// система при регистрации, её накладывает слой Tauri.
    pub fn get_settings(&self) -> Result<Settings, StoreError> {
        let default = self.inner.default_mirror_dir.clone();
        let (text_size, folder, pending_days, shortcuts, collapsed, recent) =
            self.read(|conn| {
                let text_size = get_setting(conn, KEY_TEXT_SIZE).map_err(read_err)?;
                let folder = effective_mirror_dir(conn, default.as_deref()).map_err(read_err)?;
                let pending: i64 = conn
                    .query_row("SELECT COUNT(*) FROM mirror_dirty", [], |row| row.get(0))
                    .map_err(read_err)?;
                let shortcuts = shortcut_statuses(conn).map_err(read_err)?;
                let collapsed = get_setting(conn, KEY_SIDEBAR_COLLAPSED).map_err(read_err)?;
                let recent = recent_emoji(conn).map_err(read_err)?;
                Ok((text_size, folder, pending, shortcuts, collapsed, recent))
            })?;
        Ok(Settings {
            sidebar_collapsed: collapsed.as_deref() == Some("1"),
            recent_emoji: recent,
            backup: self.inner.backup.status(&self.inner.db_path),
            text_size: text_size
                .as_deref()
                .map_or(TextSize::Normal, TextSize::from_setting),
            mirror: MirrorStatus {
                folder: folder.0.map(|path| path.to_string_lossy().into_owned()),
                is_default: folder.1,
                error: self.inner.mirror.error(),
                pending_days,
            },
            shortcuts,
        })
    }

    /// Проверка нового сочетания до регистрации в системе: запись разбирается и приводится к каноничной, зарезервированное
    /// и без модификатора отвергается (`ShortcutInvalid`), совпадение с сочетанием другого действия — `ShortcutDuplicate`.
    /// Ничего не записывает.
    pub fn check_shortcut(
        &self,
        kind: ShortcutKind,
        accelerator: &str,
    ) -> Result<String, StoreError> {
        let canonical =
            canonical_accelerator(accelerator).map_err(|_| StoreError::ShortcutInvalid)?;
        let other = self.read(|conn| {
            ShortcutKind::ALL
                .into_iter()
                .filter(|other| *other != kind)
                .map(|other| stored_accelerator(conn, other).map_err(read_err))
                .collect::<Result<Vec<_>, _>>()
        })?;
        if other.contains(&canonical) {
            return Err(StoreError::ShortcutDuplicate);
        }
        Ok(canonical)
    }

    /// Сочетание вида `kind` как есть сейчас (сохранённое или по умолчанию).
    pub fn shortcut_accelerator(&self, kind: ShortcutKind) -> Result<String, StoreError> {
        self.read(|conn| stored_accelerator(conn, kind).map_err(read_err))
    }

    /// Сохраняет сочетание, уже принятое системой. Повторяет проверку [`Store::check_shortcut`].
    pub fn save_shortcut(&self, kind: ShortcutKind, accelerator: &str) -> Result<(), StoreError> {
        let canonical = self.check_shortcut(kind, accelerator)?;
        self.write(move |conn| {
            conn.execute(
                "INSERT INTO app_setting(key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                (kind.setting_key(), canonical),
            )
            .map_err(write_err)?;
            Ok(())
        })
    }

    /// «Сбросить»: возвращает сочетание по умолчанию.
    pub fn reset_shortcut(&self, kind: ShortcutKind) -> Result<(), StoreError> {
        self.write(move |conn| {
            conn.execute(
                "DELETE FROM app_setting WHERE key = ?1",
                [kind.setting_key()],
            )
            .map_err(write_err)?;
            Ok(())
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

    /// Сайдбар свёрнут до полосы эмодзи или развёрнут: хранится, переживает перезапуск.
    pub fn set_sidebar_collapsed(&self, collapsed: bool) -> Result<(), StoreError> {
        self.write(move |conn| {
            conn.execute(
                "INSERT INTO app_setting(key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                (KEY_SIDEBAR_COLLAPSED, if collapsed { "1" } else { "0" }),
            )
            .map_err(write_err)?;
            Ok(())
        })
    }

    /// Выбранный в пикере эмодзи встаёт первым в «недавних»; повтор переезжает вперёд, список ограничен
    /// [`MAX_RECENT_EMOJI`]. Ответ — список целиком. Чтение и запись — в одной транзакции писателя.
    pub fn add_recent_emoji(&self, emoji: &str) -> Result<Vec<String>, StoreError> {
        // Пустое значение — не эмодзи, а не «эмодзи по умолчанию».
        if emoji.trim().is_empty() {
            return Err(kinige_domain::chat::ChatInputError::IconInvalid.into());
        }
        let emoji = kinige_domain::chat::normalize_icon(Some(emoji))?;
        self.write(move |conn| {
            let tx = conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(write_err)?;
            let mut list = recent_emoji(&tx).map_err(read_err)?;
            list.retain(|known| *known != emoji);
            list.insert(0, emoji);
            list.truncate(MAX_RECENT_EMOJI);
            let text = serde_json::to_string(&list)
                .map_err(|error| StoreError::Internal(error.to_string()))?;
            tx.execute(
                "INSERT INTO app_setting(key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                (KEY_RECENT_EMOJI, text),
            )
            .map_err(write_err)?;
            tx.commit().map_err(write_err)?;
            Ok(list)
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
