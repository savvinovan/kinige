//! The macOS drag pasteboard (KINIG-133). Tauri's drop event carries only what wry reads from it: the paths of the legacy
//! `NSFilenamesPboardType` list (wry 0.57, `wkwebview/drag_drop.rs`). A picture dragged out of Telegram Desktop or a browser is image
//! data or a file promise on that pasteboard, not a file, and arrives as an empty list. This module reads the same pasteboard for what
//! wry misses: file URLs the legacy list lacks, and the bytes of an image (PNG, JPEG, GIF, WebP as they are; TIFF re-encoded as PNG,
//! because the store treats a TIFF as a file card). File promises (`NSFilePromiseReceiver`) are not handled.
//!
//! Reading is by type name only: no entry text, path or byte count goes to the log.

use crate::drag_source::{DragImage, DragSource};
use kinige_domain::attachment::MAX_PASTE_BYTES;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSPasteboard};
use objc2_foundation::{NSArray, NSData, NSDictionary, NSString, NSURL};
use std::path::PathBuf;

/// `NSPasteboardNameDrag`. The test below compares it with the constant itself.
const DRAG_PASTEBOARD: &str = "Apple CFPasteboard drag";
/// `NSPasteboardTypeFileURL` and the legacy `NSFilenamesPboardType` (a property list of paths).
const FILE_URL: &str = "public.file-url";
const FILENAMES: &str = "NSFilenamesPboardType";
const TIFF: &str = "public.tiff";
/// Image types read as they are, in order of preference: a lossless or original copy before a TIFF made from pixels.
const IMAGE_TYPES: [&str; 4] = [
    "public.png",
    "public.jpeg",
    "com.compuserve.gif",
    "org.webmproject.webp",
];

/// The drag pasteboard of this system.
pub struct SystemDragPasteboard;

impl DragSource for SystemDragPasteboard {
    fn file_paths(&self) -> Vec<PathBuf> {
        file_paths_of(&NSPasteboard::pasteboardWithName(&NSString::from_str(
            DRAG_PASTEBOARD,
        )))
    }

    fn image(&self) -> Option<DragImage> {
        image_of(&NSPasteboard::pasteboardWithName(&NSString::from_str(
            DRAG_PASTEBOARD,
        )))
    }
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

/// The first image on the pasteboard, as bytes the store can read. Over 64 MiB (a paste's limit) is `TooLarge`; the bytes are dropped.
fn image_of(pasteboard: &NSPasteboard) -> Option<DragImage> {
    let types = pasteboard.types()?.to_vec();
    let has = |name: &str| types.iter().any(|kind| kind.to_string() == name);
    let data_of = |name: &str| pasteboard.dataForType(&NSString::from_str(name));
    if let Some(name) = IMAGE_TYPES.iter().copied().find(|name| has(name)) {
        return data_of(name).map(|data| bounded(&data));
    }
    if has(TIFF) {
        let tiff = data_of(TIFF)?;
        // A huge TIFF is refused before it is decoded: the PNG made from it would be as big.
        if tiff.length() > MAX_PASTE_BYTES {
            return Some(DragImage::TooLarge);
        }
        return tiff_as_png(&tiff).map(|png| bounded(&png));
    }
    None
}

/// The bytes, or `TooLarge` over a paste's limit (the bytes are not copied).
fn bounded(data: &NSData) -> DragImage {
    if data.length() > MAX_PASTE_BYTES {
        DragImage::TooLarge
    } else {
        DragImage::Bytes(data.to_vec())
    }
}

/// A TIFF re-encoded as PNG by the system's own decoder. `None` if it does not decode.
fn tiff_as_png(tiff: &NSData) -> Option<Retained<NSData>> {
    let image = NSBitmapImageRep::imageRepWithData(tiff)?;
    let options: Retained<NSDictionary<NSString, AnyObject>> = NSDictionary::new();
    // `representationUsingType:properties:` is `unsafe` in objc2 only because the dictionary's value type is not checked; the
    // dictionary is empty. ADR-0011 lists this call as the second `unsafe` of the app.
    #[allow(unsafe_code)]
    unsafe {
        image.representationUsingType_properties(NSBitmapImageFileType::PNG, &options)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(deprecated)]
    // the legacy list is what wry reads, and the literal above must be its value
    use objc2_app_kit::NSFilenamesPboardType;
    use objc2_app_kit::{
        NSPasteboardNameDrag, NSPasteboardTypeFileURL, NSPasteboardTypePNG, NSPasteboardTypeTIFF,
    };

    /// A throwaway pasteboard of this process: neither the user's clipboard nor the real drag pasteboard.
    fn scratch(types: &[(&str, &[u8])]) -> Retained<NSPasteboard> {
        let pasteboard = NSPasteboard::pasteboardWithUniqueName();
        for (name, bytes) in types {
            assert!(
                pasteboard
                    .setData_forType(Some(&NSData::with_bytes(bytes)), &NSString::from_str(name))
            );
        }
        pasteboard
    }

    fn png() -> Vec<u8> {
        let mut bytes = Vec::new();
        image::ImageEncoder::write_image(
            image::codecs::png::PngEncoder::new(&mut bytes),
            &[90u8; 3 * 3 * 3],
            3,
            3,
            image::ExtendedColorType::Rgb8,
        )
        .unwrap();
        bytes
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
    fn png_and_jpeg_bytes_are_taken_as_they_are() {
        let board = scratch(&[("public.png", &png())]);
        assert!(matches!(image_of(&board), Some(DragImage::Bytes(bytes)) if bytes == png()));
        let board = scratch(&[("public.jpeg", b"\xff\xd8jpeg"), ("public.tiff", b"II*\0")]);
        assert!(
            matches!(image_of(&board), Some(DragImage::Bytes(bytes)) if bytes == b"\xff\xd8jpeg")
        );
    }

    #[test]
    fn a_tiff_becomes_a_png_and_a_broken_one_is_nothing() {
        // A TIFF made by the system from the PNG, as an app that drags image data would put it.
        let tiff = NSBitmapImageRep::imageRepWithData(&NSData::from_vec(png()))
            .and_then(|image| image.TIFFRepresentation())
            .unwrap()
            .to_vec();
        let board = scratch(&[("public.tiff", &tiff)]);
        let Some(DragImage::Bytes(bytes)) = image_of(&board) else {
            panic!("a PNG was expected");
        };
        assert_eq!(&bytes[..4], b"\x89PNG");
        assert!(image_of(&scratch(&[("public.tiff", b"not a tiff")])).is_none());
    }

    #[test]
    fn text_alone_is_no_image() {
        assert!(image_of(&scratch(&[("public.utf8-plain-text", b"hello")])).is_none());
        assert!(image_of(&scratch(&[])).is_none());
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
