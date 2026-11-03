//! Opening an attachment in a system app (ADR-0014): the stored file is cloned and the clone is opened, so edits in the app never
//! touch the stored file. Only a clone whose extension is on the allowlist of document types is opened; everything else is only
//! revealed in the file manager, never run.

use super::make_dir;
use crate::error::read_err;
use crate::{Store, StoreError};
use kinige_domain::attachment::{is_openable_document, mirror_file_names};
use kinige_domain::hlc::is_uuid_v4;
use rusqlite::OptionalExtension;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// What the Tauri layer does with a clone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenTarget {
    /// The clone, under `<data>/open/<process>/<id>/`.
    pub path: PathBuf,
    /// Show the file in the file manager instead of opening it.
    pub reveal_only: bool,
}

impl Store {
    /// Clones the attachment's file for a system app. An unknown id is `AttachmentNotFound`, a missing or unreadable file
    /// `AttachmentUnreadable`.
    pub fn prepare_open(&self, attachment_id: &str) -> Result<OpenTarget, StoreError> {
        if !is_uuid_v4(attachment_id) {
            return Err(StoreError::AttachmentNotFound);
        }
        let (filename, file_name): (String, String) = self
            .read(|conn| {
                conn.query_row(
                    "SELECT a.filename, l.file_name FROM attachment a \
                     JOIN attachment_local l ON l.attachment_id = a.id WHERE a.id = ?1",
                    [attachment_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()
                .map_err(read_err)
            })?
            .ok_or(StoreError::AttachmentNotFound)?;
        let dirs = self.dirs();
        let source = dirs
            .stored(&file_name)
            .filter(|path| path.is_file())
            .ok_or(StoreError::AttachmentUnreadable)?;
        let stored_ext = file_name.rsplit('.').next().unwrap_or("bin");
        let name = clone_name(&filename, stored_ext);
        let dir = dirs.open_dir().join(attachment_id);
        make_dir(&dir).map_err(|_| StoreError::AttachmentUnreadable)?;
        let path = dir.join(&name);
        if !path.is_file() {
            clone_file(&source, &dir, &name).map_err(|_| StoreError::AttachmentUnreadable)?;
        }
        Ok(OpenTarget {
            reveal_only: !is_openable_document(extension(&name)),
            path,
        })
    }
}

/// The name of the clone: what the user calls the file, with the stored extension if the name has none of its own. The OS sees
/// this name, so the allowlist is applied to it, and a file called `README` is never opened as something else.
fn clone_name(filename: &str, stored_ext: &str) -> String {
    let safe = mirror_file_names(&[filename])
        .into_iter()
        .next()
        .unwrap_or_else(|| "file".to_string());
    if extension(&safe).eq_ignore_ascii_case(stored_ext) {
        safe
    } else if extension(&safe).is_empty() {
        format!("{safe}.{stored_ext}")
    } else {
        safe
    }
}

/// The lowercase extension of a name, without the dot; empty if none.
fn extension(name: &str) -> &str {
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => ext,
        _ => "",
    }
}

/// `fs::copy` through a temporary name and a rename, mode 0600 (a clone on APFS, std docs).
fn clone_file(source: &Path, dir: &Path, name: &str) -> io::Result<()> {
    let temp = dir.join(format!(".{name}.tmp"));
    let _ = fs::remove_file(&temp);
    // The destination must not exist: a clone (`fclonefileat`) refuses to overwrite.
    let result = fs::copy(source, &temp)
        .and_then(|_| private_mode(&temp))
        .and_then(|()| fs::rename(&temp, dir.join(name)));
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

/// Mode 0600 on Unix; nothing to do elsewhere.
fn private_mode(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(())
    }
}
