//! Attachment files (slice 7, PRD §4.3; ADR-0014): where files live, copying them in, drafts, rows, thumbnails, the protocol
//! reads, opening in a system app and the startup sweep. The synced bytes are built in `kinige_domain::attachment`.
//!
//! Layout under the data directory: `attachments/<id>.<ext>`, `attachments/.tmp/<process>/`, `attachments/.orphans/`,
//! `thumbs/<id>.jpg|png`, `open/<process>/<id>/<name>`. `<process>` is made at startup, so the sweep never touches a live copy.

mod copy;
mod decode;
mod draft;
mod faults;
mod media;
mod open;
mod rows;
mod sweep;

pub(crate) use copy::{Hints, Source, Staged, io_error, stage};
pub use faults::AttachFault;
pub(crate) use faults::Faults;
pub(crate) use media::MediaPool;
pub use media::{Media, MediaVariant};
pub use open::OpenTarget;
pub(crate) use rows::{
    NewFile, PurgedFile, attachments_column, delete_for_purge, draft_views, insert_attachment,
    insert_attachment_at, live_count, parse_attachments, promote_draft, summary_for,
};
pub use sweep::SweepReport;

use kinige_domain::hlc::is_uuid_v4;
use std::fs;
use std::path::{Path, PathBuf};

/// Where the attachment directories are, for one process.
#[derive(Debug, Clone)]
pub(crate) struct Dirs {
    data: PathBuf,
    process: String,
}

impl Dirs {
    pub(crate) fn new(data: &Path, process: &str) -> Dirs {
        Dirs {
            data: data.to_path_buf(),
            process: process.to_string(),
        }
    }

    pub(crate) fn attachments(&self) -> PathBuf {
        self.data.join("attachments")
    }

    /// `.tmp/` of all processes; the sweep removes the directories of the other ones.
    pub(crate) fn tmp_root(&self) -> PathBuf {
        self.attachments().join(".tmp")
    }

    /// The copies in progress of this process.
    pub(crate) fn tmp(&self) -> PathBuf {
        self.tmp_root().join(&self.process)
    }

    pub(crate) fn orphans(&self) -> PathBuf {
        self.attachments().join(".orphans")
    }

    pub(crate) fn thumbs(&self) -> PathBuf {
        self.data.join("thumbs")
    }

    pub(crate) fn open_root(&self) -> PathBuf {
        self.data.join("open")
    }

    /// Clones for the system apps made by this process.
    pub(crate) fn open_dir(&self) -> PathBuf {
        self.open_root().join(&self.process)
    }

    pub(crate) fn process(&self) -> &str {
        &self.process
    }

    /// The stored file by its name from `attachment_local.file_name`. A name that is not `<uuid>.<ext>` gives `None`:
    /// no path is built from anything else.
    pub(crate) fn stored(&self, file_name: &str) -> Option<PathBuf> {
        is_stored_name(file_name).then(|| self.attachments().join(file_name))
    }

    /// The cached thumbnail files of an attachment (either extension).
    pub(crate) fn thumb_paths(&self, id: &str) -> [PathBuf; 2] {
        [
            self.thumbs().join(format!("{id}.jpg")),
            self.thumbs().join(format!("{id}.png")),
        ]
    }
}

/// `^<uuid v4>\.[a-z0-9]{1,10}$`: the only shape of a stored file name.
pub(crate) fn is_stored_name(name: &str) -> bool {
    let Some((id, ext)) = name.split_once('.') else {
        return false;
    };
    is_uuid_v4(id)
        && (1..=10).contains(&ext.len())
        && ext
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
}

/// The id of a stored file name.
pub(crate) fn stored_id(name: &str) -> Option<&str> {
    let (id, _) = name.split_once('.')?;
    is_stored_name(name).then_some(id)
}

/// Deletes the stored files, their thumbnails and their clones for the system apps: `(attachment id, stored file name)`. Best
/// effort and after the commit: what could not be deleted is finished by the sweep, because a purged id has a tombstone.
pub(crate) fn delete_stored_files(
    dirs: &Dirs,
    faults: &Faults,
    files: &[(String, Option<String>)],
) {
    if faults.delete_fails() {
        return;
    }
    for (id, file_name) in files {
        if let Some(path) = file_name.as_deref().and_then(|name| dirs.stored(name)) {
            let _ = fs::remove_file(path);
        }
        if is_uuid_v4(id) {
            for thumb in dirs.thumb_paths(id) {
                let _ = fs::remove_file(thumb);
            }
            let _ = fs::remove_dir_all(dirs.open_dir().join(id));
        }
    }
}

/// Creates a directory (and its parents) with the data directory's mode.
pub(crate) fn make_dir(path: &Path) -> std::io::Result<()> {
    crate::data_dir::create(path)
}

/// Opens a new private file (mode 0600 on Unix); it must not exist.
pub(crate) fn create_private(path: &Path) -> std::io::Result<std::fs::File> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)
}

/// The `fsync` of a directory entry after a rename; a directory cannot be opened on Windows, so errors are ignored.
pub(crate) fn sync_dir(dir: &Path) {
    if let Ok(handle) = std::fs::File::open(dir) {
        let _ = handle.sync_all();
    }
}
