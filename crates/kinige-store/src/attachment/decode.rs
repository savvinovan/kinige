//! Reading images (ADR-0014): the header when a file is attached (format, size after the EXIF orientation) and a thumbnail on
//! first display. `image` with the features `jpeg`, `png`, `gif` and `webp` only. Every codec call runs under `catch_unwind`:
//! a panic in a decoder is "No preview", never a dead thread.

use image::metadata::Orientation;
use image::{DynamicImage, ImageDecoder, ImageFormat as ImageFormatCrate, ImageReader, Limits};
use kinige_domain::attachment::ImageFormat;
use std::fs::File;
use std::io::{BufReader, Cursor};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;

/// The longest side of a thumbnail in pixels.
pub(crate) const THUMB_SIDE: u32 = 960;
/// The JPEG quality of a thumbnail.
const JPEG_QUALITY: u8 = 85;
/// What a decoder may allocate: 512 MiB (ADR-0014).
const MAX_ALLOC: u64 = 512 * 1024 * 1024;
/// The widest or tallest image the decoder accepts for a thumbnail; a larger one is "No preview".
const MAX_SIDE: u32 = 30_000;

/// What the header of an image says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ImageInfo {
    pub format: ImageFormat,
    /// Pixels after the EXIF orientation (swapped for 90 and 270 degrees).
    pub width: u32,
    pub height: u32,
}

/// A thumbnail encoded in memory.
pub(crate) struct Thumb {
    pub bytes: Vec<u8>,
    /// `jpg`, or `png` when the image has alpha.
    pub ext: &'static str,
}

/// The format and size of a file if its content is a JPEG, PNG, GIF or WebP whose header reads. Only the header is read, with no
/// limits: it allocates nothing, and an enormous image still gets its size (its thumbnail fails later).
pub(crate) fn probe_image(path: &Path) -> Option<ImageInfo> {
    catch_unwind(AssertUnwindSafe(|| probe(path))).unwrap_or(None)
}

fn probe(path: &Path) -> Option<ImageInfo> {
    let file = File::open(path).ok()?;
    let mut reader = ImageReader::new(BufReader::new(file))
        .with_guessed_format()
        .ok()?;
    let format = ours(reader.format()?)?;
    reader.no_limits();
    let mut decoder = reader.into_decoder().ok()?;
    let (width, height) = decoder.dimensions();
    if width == 0 || height == 0 {
        return None;
    }
    // PNG and GIF have no orientation (`NoTransforms`); a broken Exif block is "no rotation".
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let (width, height) = if swaps_sides(orientation) {
        (height, width)
    } else {
        (width, height)
    };
    Some(ImageInfo {
        format,
        width,
        height,
    })
}

/// The thumbnail of an image file: fit in 960 x 960 (never enlarged), orientation applied, EXIF dropped by re-encoding.
/// `None` if the file does not decode inside the limits.
pub(crate) fn make_thumbnail(path: &Path, panic_for_test: bool) -> Option<Thumb> {
    catch_unwind(AssertUnwindSafe(|| {
        // Hook for the safety-net test; absent in a release build (`debug_assertions` off).
        assert!(
            !(cfg!(debug_assertions) && panic_for_test),
            "check: panic in a decoder"
        );
        thumbnail(path)
    }))
    .unwrap_or_else(|_| {
        log::error!("a decoder panicked, the thumbnail is skipped");
        None
    })
}

fn thumbnail(path: &Path) -> Option<Thumb> {
    let file = File::open(path).ok()?;
    let mut reader = ImageReader::new(BufReader::new(file))
        .with_guessed_format()
        .ok()?;
    ours(reader.format()?)?;
    let mut limits = Limits::default();
    limits.max_alloc = Some(MAX_ALLOC);
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().ok()?;
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let mut image = DynamicImage::from_decoder(decoder).ok()?;
    image.apply_orientation(orientation);
    if image.width() > THUMB_SIDE || image.height() > THUMB_SIDE {
        image = image.thumbnail(THUMB_SIDE, THUMB_SIDE);
    }
    encode(&image)
}

fn encode(image: &DynamicImage) -> Option<Thumb> {
    let mut bytes = Vec::new();
    if image.has_alpha() {
        image
            .write_to(&mut Cursor::new(&mut bytes), ImageFormatCrate::Png)
            .ok()?;
        return Some(Thumb { bytes, ext: "png" });
    }
    let rgb = image.to_rgb8();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, JPEG_QUALITY)
        .encode_image(&rgb)
        .ok()?;
    Some(Thumb { bytes, ext: "jpg" })
}

fn ours(format: ImageFormatCrate) -> Option<ImageFormat> {
    match format {
        ImageFormatCrate::Jpeg => Some(ImageFormat::Jpeg),
        ImageFormatCrate::Png => Some(ImageFormat::Png),
        ImageFormatCrate::Gif => Some(ImageFormat::Gif),
        ImageFormatCrate::WebP => Some(ImageFormat::WebP),
        _ => None,
    }
}

fn swaps_sides(orientation: Orientation) -> bool {
    matches!(
        orientation,
        Orientation::Rotate90
            | Orientation::Rotate270
            | Orientation::Rotate90FlipH
            | Orientation::Rotate270FlipH
    )
}
