//! Media of a Telegram import (slice 7, ADR-0014): resolving a path from the export without trusting it, copying the file in,
//! and attaching it to a new entry or to an entry imported earlier. The files are copied outside the writer (stage 1 of the
//! pipeline); the writer transaction of the batch renames them into place and writes the rows (stage 2).

use crate::attachment::{
    Dirs, Faults, Hints, NewFile, Source, Staged, insert_attachment, insert_attachment_at,
    make_dir, stage,
};
use crate::message_body::replace_body_without_revision;
use crate::messages::load;
use crate::send::mark_dirty;
use crate::{Now, StoreError};
use kinige_domain::telegram::TgFile;
use rusqlite::Transaction;
use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

/// What a path from the export turned out to be.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum MediaPath {
    /// A regular file inside the export folder (canonical path).
    Found(PathBuf),
    /// Nothing is there: the export promised a file it does not hold.
    Missing,
    /// Not allowed: absolute, `..`, a drive letter, a backslash, a NUL, outside the folder after resolving links, or not a regular file.
    Bad,
}

/// Resolves `relative` (as written in `result.json`) against `root`, the canonical folder of the export. Nothing is opened:
/// `canonicalize` and `symlink_metadata` only look, so a FIFO or a device is never read (it would block).
pub(crate) fn media_path(root: &Path, relative: &str) -> MediaPath {
    if relative.is_empty()
        || relative.contains(['\0', '\\'])
        || relative.as_bytes().get(1) == Some(&b':')
    {
        return MediaPath::Bad;
    }
    let clean = Path::new(relative)
        .components()
        .all(|part| matches!(part, Component::Normal(_) | Component::CurDir));
    if !clean {
        return MediaPath::Bad;
    }
    let resolved = match fs::canonicalize(root.join(relative)) {
        Ok(path) => path,
        Err(error) if error.kind() == ErrorKind::NotFound => return MediaPath::Missing,
        Err(_) => return MediaPath::Bad,
    };
    if !resolved.starts_with(root) {
        return MediaPath::Bad;
    }
    match fs::symlink_metadata(&resolved) {
        Ok(meta) if meta.is_file() => MediaPath::Found(resolved),
        Ok(_) => MediaPath::Bad,
        Err(error) if error.kind() == ErrorKind::NotFound => MediaPath::Missing,
        Err(_) => MediaPath::Bad,
    }
}

/// A file copied into `.tmp/`, waiting for the batch transaction.
pub(crate) struct StagedMedia {
    pub staged: Staged,
    pub duration_ms: Option<i64>,
}

/// What preparing the file of one message gave.
pub(crate) enum Prepared {
    /// The export holds no file for it (a note instead of a path, or the file is not there).
    Missing,
    /// A bad path, an unreadable file, a file over the limit.
    Skipped,
    /// The disk is full: the import stops.
    DiskFull,
    Ready(StagedMedia),
}

/// Resolves and copies the file of a message.
pub(crate) fn prepare(dirs: &Dirs, faults: &Faults, root: &Path, file: &TgFile) -> Prepared {
    let Some(relative) = file.path.as_deref() else {
        return Prepared::Missing;
    };
    let path = match media_path(root, relative) {
        MediaPath::Found(path) => path,
        MediaPath::Missing => return Prepared::Missing,
        MediaPath::Bad => return Prepared::Skipped,
    };
    let own_name = path.file_name().map(|n| n.to_string_lossy().into_owned());
    let name = file.name.clone().or(own_name);
    let hints = Hints {
        name: name.as_deref(),
        mime: file.mime.as_deref(),
    };
    match stage(dirs, faults, Source::Path(&path), &hints) {
        Ok(staged) => Prepared::Ready(StagedMedia {
            staged,
            duration_ms: file
                .duration_seconds
                .map(|seconds| i64::from(seconds).saturating_mul(1000)),
        }),
        Err(StoreError::DiskFull) => Prepared::DiskFull,
        Err(_) => Prepared::Skipped,
    }
}

impl StagedMedia {
    pub(crate) fn bytes(&self) -> i64 {
        i64::try_from(self.staged.byte_size).unwrap_or(i64::MAX)
    }

    /// Deletes the copy that was never attached (the batch did not happen).
    pub(crate) fn discard(&self) {
        let _ = fs::remove_file(&self.staged.part);
    }

    fn new_file(&self) -> NewFile {
        let staged = &self.staged;
        NewFile {
            id: staged.id.clone(),
            file_name: staged.file_name.clone(),
            sha256: staged.sha256.clone(),
            byte_size: self.bytes(),
            filename: staged.filename.clone(),
            kind: staged.kind,
            mime: staged.mime.clone(),
            width: staged.width,
            height: staged.height,
            duration_ms: self.duration_ms,
        }
    }

    /// Stage 2: the copy goes into `attachments/` under its final name.
    fn place(&self, dirs: &Dirs) -> Result<(), StoreError> {
        let target = dirs
            .stored(&self.staged.file_name)
            .ok_or_else(|| StoreError::Internal("bad stored file name".into()))?;
        make_dir(&dirs.attachments()).map_err(crate::attachment::io_error)?;
        fs::rename(&self.staged.part, target).map_err(crate::attachment::io_error)
    }
}

/// The file of a new entry, in the batch transaction: placed, and its rows written under `hlc`.
pub(crate) fn attach_to_new(
    tx: &Transaction<'_>,
    dirs: &Dirs,
    message_id: &str,
    media: &StagedMedia,
    hlc: &str,
) -> Result<(), StoreError> {
    media.place(dirs)?;
    insert_attachment_at(tx, hlc, message_id, 0, &media.new_file())?;
    Ok(())
}

/// A file for an entry imported before the files were (slice 7): the entry is found through `message_ext`.
pub(crate) struct Late {
    pub message_id: String,
    pub media: StagedMedia,
    /// For each language (Russian, English): the text the import wrote with its placeholder, and the same text without it.
    pub renderings: Vec<(String, String)>,
}

/// What attaching a late file did.
pub(crate) enum LateOutcome {
    /// Attached; the placeholder line was removed when the entry was unedited since import.
    Attached { bytes: i64 },
    /// The entry already has attachments (live or in the trash) or no longer exists: nothing is attached.
    Skipped,
}

/// In the batch transaction: attaches the file to an entry that has none, and removes the placeholder line if the entry is
/// **unedited since import**: `hlc(body) == hlc(core)` (the import wrote all groups under one HLC and every rewrite bumps `body`)
/// and the text is exactly the import's rendering in one of the two languages. The rewrite is the usual one without the `edit`
/// revision (ADR-0014, appendix §C.6).
pub(crate) fn attach_late(
    tx: &Transaction<'_>,
    dirs: &Dirs,
    late: &Late,
    now: &Now,
) -> Result<LateOutcome, StoreError> {
    let has_files: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM attachment WHERE message_id = ?1)",
            [&late.message_id],
            |row| row.get(0),
        )
        .map_err(crate::error::write_err)?;
    let loaded = match load(tx, &late.message_id) {
        Ok(loaded) => loaded,
        Err(StoreError::MessageNotFound) => {
            late.media.discard();
            return Ok(LateOutcome::Skipped);
        }
        Err(error) => return Err(error),
    };
    if has_files || loaded.ensure_writable().is_err() {
        late.media.discard();
        return Ok(LateOutcome::Skipped);
    }
    late.media.place(dirs)?;
    insert_attachment(tx, &late.message_id, 0, &late.media.new_file(), now.ms)?;
    let rewrite = untouched_since_import(&loaded.hlc_json).then(|| {
        late.renderings
            .iter()
            .find(|(placeholder, _)| *placeholder == loaded.columns.body_md)
            .map(|(_, without)| without.as_str())
    });
    match rewrite.flatten() {
        Some(without) => replace_body_without_revision(tx, &loaded, without, now)?,
        None => {
            crate::search::reindex(tx, &late.message_id, &loaded.columns.body_md)
                .map_err(crate::error::write_err)?;
            mark_dirty(tx, &loaded.columns.chat_id, &loaded.columns.journal_date)
                .map_err(crate::error::write_err)?;
        }
    }
    Ok(LateOutcome::Attached {
        bytes: late.media.bytes(),
    })
}

/// `hlc(body) == hlc(core)`: nothing has rewritten the text since the import wrote every group under one HLC (ADR-0001 §C.3).
fn untouched_since_import(hlc_json: &str) -> bool {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(hlc_json) else {
        return false;
    };
    let stamp = |group: &str| value.get(group)?.get("h")?.as_str().map(str::to_string);
    matches!((stamp("core"), stamp("body")), (Some(core), Some(body)) if core == body)
}
