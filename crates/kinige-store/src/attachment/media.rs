//! What the `kinige-media` protocol serves (ADR-0014): the thumbnail of an image (built on first use by a pool of two threads
//! and cached in `thumbs/`) and the file itself for the lightbox. The Tauri layer checks the window; this module answers
//! only by attachment id, never by a path.

use super::decode::make_thumbnail;
use super::{Dirs, make_dir};
use crate::error::read_err;
use crate::{Store, StoreError};
use kinige_domain::attachment::{AttachmentKind, ImageFormat, MAX_FULL_BYTES};
use kinige_domain::hlc::is_uuid_v4;
use rusqlite::OptionalExtension;
use std::fs;
use std::io::Write;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex, PoisonError, mpsc};
use std::thread;

/// How many threads build thumbnails and read files for the protocol.
const WORKERS: usize = 2;

/// Which file of an attachment is wanted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaVariant {
    /// The thumbnail, at most 960 x 960.
    Thumb,
    /// The file itself: an image of a format we can show, up to 64 MiB.
    Full,
}

/// A body for the protocol response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Media {
    /// The MIME type of the bytes: from the detected format, never from the file name.
    pub content_type: &'static str,
    pub bytes: Vec<u8>,
}

type Job = Box<dyn FnOnce() + Send>;

/// The threads behind [`Store::media_async`].
pub(crate) struct MediaPool {
    jobs: mpsc::Sender<Job>,
}

impl MediaPool {
    pub(crate) fn spawn() -> Result<MediaPool, StoreError> {
        let (jobs, queue) = mpsc::channel::<Job>();
        let queue = Arc::new(Mutex::new(queue));
        for n in 0..WORKERS {
            let queue = queue.clone();
            thread::Builder::new()
                .name(format!("kinige-media-{n}"))
                .spawn(move || {
                    loop {
                        let next = queue
                            .lock()
                            .unwrap_or_else(PoisonError::into_inner)
                            .recv();
                        let Ok(job) = next else { return };
                        // A panic in a codec is one lost thumbnail, not a dead worker.
                        let _ = catch_unwind(AssertUnwindSafe(job));
                    }
                })
                .map_err(|_| StoreError::Internal("could not start the media threads".into()))?;
        }
        Ok(MediaPool { jobs })
    }
}

/// What the protocol needs to know about an attachment.
struct Found {
    kind: AttachmentKind,
    mime: String,
    byte_size: i64,
    file_name: String,
}

impl Store {
    /// The bytes for `kinige-media://localhost/{thumb|full}/<id>`; `None` is 404: a bad or purged id, a file that is gone, a
    /// kind or size that variant does not serve, an image that does not decode. Blocking: use [`Store::media_async`] from the
    /// protocol handler.
    pub fn media(&self, id: &str, variant: MediaVariant) -> Option<Media> {
        if !is_uuid_v4(id) {
            return None;
        }
        let found = self.read(|conn| find(conn, id)).ok().flatten()?;
        let dirs = self.dirs();
        match variant {
            MediaVariant::Full => full(&dirs, &found),
            MediaVariant::Thumb => self.thumb(&dirs, id, &found),
        }
    }

    /// Runs [`Store::media`] on the pool of two threads and hands the result to `done` (which may run on any of them).
    pub fn media_async(
        &self,
        id: String,
        variant: MediaVariant,
        done: impl FnOnce(Option<Media>) + Send + 'static,
    ) {
        let store = self.clone();
        let job: Job = Box::new(move || done(store.media(&id, variant)));
        if self.inner.media_pool.jobs.send(job).is_err() {
            log::error!("media: the pool has stopped");
        }
    }

    fn thumb(&self, dirs: &Dirs, id: &str, found: &Found) -> Option<Media> {
        if found.kind != AttachmentKind::Image {
            return None;
        }
        for (path, content_type) in dirs
            .thumb_paths(id)
            .into_iter()
            .zip(["image/jpeg", "image/png"])
        {
            if let Ok(bytes) = fs::read(&path) {
                return Some(Media {
                    content_type,
                    bytes,
                });
            }
        }
        if self.inner.thumb_failed.lock().ok()?.contains(id) {
            return None;
        }
        let source = dirs.stored(&found.file_name)?;
        let Some(thumb) = make_thumbnail(&source) else {
            // Do not decode the same file again on every render; the next launch tries again.
            if let Ok(mut failed) = self.inner.thumb_failed.lock() {
                failed.insert(id.to_string());
            }
            return None;
        };
        let content_type = if thumb.ext == "png" {
            "image/png"
        } else {
            "image/jpeg"
        };
        cache(dirs, id, thumb.ext, &thumb.bytes);
        Some(Media {
            content_type,
            bytes: thumb.bytes,
        })
    }
}

fn find(conn: &rusqlite::Connection, id: &str) -> Result<Option<Found>, StoreError> {
    let row = conn
        .query_row(
            "SELECT a.kind, a.mime, a.byte_size, l.file_name FROM attachment a \
             JOIN attachment_local l ON l.attachment_id = a.id WHERE a.id = ?1",
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
        .map_err(read_err)?;
    Ok(row.and_then(|(kind, mime, byte_size, file_name)| {
        Some(Found {
            kind: AttachmentKind::parse(&kind)?,
            mime,
            byte_size,
            file_name,
        })
    }))
}

/// The file for the lightbox: only an image whose detected format is JPEG, PNG, GIF or WebP, within the size the responder may
/// hold in memory. The type comes from the stored format, never from the extension; SVG is not among the four.
fn full(dirs: &Dirs, found: &Found) -> Option<Media> {
    let format = ImageFormat::from_mime(&found.mime)?;
    let size = u64::try_from(found.byte_size).ok()?;
    if found.kind != AttachmentKind::Image || size > MAX_FULL_BYTES {
        return None;
    }
    let path = dirs.stored(&found.file_name)?;
    // The row's size is a claim; the file is what is read, and it is checked too.
    if fs::metadata(&path).ok()?.len() > MAX_FULL_BYTES {
        return None;
    }
    let bytes = fs::read(path).ok()?;
    Some(Media {
        content_type: format.mime(),
        bytes,
    })
}

/// Writes the thumbnail through a temporary name; a failure only means it is built again next time.
fn cache(dirs: &Dirs, id: &str, ext: &str, bytes: &[u8]) {
    let dir = dirs.thumbs();
    if make_dir(&dir).is_err() {
        return;
    }
    let temp = dir.join(format!(".{id}.tmp"));
    let written = (|| {
        let mut file = super::create_private(&temp)?;
        file.write_all(bytes)?;
        fs::rename(&temp, dir.join(format!("{id}.{ext}")))
    })();
    if written.is_err() {
        let _ = fs::remove_file(&temp);
    }
}
