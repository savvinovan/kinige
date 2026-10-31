//! Файл настроек `settings.json` (слайс 4b, ADR-0010): файл — вход, `app_setting` — действующее состояние. Разбор без
//! паник (`parse`), безопасное чтение и атомарная запись (`io`), применение одной транзакцией (`apply`), файл из состояния
//! (`render`), поток опроса и записи (`engine`).

mod apply;
mod engine;
mod io;
mod parse;
mod reconcile;
mod render;
mod restore;

#[cfg(test)]
mod tests;

pub use apply::{LOG_CLIENT, LOG_TOOL};
pub use engine::{SettingsFile, SettingsFileOptions, ShortcutApplier, ShortcutRefusal};
pub use io::{FILE_NAME, file_path};
pub use parse::{FileError, MAX_BYTES};

pub(crate) use restore::{mirror_folder_from_file, reset_access_after_restore};
