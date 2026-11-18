//! What a drag carries beyond the paths Tauri's drop event reports (KINIG-133, slice 7). Tauri hands over only file paths; a picture
//! dragged from Telegram Desktop or a browser is image data on the drag pasteboard, and the event arrives with no path at all. The
//! source is a trait so that the drop logic can be tested without a window and a pasteboard; the system one exists on macOS only.

use std::path::PathBuf;

/// An image taken from a drag that carried no file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DragImage {
    /// The bytes of a PNG, JPEG, GIF or WebP (a TIFF is already a PNG here). They go the way a pasted image goes.
    Bytes(Vec<u8>),
    /// More than a paste may carry (64 MiB): the bytes were not kept.
    TooLarge,
}

/// The drag in progress, read from the system when asked. Reading may block on the app that started the drag.
pub trait DragSource {
    /// Files the drag carries that the drop event did not list.
    fn file_paths(&self) -> Vec<PathBuf>;
    /// The image the drag carries, if it carries one and no file.
    fn image(&self) -> Option<DragImage>;
    /// The names of the data types the drag offers (type identifiers only, never contents), for the log line of a drop that could not
    /// be used: the one thing that tells which app sent what.
    fn type_names(&self) -> Vec<String>;
}

/// The drag of this system: the drag pasteboard on macOS, nothing elsewhere (Windows and Linux drops are files only).
#[cfg(target_os = "macos")]
pub use crate::drag_pasteboard::SystemDragPasteboard as SystemDrag;

#[cfg(not(target_os = "macos"))]
pub struct SystemDrag;

#[cfg(not(target_os = "macos"))]
impl DragSource for SystemDrag {
    fn file_paths(&self) -> Vec<PathBuf> {
        Vec::new()
    }

    fn image(&self) -> Option<DragImage> {
        None
    }

    fn type_names(&self) -> Vec<String> {
        Vec::new()
    }
}
