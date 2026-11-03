//! The settings file `settings.json` (slice 4b, ADR-0010): the file is the input, `app_setting` is the effective state.
//! Panic-free parsing (`parse`), safe reading and atomic writing (`io`), applying in one transaction (`apply`), the file
//! from the state (`render`), the polling and writing thread (`engine`).

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
