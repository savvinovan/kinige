//! Where the data lives: the database and the Markdown copy folder.

use std::path::PathBuf;
use tauri::{AppHandle, Manager, Runtime};

/// Where the data lives. `KINIGE_DATA_DIR` and `KINIGE_MIRROR_DIR` are for checks on a clean machine and for autotests; a regular
/// launch does not set them: the database is in the app data directory, the copy is in `~/Documents/Kinige`.
pub(crate) fn db_path<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let data_dir = match std::env::var_os("KINIGE_DATA_DIR") {
        Some(dir) => PathBuf::from(dir),
        None => default_data_dir(app)?,
    };
    Ok(data_dir.join("kinige.db"))
}

/// The default data directory. On Windows it is the local one (`%LOCALAPPDATA%\me.savvinov.kinige`): a SQLite database
/// with its journal and WAL does not belong in the roaming profile (`%APPDATA%`), which is synced and copied around.
/// Elsewhere it is the app data directory as before (macOS: `~/Library/Application Support/me.savvinov.kinige`).
fn default_data_dir<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, tauri::Error> {
    if cfg!(windows) {
        app.path().app_local_data_dir()
    } else {
        app.path().app_data_dir()
    }
}

/// The default folder of the Markdown copy: `~/Documents/Kinige` (or `KINIGE_MIRROR_DIR`).
pub(crate) fn default_mirror_dir<R: Runtime>(app: &AppHandle<R>) -> Option<PathBuf> {
    match std::env::var_os("KINIGE_MIRROR_DIR") {
        Some(dir) => Some(PathBuf::from(dir)),
        None => app
            .path()
            .document_dir()
            .ok()
            .map(|documents| documents.join("Kinige")),
    }
}
