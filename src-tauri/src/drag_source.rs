//! What a drag carries beyond the paths Tauri's drop event reports (KINIG-133, slice 7). Tauri hands over only file paths; a picture
//! dragged from Telegram Desktop or a browser is image data on the drag pasteboard, and the event arrives with no path at all. The
//! source is a trait so that the drop logic can be tested without a window and a pasteboard; the system one exists on macOS only.

use std::path::PathBuf;

/// How many type names the log line of a drop lists, and how long one may be (identifiers only: they come from other apps).
const LOG_NAMES: usize = 12;
const LOG_NAME_CHARS: usize = 64;

/// An image taken from a drag that carried no file. The bytes are as the drag gave them: nothing is decoded while the drag is over the
/// window (that runs on the UI thread); a TIFF is converted when the drop is claimed, off that thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DragImage {
    /// A PNG, JPEG, GIF or WebP. It goes the way a pasted image goes.
    Bytes(Vec<u8>),
    /// A TIFF (what Qt's `setImageData` becomes on the pasteboard). Turned into a PNG inside the decoder limits when it is attached;
    /// if that is not possible it is attached as it is, which the store keeps as a file card, as it does for a pasted TIFF.
    Tiff(Vec<u8>),
    /// More than a paste may carry (64 MiB): the bytes were not kept.
    TooLarge,
}

/// The drag in progress, read from the system when asked. Reading may block on the app that started the drag.
pub trait DragSource {
    /// Files the drag carries that the drop event did not list.
    fn file_paths(&self) -> Vec<PathBuf>;
    /// The image the drag carries, if it carries one and no file.
    fn image(&self) -> Option<DragImage>;
    /// Changes with every new drag (the pasteboard's change count): an image read for one drag is never used for another.
    fn generation(&self) -> i64;
    /// The names of the data types the drag offers (type identifiers only, never contents): the one thing that tells which app sent
    /// what, for the log line of a drop that went through the pasteboard.
    fn type_names(&self) -> Vec<String>;
}

/// The names for a log line: at most 12, each cut to 64 characters, joined with commas; `none` for an empty list.
pub fn log_names(names: &[String]) -> String {
    let mut listed: Vec<String> = names
        .iter()
        .take(LOG_NAMES)
        .map(|name| name.chars().take(LOG_NAME_CHARS).collect())
        .collect();
    if names.len() > LOG_NAMES {
        listed.push(format!("and {} more", names.len() - LOG_NAMES));
    }
    if listed.is_empty() {
        "none".to_string()
    } else {
        listed.join(", ")
    }
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

    fn generation(&self) -> i64 {
        0
    }

    fn type_names(&self) -> Vec<String> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_log_names_are_capped_in_number_and_length() {
        assert_eq!(log_names(&[]), "none");
        assert_eq!(log_names(&["a".into(), "b".into()]), "a, b");
        let many: Vec<String> = (0..20).map(|n| format!("t{n}")).collect();
        let line = log_names(&many);
        assert!(line.ends_with("t11, and 8 more"), "{line}");
        let long = log_names(&["x".repeat(500)]);
        assert_eq!(long.chars().count(), LOG_NAME_CHARS);
        assert_eq!(
            log_names(&["я".repeat(100)]).chars().count(),
            LOG_NAME_CHARS
        );
    }
}
