//! The file behind `kinige-media://localhost/audio/<id>` (slice 7a, ADR-0015): an open handle to a stored audio file that passed
//! every check the route needs, and nothing else. The Tauri layer decides the `Range` arithmetic and reads the bytes; this module
//! answers only by attachment id, never by a path.

use super::media::Job;
use crate::error::read_err;
use crate::{Store, StoreError};
use kinige_domain::attachment::{
    AttachmentKind, MAX_FULL_BYTES, audio_head_matches, playable_audio_mime,
};
use kinige_domain::hlc::is_uuid_v4;
use rusqlite::OptionalExtension;
use std::fs::File;
use std::io::Read;

/// How many leading bytes decide whether the file is what its row says.
const HEAD_BYTES: usize = 12;

/// An audio file the route may serve.
#[derive(Debug)]
pub struct AudioFile {
    /// Opened for reading, positioned at the start; seek before reading a range.
    pub file: File,
    /// The real length of the file (not the row's claim), 1 to 64 MiB.
    pub len: u64,
    /// From the allowlist, never from the file name.
    pub content_type: &'static str,
}

impl Store {
    /// The file for `/audio/<id>`; `None` is 404: a bad or purged id, a row that is not `audio` or is `missing`, a MIME type off the
    /// allowlist, a row or a file over 64 MiB (the responder holds a whole body in memory), an empty or unreadable file, or first
    /// bytes that do not fit the claimed container. Blocking: use [`Store::audio_async`] from the protocol handler.
    pub fn audio_file(&self, id: &str) -> Option<AudioFile> {
        if !is_uuid_v4(id) {
            return None;
        }
        let (mime, byte_size, file_name) = self.read(|conn| find(conn, id)).ok().flatten()?;
        let content_type = playable_audio_mime(&mime)?;
        if u64::try_from(byte_size).ok()? > MAX_FULL_BYTES {
            return None;
        }
        let path = self.dirs().stored(&file_name)?;
        let mut file = File::open(path).ok()?;
        let meta = file.metadata().ok()?;
        // The row's size is a claim; the file is what is read, and it is checked too.
        if !meta.is_file() || meta.len() == 0 || meta.len() > MAX_FULL_BYTES {
            return None;
        }
        let mut head = [0u8; HEAD_BYTES];
        let read = read_head(&mut file, &mut head)?;
        if !audio_head_matches(content_type, head.get(..read)?) {
            return None;
        }
        Some(AudioFile {
            file,
            len: meta.len(),
            content_type,
        })
    }

    /// Runs [`Store::audio_file`] on the media pool and hands the result to `done` (which may run on any of its threads, so it may
    /// read the file).
    pub fn audio_async(&self, id: String, done: impl FnOnce(Option<AudioFile>) + Send + 'static) {
        let store = self.clone();
        let job: Job = Box::new(move || done(store.audio_file(&id)));
        if !self.inner.media_pool.submit(job) {
            log::error!("media: the pool has stopped");
        }
    }
}

/// `(mime, byte_size, stored file name)` of a live, local `audio` attachment of an entry. A tile above the input field is not served.
fn find(
    conn: &rusqlite::Connection,
    id: &str,
) -> Result<Option<(String, i64, String)>, StoreError> {
    conn.query_row(
        "SELECT a.kind, a.mime, a.byte_size, l.file_name FROM attachment a \
         JOIN attachment_local l ON l.attachment_id = a.id \
         WHERE a.id = ?1 AND l.state = 'local'",
        [id],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
            ))
        },
    )
    .optional()
    .map_err(read_err)
    .map(|found| {
        found.and_then(|(kind, mime, byte_size, file_name)| {
            (AttachmentKind::parse(&kind) == Some(AttachmentKind::Audio))
                .then_some((mime, byte_size, file_name))
        })
    })
}

/// Reads up to `head.len()` bytes from the start; fewer only at the end of the file.
fn read_head(file: &mut File, head: &mut [u8]) -> Option<usize> {
    let mut filled = 0;
    while filled < head.len() {
        let read = file.read(head.get_mut(filled..)?).ok()?;
        if read == 0 {
            break;
        }
        filled = filled.checked_add(read)?;
    }
    Some(filled)
}
