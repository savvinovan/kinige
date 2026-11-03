//! Settings (`app_setting`, not synchronised): the Markdown copy folder and the text size (ADR-0004).

use crate::error::{read_err, write_err};
use crate::forest::stored_view;
use crate::send::mark_all_dirty;
use crate::{Store, StoreError};
use kinige_domain::emoji::emoji_identity;
use kinige_domain::shortcuts::canonical_accelerator;
use kinige_domain::{
    MirrorStatus, Settings, ShortcutKind, ShortcutState, ShortcutStatus, TextSize,
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub(crate) const KEY_MIRROR_DIR: &str = "mirror_dir";
pub(crate) const KEY_TEXT_SIZE: &str = "text_size";
const KEY_SIDEBAR_COLLAPSED: &str = "sidebar_collapsed";
const KEY_RECENT_EMOJI: &str = "recent_emoji";
/// How many recent emoji the picker remembers.
pub const MAX_RECENT_EMOJI: usize = 24;

/// Recent emoji, the freshest first. A corrupted value is an empty list: recents are not worth an error. Entries that
/// differ only by `VS16` (the old `✅\u{FE0F}` and the new `✅` from `emojibase-data` 18) count as one emoji: the
/// freshest one stays.
fn recent_emoji(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let stored = get_setting(conn, KEY_RECENT_EMOJI)?
        .and_then(|text| serde_json::from_str::<Vec<String>>(&text).ok())
        .unwrap_or_default();
    let mut seen = HashSet::new();
    Ok(stored
        .into_iter()
        .filter(|emoji| seen.insert(emoji_identity(emoji)))
        .collect())
}

pub(crate) fn get_setting(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT value FROM app_setting WHERE key = ?1",
        [key],
        |row| row.get(0),
    )
    .optional()
}

/// The copy folder: chosen by the user or the default. `None` means the copy is off. The second value is "is default".
pub(crate) fn effective_mirror_dir(
    conn: &Connection,
    default: Option<&Path>,
) -> rusqlite::Result<(Option<PathBuf>, bool)> {
    Ok(match get_setting(conn, KEY_MIRROR_DIR)? {
        Some(chosen) => (Some(PathBuf::from(chosen)), false),
        None => (default.map(Path::to_path_buf), true),
    })
}

/// The stored shortcut of kind `kind`; no record, an unknown or a corrupted value gives the default shortcut.
pub(crate) fn stored_accelerator(
    conn: &Connection,
    kind: ShortcutKind,
) -> rusqlite::Result<String> {
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
    /// The settings and the copy state for the Settings window. The shortcut state here is "assigned": whether a
    /// shortcut is taken is decided by the system at registration, and the Tauri layer overlays it.
    pub fn get_settings(&self) -> Result<Settings, StoreError> {
        let default = self.inner.default_mirror_dir.clone();
        let (text_size, folder, pending_days, shortcuts, collapsed, recent, forest) =
            self.read(|conn| {
                let text_size = get_setting(conn, KEY_TEXT_SIZE).map_err(read_err)?;
                let folder = effective_mirror_dir(conn, default.as_deref()).map_err(read_err)?;
                let pending: i64 = conn
                    .query_row("SELECT COUNT(*) FROM mirror_dirty", [], |row| row.get(0))
                    .map_err(read_err)?;
                let shortcuts = shortcut_statuses(conn).map_err(read_err)?;
                let collapsed = get_setting(conn, KEY_SIDEBAR_COLLAPSED).map_err(read_err)?;
                let recent = recent_emoji(conn).map_err(read_err)?;
                let forest = stored_view(conn).map_err(read_err)?;
                Ok((
                    text_size, folder, pending, shortcuts, collapsed, recent, forest,
                ))
            })?;
        Ok(Settings {
            forest_view: forest.0,
            forest_mode: forest.1,
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

    /// Checks a new shortcut before registering it in the system: the text is parsed and made canonical, a reserved one
    /// or one without a modifier is rejected (`ShortcutInvalid`), a clash with the shortcut of another action gives
    /// `ShortcutDuplicate`. Writes nothing.
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

    /// The shortcut of kind `kind` as it is now (stored or the default).
    pub fn shortcut_accelerator(&self, kind: ShortcutKind) -> Result<String, StoreError> {
        self.read(|conn| stored_accelerator(conn, kind).map_err(read_err))
    }

    /// Saves a shortcut the system has already accepted. Repeats the check of [`Store::check_shortcut`].
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

    /// "Reset": returns the default shortcut.
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

    /// "Text size": stored, survives a restart.
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

    /// The sidebar is collapsed to an emoji strip or expanded: stored, survives a restart.
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

    /// An emoji chosen in the picker goes first in "recent"; a repeat moves to the front, the list is capped at
    /// [`MAX_RECENT_EMOJI`]. The reply is the whole list. Reading and writing happen in one writer transaction.
    pub fn add_recent_emoji(&self, emoji: &str) -> Result<Vec<String>, StoreError> {
        // Only one emoji sequence (no spaces, markup or text): the "recent" list is shown in the picker of both
        // windows. The capture window gets this command separately, so the check is here, not in the picker.
        if !kinige_domain::emoji::is_emoji_sequence(emoji) {
            return Err(kinige_domain::chat::ChatInputError::IconInvalid.into());
        }
        let emoji = emoji.to_string();
        let identity = emoji_identity(&emoji);
        self.write(move |conn| {
            let tx = conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(write_err)?;
            let mut list = recent_emoji(&tx).map_err(read_err)?;
            list.retain(|known| emoji_identity(known) != identity);
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

    /// A new copy folder (the path came from the system dialog in Rust). In the same transaction all days with entries
    /// are marked for rewriting: the copy in the new folder comes out complete, not only with new entries.
    pub fn set_mirror_dir(&self, dir: &Path) -> Result<(), StoreError> {
        let text = dir
            .to_str()
            .ok_or_else(|| StoreError::Internal("path is not UTF-8".into()))?
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
        self.mirror_dir_changed();
        Ok(())
    }

    /// The copy folder changed (by the window or the settings file): the previous copy error is cleared, the writer wakes
    /// up and writes the days into the new folder at once, not after the next entry or a restart.
    pub(crate) fn mirror_dir_changed(&self) {
        self.inner.mirror.clear_error();
        self.wake_mirror();
    }
}
