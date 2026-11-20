//! A drop on the main window (slice 7, KINIG-133): the window events, the one-time tokens and the image of a drag that carried no file.
//! The commands that claim a token are in `attachments.rs`. Paths and bytes never reach the frontend: it gets a token.
//!
//! Tauri's drop event lists only file paths. When a drag enters with none, and a chat is open to receive it, the image the drag carries
//! (if any) is read at once, while the app that started the drag still answers, and kept as the single staged image; it is used by the
//! drop that follows if the drag is still the same one (the pasteboard's generation), and is forgotten on leaving, on the next enter or
//! when it is used. Nothing is decoded here: the UI thread only copies at most 64 MiB.

use crate::capture::MAIN_LABEL;
use crate::drag_source::{DragImage, DragSource, SystemDrag, log_names};
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};
use tauri::{AppHandle, DragDropEvent, Emitter, Manager, Runtime};
use uuid::Uuid;

/// The event for the main window while files are dragged over it: `true` on entering, `false` on leaving or dropping. It shows the
/// "Drop to attach" overlay.
pub const ATTACH_DRAG: &str = "attachments-drag";
/// The event for the main window after a drop: the one-time token for `attach_dropped`.
pub const ATTACH_DROPPED: &str = "attachments-dropped";

/// How long a drop token lives.
const TOKEN_TTL: Duration = Duration::from_secs(60);
/// How many drops are remembered at once: older ones are forgotten.
const TOKEN_CAP: usize = 8;
/// How many dropped paths one drop may carry: a bigger one is cut (an entry takes ten attachments anyway).
const MAX_DROPPED_PATHS: usize = 64;

/// Dropped files (or a dropped image) waiting for the window to claim them. Only the Rust side ever sees the paths and the bytes.
#[derive(Default)]
pub struct DropTokens {
    drops: Mutex<Vec<PendingDrop>>,
    /// The image of the drag that is over the window now.
    staged: Mutex<Option<Staged>>,
    /// Whether the window has a chat open to receive an attachment (the web view says so, `set_drop_target`). Without one, no image
    /// is read from a drag at all.
    target: Mutex<DropTarget>,
}

/// The newest answer of the web view and the number of the call that gave it.
#[derive(Default)]
struct DropTarget {
    seq: u64,
    active: bool,
}

struct PendingDrop {
    token: String,
    dropped: Dropped,
    at: Instant,
}

struct Staged {
    /// The drag pasteboard's change count when the image was read.
    generation: i64,
    image: DragImage,
}

/// What a drop carried: files, or, when it carried none, maybe an image. Neither: nothing the app can use.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Dropped {
    pub paths: Vec<PathBuf>,
    pub image: Option<DragImage>,
}

impl DropTokens {
    fn lock(&self) -> MutexGuard<'_, Vec<PendingDrop>> {
        self.drops.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn staged(&self) -> MutexGuard<'_, Option<Staged>> {
        self.staged.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Whether a chat is open to receive a drop.
    pub fn accepting(&self) -> bool {
        self.target().active
    }

    /// The web view says whether a chat is open, with the number of its call (they only grow). A call older than the newest one seen
    /// is ignored, so two calls that arrive out of order cannot leave a stale answer. Closing a chat forgets a staged image.
    pub fn set_accepting(&self, active: bool, seq: u64) {
        let mut target = self.target();
        if seq <= target.seq {
            return;
        }
        *target = DropTarget { seq, active };
        drop(target);
        if !active {
            *self.staged() = None;
        }
    }

    fn target(&self) -> MutexGuard<'_, DropTarget> {
        self.target.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Remembers the paths of a drop and returns its token (a UUID v4).
    pub fn put(&self, paths: Vec<PathBuf>) -> String {
        self.put_drop(Dropped { paths, image: None })
    }

    /// Remembers a drop (files, an image or nothing) and returns its token. Only the newest drop keeps an image: an unclaimed one
    /// cannot pile up 64 MiB each.
    pub fn put_drop(&self, dropped: Dropped) -> String {
        let token = Uuid::new_v4().to_string();
        let mut drops = self.lock();
        drops.retain(|drop| drop.at.elapsed() < TOKEN_TTL);
        while drops.len() >= TOKEN_CAP {
            drops.remove(0);
        }
        let mut dropped = dropped;
        dropped.paths.truncate(MAX_DROPPED_PATHS);
        if dropped.image.is_some() {
            for older in drops.iter_mut() {
                older.dropped.image = None;
            }
        }
        drops.push(PendingDrop {
            token: token.clone(),
            dropped,
            at: Instant::now(),
        });
        token
    }

    /// What a drop carried, once: the token is spent. An unknown, spent or expired token gives `None`.
    pub fn take(&self, token: &str) -> Option<Dropped> {
        let mut drops = self.lock();
        let index = drops.iter().position(|drop| drop.token == token)?;
        let drop = drops.remove(index);
        (drop.at.elapsed() < TOKEN_TTL).then_some(drop.dropped)
    }

    /// Forgets the drops nobody claimed in time (and the bytes they hold).
    fn expire(&self) {
        self.lock().retain(|drop| drop.at.elapsed() < TOKEN_TTL);
    }

    /// Tests only: a token that is `age` old (expiry cannot be waited for).
    #[cfg(feature = "test-hooks")]
    #[doc(hidden)]
    pub fn put_aged_for_test(&self, paths: Vec<PathBuf>, age: Duration) -> String {
        let token = self.put(paths);
        if let Some(drop) = self.lock().iter_mut().find(|drop| drop.token == token) {
            drop.at = Instant::now().checked_sub(age).unwrap_or(drop.at);
        }
        token
    }

    /// Tests only: how many drops are waiting.
    #[cfg(feature = "test-hooks")]
    #[doc(hidden)]
    pub fn pending_for_test(&self) -> usize {
        self.lock().len()
    }

    /// Tests only: how many waiting drops hold an image, and whether one is staged.
    #[cfg(feature = "test-hooks")]
    #[doc(hidden)]
    pub fn images_held_for_test(&self) -> (usize, bool) {
        let held = self
            .lock()
            .iter()
            .filter(|drop| drop.dropped.image.is_some())
            .count();
        (held, self.staged().is_some())
    }
}

/// A drag over the main window: the overlay goes on and off, a drop leaves a token. Other windows are not asked: a drop on them
/// does nothing.
pub fn on_drag_drop<R: Runtime>(app: &AppHandle<R>, event: &DragDropEvent) {
    on_drag_drop_from(app, event, &SystemDrag);
}

/// [`on_drag_drop`] with the drag read from `source` (the IPC tests give it a fake one).
///
/// A drag that enters with no paths has its image read at once when a chat is open; `Leave` forgets it; the drop uses it only if the
/// drag's generation is unchanged, else reads again. A drop always leaves a token, even an empty one: the window claims it and says so
/// when there was nothing to attach (before KINIG-133 such a drop did nothing, silently). A drop that came with no paths logs one
/// line: the type identifiers the drag offered (never contents) and what was made of them.
pub fn on_drag_drop_from<R: Runtime>(
    app: &AppHandle<R>,
    event: &DragDropEvent,
    source: &dyn DragSource,
) {
    let Some(tokens) = app.try_state::<DropTokens>() else {
        return;
    };
    match event {
        DragDropEvent::Enter { paths, .. } => {
            tokens.expire();
            let staged = (paths.is_empty() && tokens.accepting())
                .then(|| source.image())
                .flatten()
                .map(|image| Staged {
                    generation: source.generation(),
                    image,
                });
            *tokens.staged() = staged;
            let _ = app.emit_to(MAIN_LABEL, ATTACH_DRAG, true);
        }
        DragDropEvent::Leave => {
            *tokens.staged() = None;
            tokens.expire();
            let _ = app.emit_to(MAIN_LABEL, ATTACH_DRAG, false);
        }
        DragDropEvent::Drop { paths, .. } => {
            let _ = app.emit_to(MAIN_LABEL, ATTACH_DRAG, false);
            let staged = tokens.staged().take();
            let dropped = if paths.is_empty() {
                from_the_system(&tokens, source, staged)
            } else {
                Dropped {
                    paths: paths.clone(),
                    image: None,
                }
            };
            let token = tokens.put_drop(dropped);
            let _ = app.emit_to(MAIN_LABEL, ATTACH_DROPPED, token);
        }
        _ => {}
    }
}

/// A drop Tauri listed no path for: the files the pasteboard has, else its image (the staged one if it is of this drag).
fn from_the_system(
    tokens: &DropTokens,
    source: &dyn DragSource,
    staged: Option<Staged>,
) -> Dropped {
    let mut dropped = Dropped::default();
    let asked = tokens.accepting();
    if asked {
        dropped.paths = source.file_paths();
        if dropped.paths.is_empty() {
            let generation = source.generation();
            dropped.image = staged
                .filter(|staged| staged.generation == generation)
                .map(|staged| staged.image)
                .or_else(|| source.image());
        }
    }
    let result = match (dropped.paths.as_slice(), &dropped.image) {
        _ if !asked => "not read: no chat is open",
        ([_, ..], _) => "files",
        (_, Some(DragImage::Bytes(_))) => "an image",
        (_, Some(DragImage::Tiff(_))) => "a TIFF image",
        (_, Some(DragImage::TooLarge)) => "an image over the limit",
        _ => "nothing it can attach",
    };
    log::info!(
        "a drop came with no paths; the drag offered: {}; used: {result}",
        log_names(&source.type_names())
    );
    dropped
}
