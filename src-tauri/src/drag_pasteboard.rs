//! The macOS drag pasteboard (KINIG-133). Tauri's drop event carries only what wry reads from it: the paths of the legacy
//! `NSFilenamesPboardType` list (wry 0.57, `wkwebview/drag_drop.rs`). A picture dragged out of Telegram Desktop or a browser is image
//! data or a file promise on that pasteboard, not a file, and arrives as an empty list. This module reads the same pasteboard for what
//! wry misses: file URLs the legacy list lacks, and the bytes of an image, as the drag gives them (a TIFF is returned as a TIFF: it is
//! converted later, off the UI thread, by `drag_tiff`). File promises (`NSFilePromiseReceiver`) are not handled.
//!
//! Nothing is decoded here, and reading is by type name only: no entry text, path or byte count goes to the log.

use crate::drag_source::{DragImage, DragSource};
use kinige_domain::attachment::MAX_PASTE_BYTES;
use objc2_app_kit::NSPasteboard;
use objc2_foundation::{NSArray, NSData, NSString, NSURL};
use std::path::PathBuf;

/// `NSPasteboardNameDrag`. The test below compares it with the constant itself.
const DRAG_PASTEBOARD: &str = "Apple CFPasteboard drag";
/// `NSPasteboardTypeFileURL` and the legacy `NSFilenamesPboardType` (a property list of paths).
const FILE_URL: &str = "public.file-url";
const FILENAMES: &str = "NSFilenamesPboardType";
const TIFF: &str = "public.tiff";
/// Image types read as they are, in order of preference: an original copy before a TIFF made from pixels. The `com.trolltech.anymime`
/// ones are how Qt (Telegram Desktop) names an arbitrary MIME type on the pasteboard: `utiForMime` there is `com.trolltech.anymime.`
/// plus the MIME type with `/` as `--` (qtbase, `qutimimeconverter.mm`); Telegram sets `image/jpeg` next to its TIFF image data.
const IMAGE_TYPES: [&str; 8] = [
    "public.png",
    "public.jpeg",
    "com.compuserve.gif",
    "org.webmproject.webp",
    "com.trolltech.anymime.image--jpeg",
    "com.trolltech.anymime.image--png",
    "com.trolltech.anymime.image--gif",
    "com.trolltech.anymime.image--webp",
];

/// The drag pasteboard of this system.
pub struct SystemDragPasteboard;

impl DragSource for SystemDragPasteboard {
    fn file_paths(&self) -> Vec<PathBuf> {
        file_paths_of(&drag_pasteboard())
    }

    fn image(&self) -> Option<DragImage> {
        image_of(&drag_pasteboard())
    }

    fn generation(&self) -> i64 {
        i64::try_from(drag_pasteboard().changeCount()).unwrap_or(i64::MIN)
    }

    fn type_names(&self) -> Vec<String> {
        type_names_of(&drag_pasteboard())
    }
}

fn drag_pasteboard() -> objc2::rc::Retained<NSPasteboard> {
    NSPasteboard::pasteboardWithName(&NSString::from_str(DRAG_PASTEBOARD))
}

fn type_names_of(pasteboard: &NSPasteboard) -> Vec<String> {
    let types = pasteboard.types().map(|types| types.to_vec());
    types
        .unwrap_or_default()
        .iter()
        .map(|kind| kind.to_string())
        .collect()
}

/// File URLs of the pasteboard's items; failing that, the legacy list, which an app may fill only after it was asked for a file URL
/// (the native Telegram app declares a file URL and puts the path list in when asked; not verified on a real drag).
fn file_paths_of(pasteboard: &NSPasteboard) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let items = pasteboard.pasteboardItems().map(|items| items.to_vec());
    for item in items.unwrap_or_default() {
        let url = item
            .stringForType(&NSString::from_str(FILE_URL))
            .and_then(|text| NSURL::URLWithString(&text))
            .filter(|url| url.isFileURL())
            .and_then(|url| url.path());
        paths.extend(url.map(|path| PathBuf::from(path.to_string())));
    }
    if paths.is_empty() {
        paths = legacy_filenames(pasteboard);
    }
    paths
}

fn legacy_filenames(pasteboard: &NSPasteboard) -> Vec<PathBuf> {
    let list = pasteboard.propertyListForType(&NSString::from_str(FILENAMES));
    let Some(Ok(list)) = list.map(|list| list.downcast::<NSArray>()) else {
        return Vec::new();
    };
    list.to_vec()
        .into_iter()
        .filter_map(|path| path.downcast::<NSString>().ok())
        .map(|path| PathBuf::from(path.to_string()))
        .collect()
}

/// The first image on the pasteboard, as the bytes the drag gave: the first of `IMAGE_TYPES`, else the TIFF. Over 64 MiB is `TooLarge`
/// and is not copied. Only a copy of the bytes is made; nothing is decoded.
fn image_of(pasteboard: &NSPasteboard) -> Option<DragImage> {
    let types = type_names_of(pasteboard);
    let has = |name: &str| types.iter().any(|kind| kind == name);
    let data_of = |name: &str| pasteboard.dataForType(&NSString::from_str(name));
    if let Some(name) = IMAGE_TYPES.iter().copied().find(|name| has(name)) {
        log::debug!("the drag image is taken from {name}");
        return data_of(name).map(|data| bounded(&data, DragImage::Bytes));
    }
    if has(TIFF) {
        log::debug!("the drag image is taken from {TIFF}");
        return data_of(TIFF).map(|data| bounded(&data, DragImage::Tiff));
    }
    None
}

/// The bytes in the variant `make`, or `TooLarge` over a paste's limit (the bytes are not copied).
fn bounded(data: &NSData, make: fn(Vec<u8>) -> DragImage) -> DragImage {
    if data.length() > MAX_PASTE_BYTES {
        DragImage::TooLarge
    } else {
        make(data.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use objc2::rc::Retained;
    #[allow(deprecated)]
    // the legacy list is what wry reads, and the literal above must be its value
    use objc2_app_kit::NSFilenamesPboardType;
    use objc2_app_kit::{
        NSPasteboardNameDrag, NSPasteboardTypeFileURL, NSPasteboardTypePNG, NSPasteboardTypeTIFF,
    };

    /// A throwaway pasteboard with a name of its own (a UUID): neither the user's clipboard nor the real drag pasteboard. Not
    /// `pasteboardWithUniqueName`: with the tests running in parallel threads and processes it failed in about 5 runs of 100 (a
    /// write refused, or the types of another test seen); with UUID names none failed in 520.
    fn scratch(types: &[(&str, &[u8])]) -> Retained<NSPasteboard> {
        let name = format!("kinige-test-{}", uuid::Uuid::new_v4());
        let pasteboard = NSPasteboard::pasteboardWithName(&NSString::from_str(&name));
        // AppKit: a process must claim a pasteboard (`clearContents`) before it writes to it.
        pasteboard.clearContents();
        for (name, bytes) in types {
            assert!(
                pasteboard
                    .setData_forType(Some(&NSData::with_bytes(bytes)), &NSString::from_str(name))
            );
        }
        pasteboard
    }

    #[test]
    #[allow(unsafe_code, deprecated)] // reads AppKit's extern constants, to prove the literals above are their values
    fn the_literals_are_the_constants_of_appkit() {
        let (drag, url, filenames, png, tiff) = unsafe {
            (
                NSPasteboardNameDrag,
                NSPasteboardTypeFileURL,
                NSFilenamesPboardType,
                NSPasteboardTypePNG,
                NSPasteboardTypeTIFF,
            )
        };
        assert_eq!(drag.to_string(), DRAG_PASTEBOARD);
        assert_eq!(url.to_string(), FILE_URL);
        assert_eq!(filenames.to_string(), FILENAMES);
        assert_eq!(png.to_string(), IMAGE_TYPES[0]);
        assert_eq!(tiff.to_string(), TIFF);
    }

    #[test]
    fn png_and_jpeg_bytes_are_taken_as_they_are_and_before_a_tiff() {
        let board = scratch(&[("public.png", b"\x89PNGxx")]);
        assert_eq!(
            image_of(&board),
            Some(DragImage::Bytes(b"\x89PNGxx".to_vec()))
        );
        let board = scratch(&[("public.jpeg", b"\xff\xd8jpeg"), ("public.tiff", b"II*\0")]);
        assert_eq!(
            image_of(&board),
            Some(DragImage::Bytes(b"\xff\xd8jpeg".to_vec()))
        );
    }

    #[test]
    fn telegram_desktop_gives_its_jpeg_under_the_qt_name_and_it_beats_the_tiff() {
        // What a Qt drag with `setImageData` and `setData("image/jpeg")` offers: a TIFF and the JPEG under the anymime name.
        let board = scratch(&[
            ("public.tiff", b"II*\0tiff"),
            ("com.trolltech.anymime.image--jpeg", b"\xff\xd8telegram"),
        ]);
        assert_eq!(
            image_of(&board),
            Some(DragImage::Bytes(b"\xff\xd8telegram".to_vec()))
        );
        for kind in ["png", "gif", "webp"] {
            let name = format!("com.trolltech.anymime.image--{kind}");
            let board = scratch(&[(name.as_str(), b"bytes")]);
            assert_eq!(
                image_of(&board),
                Some(DragImage::Bytes(b"bytes".to_vec())),
                "{kind}"
            );
        }
    }

    #[test]
    fn a_tiff_alone_is_returned_as_a_tiff_and_not_decoded_here() {
        // The bytes are not a TIFF at all: nothing looks inside them while the drag is over the window.
        let board = scratch(&[("public.tiff", b"not even a tiff")]);
        assert_eq!(
            image_of(&board),
            Some(DragImage::Tiff(b"not even a tiff".to_vec()))
        );
    }

    #[test]
    fn over_the_paste_limit_is_too_large_and_is_not_copied() {
        let over = NSData::from_vec(vec![0u8; MAX_PASTE_BYTES + 1]);
        let at = NSData::from_vec(vec![0u8; MAX_PASTE_BYTES]);
        assert_eq!(bounded(&over, DragImage::Bytes), DragImage::TooLarge);
        assert_eq!(bounded(&over, DragImage::Tiff), DragImage::TooLarge);
        assert!(
            matches!(bounded(&at, DragImage::Bytes), DragImage::Bytes(bytes) if bytes.len() == MAX_PASTE_BYTES)
        );
        assert!(
            matches!(bounded(&at, DragImage::Tiff), DragImage::Tiff(bytes) if bytes.len() == MAX_PASTE_BYTES)
        );
    }

    #[test]
    fn a_pasteboard_with_a_huge_image_says_too_large() {
        let big = vec![0u8; MAX_PASTE_BYTES + 1];
        assert_eq!(
            image_of(&scratch(&[("public.jpeg", &big)])),
            Some(DragImage::TooLarge)
        );
        assert_eq!(
            image_of(&scratch(&[("public.tiff", &big)])),
            Some(DragImage::TooLarge)
        );
    }

    #[test]
    fn text_alone_is_no_image() {
        assert_eq!(
            image_of(&scratch(&[("public.utf8-plain-text", b"hello")])),
            None
        );
        assert_eq!(image_of(&scratch(&[])), None);
    }

    #[test]
    fn the_type_names_are_listed_without_contents() {
        let board = scratch(&[
            ("public.utf8-plain-text", b"secret words"),
            ("public.url", b"x"),
        ]);
        let names = type_names_of(&board);
        assert!(names.contains(&"public.utf8-plain-text".to_string()));
        assert!(names.contains(&"public.url".to_string()));
        assert!(names.iter().all(|name| !name.contains("secret")));
        assert!(type_names_of(&scratch(&[])).is_empty());
    }

    #[test]
    fn the_generation_changes_with_every_write_to_the_drag_pasteboard() {
        let board = scratch(&[("public.png", b"a")]);
        let before = board.changeCount();
        board.clearContents();
        assert!(board.setData_forType(
            Some(&NSData::with_bytes(b"b")),
            &NSString::from_str("public.png")
        ));
        assert_ne!(board.changeCount(), before);
    }

    #[test]
    fn file_urls_come_back_as_paths_and_other_urls_do_not() {
        let board = scratch(&[("public.file-url", b"file:///tmp/a%20b.png")]);
        assert_eq!(file_paths_of(&board), vec![PathBuf::from("/tmp/a b.png")]);
        // A link dragged out of a browser is a URL, not a file.
        let board = scratch(&[("public.url", b"https://example.org/a.png")]);
        assert!(file_paths_of(&board).is_empty());
        assert!(file_paths_of(&scratch(&[])).is_empty());
    }
}
