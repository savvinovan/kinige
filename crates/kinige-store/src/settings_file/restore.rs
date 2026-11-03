//! Restoring from a backup and the settings file (ADR-0010).

use super::apply::{KEY_FILE_HASH, check_folder};
use super::io::{self, ReadOutcome};
use super::parse::{Field, parse};
use crate::agent::{KEY_CHATS, KEY_ENABLED};
use crate::agent_settings::put;
use rusqlite::Connection;
use std::path::{Path, PathBuf};

/// The Markdown copy folder from `settings.json` for restoring: read only. If the file is unsafe, not parsed or the
/// value is unusable, the result is `None` (the file would not have applied such a folder anyway). `null` in the file
/// means the default folder (`default_mirror`): after the file is applied the copy will go there, so it too must be
/// moved aside if it holds days that appeared after the snapshot.
pub(crate) fn mirror_folder_from_file(
    data_dir: &Path,
    default_mirror: Option<&Path>,
) -> Option<PathBuf> {
    let ReadOutcome::Content {
        bytes,
        unsafe_reason: None,
        ..
    } = io::read_file(&io::file_path(data_dir), data_dir)
    else {
        return None;
    };
    match parse(&bytes).ok()?.mirror_folder {
        Field::Value(path) => check_folder(&path, data_dir).ok(),
        Field::Default => default_mirror.map(Path::to_path_buf),
        Field::Absent => None,
    }
}

/// In the restore transaction: agent access is off, there are no visible chats (the mode stays from the backup), and
/// there is no file hash.
pub(crate) fn reset_access_after_restore(conn: &Connection) -> rusqlite::Result<()> {
    put(conn, KEY_ENABLED, "0")?;
    put(conn, KEY_CHATS, "[]")?;
    conn.execute("DELETE FROM app_setting WHERE key = ?1", [KEY_FILE_HASH])?;
    Ok(())
}
