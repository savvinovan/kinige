//! Stage 1 of the copy pipeline (PRD §3.2, ADR-0014), outside the writer: stream the source in 1 MiB chunks into
//! `attachments/.tmp/<process>/<id>.part`, hash and count on the way, compare the count with the size the source had, read the
//! image header, `fsync`. The writer job of stage 2 (`draft.rs`) renames the file into place and writes the row. A file is
//! never loaded into memory.

use super::decode::{ImageInfo, probe_image};
use super::{Dirs, Faults, create_private, make_dir};
use crate::StoreError;
use kinige_domain::attachment::{
    AttachmentKind, MAX_FILE_BYTES, classify, hex_lower, sanitize_filename, stored_ext,
};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{Cursor, Error as IoError, ErrorKind, Read, Write};
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// The copy chunk (ADR-0001 appendix §A.2 uses the same size for the blob stream).
const CHUNK: usize = 1024 * 1024;

/// A file copied into `.tmp/` and described, waiting for stage 2.
#[derive(Debug)]
pub(crate) struct Staged {
    /// The future attachment id (UUID v4); it names the stored file.
    pub id: String,
    pub part: PathBuf,
    /// `<id>.<ext>` -- the name inside `attachments/`.
    pub file_name: String,
    pub sha256: String,
    pub byte_size: u64,
    /// The cleaned name the user sees.
    pub filename: String,
    pub kind: AttachmentKind,
    pub mime: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

/// The source of the bytes. A path is checked before it is opened; bytes come from a paste.
pub(crate) enum Source<'a> {
    Path(&'a Path),
    Bytes(&'a [u8]),
}

/// What the caller knows about the file besides its bytes.
pub(crate) struct Hints<'a> {
    /// The name to show; for a path the file's own name is used if this is `None`.
    pub name: Option<&'a str>,
    /// A MIME type stated by the source (a Telegram export).
    pub mime: Option<&'a str>,
}

/// Copies the source into `.tmp/` and describes it. On any error nothing stays behind.
pub(crate) fn stage(
    dirs: &Dirs,
    faults: &Faults,
    source: Source<'_>,
    hints: &Hints<'_>,
) -> Result<Staged, StoreError> {
    let id = Uuid::new_v4().to_string();
    let tmp = dirs.tmp();
    make_dir(&tmp).map_err(io_error)?;
    let part = tmp.join(format!("{id}.part"));
    let outcome = copy_and_describe(faults, &source, hints, &id, &part);
    match outcome {
        Ok(staged) => Ok(staged),
        Err(error) => {
            let _ = fs::remove_file(&part);
            Err(error)
        }
    }
}

fn copy_and_describe(
    faults: &Faults,
    source: &Source<'_>,
    hints: &Hints<'_>,
    id: &str,
    part: &Path,
) -> Result<Staged, StoreError> {
    let (name, copied) = match source {
        Source::Path(path) => {
            let own = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            (
                hints.name.map_or(own, str::to_string),
                copy_path(faults, path, part)?,
            )
        }
        Source::Bytes(bytes) => {
            let size = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
            if size > MAX_FILE_BYTES {
                return Err(StoreError::AttachmentTooLarge);
            }
            let mut cursor = Cursor::new(*bytes);
            (
                hints.name.unwrap_or_default().to_string(),
                stream(faults, &mut cursor, part, None)?,
            )
        }
    };
    let mut filename = sanitize_filename(&name);
    let image = probe_image(part);
    let ext = stored_ext(&filename, image.as_ref().map(|i| i.format));
    // A paste has no extension of its own (`pasted-HHMMSS`, or WebKit's bare hint): give it the detected one, so the
    // Markdown copy and other apps can tell what the file is.
    if matches!(source, Source::Bytes(_)) && ext != stored_ext(&filename, None) {
        filename = sanitize_filename(&format!("{filename}.{ext}"));
    }
    let (kind, mime) = classify(&ext, hints.mime, image.as_ref().map(|info| info.format));
    let (width, height) = match (&image, kind) {
        (Some(ImageInfo { width, height, .. }), AttachmentKind::Image) => {
            (Some(*width), Some(*height))
        }
        _ => (None, None),
    };
    Ok(Staged {
        id: id.to_string(),
        part: part.to_path_buf(),
        file_name: format!("{id}.{ext}"),
        sha256: copied.sha256,
        byte_size: copied.bytes,
        filename,
        kind,
        mime,
        width,
        height,
    })
}

/// A path is a regular file (not a symlink, FIFO, device or folder -- checked with `symlink_metadata` before anything is
/// opened), not larger than the limit, and the same size when the copy ends.
fn copy_path(faults: &Faults, path: &Path, part: &Path) -> Result<Copied, StoreError> {
    let meta = fs::symlink_metadata(path).map_err(|_| StoreError::AttachmentUnreadable)?;
    if !meta.is_file() {
        return Err(StoreError::AttachmentUnreadable);
    }
    if meta.len() > MAX_FILE_BYTES {
        return Err(StoreError::AttachmentTooLarge);
    }
    let mut file = open_regular(path)?;
    let copied = stream(faults, &mut file, part, Some(meta.len()))?;
    // A source that changed while it was copied is not a faithful copy.
    let after = file
        .metadata()
        .map_err(|_| StoreError::AttachmentUnreadable)?;
    if after.len() != meta.len() || copied.bytes != meta.len() {
        return Err(StoreError::AttachmentUnreadable);
    }
    Ok(copied)
}

/// Opens the source for reading and checks that what was opened is a regular file. The `symlink_metadata` check above and this open
/// are two steps, so a path swapped for a symlink or a FIFO in between must not be followed or waited on: on Unix the open refuses
/// a final symlink (`O_NOFOLLOW`) and does not block on a FIFO (`O_NONBLOCK`), and the opened handle is checked again.
fn open_regular(path: &Path) -> Result<File, StoreError> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    let file = options
        .open(path)
        .map_err(|_| StoreError::AttachmentUnreadable)?;
    match file.metadata() {
        Ok(meta) if meta.is_file() => Ok(file),
        _ => Err(StoreError::AttachmentUnreadable),
    }
}

struct Copied {
    sha256: String,
    bytes: u64,
}

/// Reads to the end in chunks, writes `part`, hashes, counts, and `fsync`s. `expected` is the size the source had at the start.
fn stream(
    faults: &Faults,
    reader: &mut dyn Read,
    part: &Path,
    expected: Option<u64>,
) -> Result<Copied, StoreError> {
    let mut out = create_private(part).map_err(io_error)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; CHUNK];
    let mut total: u64 = 0;
    let mut first = true;
    loop {
        let read = match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => read,
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(_) => return Err(StoreError::AttachmentUnreadable),
        };
        let chunk = buffer.get(..read).unwrap_or_default();
        total = total.saturating_add(u64::try_from(read).unwrap_or(u64::MAX));
        if total > MAX_FILE_BYTES {
            return Err(StoreError::AttachmentTooLarge);
        }
        if faults.disk_full(total) {
            return Err(StoreError::DiskFull);
        }
        hasher.update(chunk);
        out.write_all(chunk).map_err(io_error)?;
        if first {
            first = false;
            faults.run_mid_copy();
        }
        // The source grew past its size at the start: stop now instead of copying a file that keeps changing.
        if expected.is_some_and(|size| total > size) {
            return Err(StoreError::AttachmentUnreadable);
        }
    }
    out.sync_all().map_err(io_error)?;
    Ok(Copied {
        sha256: hex_lower(hasher.finalize().as_slice()),
        bytes: total,
    })
}

/// "No space left on the device" is `DiskFull`; any other write error is an unreadable file or a failed write.
pub(crate) fn io_error(error: IoError) -> StoreError {
    if error.kind() == ErrorKind::StorageFull {
        StoreError::DiskFull
    } else {
        StoreError::Write("file operation failed".into())
    }
}
