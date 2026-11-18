//! A TIFF from a drag, turned into a PNG (KINIG-133). Telegram Desktop puts its photo on the drag as a TIFF (Qt's image data), and the
//! store keeps a TIFF as a file card, so without this a photo would arrive as a card, not as a picture.
//!
//! The decoder is the `image` crate's, in pure Rust, so it is safe to run on any thread; it runs in `attach_dropped`, a command that
//! is off the UI thread. A TIFF is small on the wire and can be huge once decoded, so the header is read first and the limits of
//! ADR-0014 apply before any pixel is decoded: 30 000 px on a side and 512 MiB of memory, and for a conversion also at most 50
//! megapixels. A TIFF over a limit, a broken one or a decoder panic gives `None`: the caller attaches the bytes as they are.

use image::metadata::Orientation;
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, Limits};
use kinige_domain::attachment::MAX_PASTE_BYTES;
use std::io::Cursor;
use std::panic::{AssertUnwindSafe, catch_unwind};

/// The decoder limits of the thumbnails (`kinige-store`, `attachment/decode.rs`, ADR-0014).
const MAX_ALLOC: u64 = 512 * 1024 * 1024;
const MAX_SIDE: u32 = 30_000;
/// A conversion decodes the whole bitmap and encodes it again: a bigger picture than this stays a TIFF file card.
const MAX_PIXELS: u64 = 50_000_000;

/// The PNG made from a TIFF, with the orientation applied, or `None` if the TIFF is over a limit, does not decode, or the PNG would
/// be larger than a paste may be.
pub fn tiff_to_png(tiff: &[u8]) -> Option<Vec<u8>> {
    catch_unwind(AssertUnwindSafe(|| convert(tiff)))
        .unwrap_or_else(|_| {
            log::error!("a decoder panicked, a dragged TIFF is attached as it is");
            None
        })
        .filter(|png| png.len() <= MAX_PASTE_BYTES)
}

fn convert(tiff: &[u8]) -> Option<Vec<u8>> {
    let mut reader = ImageReader::with_format(Cursor::new(tiff), ImageFormat::Tiff);
    let mut limits = Limits::default();
    limits.max_alloc = Some(MAX_ALLOC);
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    reader.limits(limits);
    // Only the header is read here; the side limits are checked in `into_decoder`, the pixel count right below, both before a pixel.
    let mut decoder = reader.into_decoder().ok()?;
    let (width, height) = decoder.dimensions();
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > MAX_PIXELS {
        return None;
    }
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let mut image = DynamicImage::from_decoder(decoder).ok()?;
    image.apply_orientation(orientation);
    let mut png = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
        .ok()?;
    Some(png)
}

/// Tests only: a PackBits TIFF of `side` x `side` RGB pixels that is a few MiB on the wire (every row is runs of one byte), the shape
/// of a decompression bomb. The reviewer's probe of the first version of this fix: 12 000 px took 3 s and 1 GB on the UI thread.
/// The arithmetic is on small literals the test chooses; nothing here runs in the app.
#[cfg(feature = "test-hooks")]
#[doc(hidden)]
#[allow(clippy::cast_possible_truncation)]
pub fn packbits_tiff_for_test(side: u32) -> Vec<u8> {
    let row = (side * 3) as usize;
    let mut packed_row = Vec::new();
    let mut left = row;
    while left > 0 {
        let run = left.min(128);
        packed_row.push((1i16 - run as i16) as i8 as u8);
        packed_row.push(0x80);
        left -= run;
    }
    let data_len = packed_row.len() * side as usize;
    let mut tiff: Vec<u8> = b"II*\0".to_vec();
    tiff.extend(8u32.to_le_bytes());
    // width, height, bits per sample, compression (PackBits), photometric (RGB), strip offset, samples per pixel, rows per strip,
    // strip byte count, planar configuration
    let entries: [(u16, u16, u32, u32); 10] = [
        (256, 4, 1, side),
        (257, 4, 1, side),
        (258, 3, 3, 134),
        (259, 3, 1, 32773),
        (262, 3, 1, 2),
        (273, 4, 1, 140),
        (277, 3, 1, 3),
        (278, 4, 1, side),
        (279, 4, 1, data_len as u32),
        (284, 3, 1, 1),
    ];
    tiff.extend(10u16.to_le_bytes());
    for (tag, kind, count, value) in entries {
        tiff.extend(tag.to_le_bytes());
        tiff.extend(kind.to_le_bytes());
        tiff.extend(count.to_le_bytes());
        if kind == 3 && count == 1 {
            tiff.extend((value as u16).to_le_bytes());
            tiff.extend([0, 0]);
        } else {
            tiff.extend(value.to_le_bytes());
        }
    }
    tiff.extend(0u32.to_le_bytes());
    for _ in 0..3 {
        tiff.extend(8u16.to_le_bytes());
    }
    for _ in 0..side {
        tiff.extend(&packed_row);
    }
    tiff
}

#[cfg(test)]
mod tests {
    use super::packbits_tiff_for_test as packbits_tiff;
    use super::*;
    use std::time::{Duration, Instant};

    fn tiff_of(width: u32, height: u32) -> Vec<u8> {
        let image = DynamicImage::new_rgb8(width, height);
        let mut bytes = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Tiff)
            .unwrap();
        bytes
    }

    #[test]
    fn a_small_tiff_becomes_a_png() {
        let png = tiff_to_png(&tiff_of(40, 30)).unwrap();
        assert_eq!(&png[..4], b"\x89PNG");
    }

    #[test]
    fn the_probe_tiff_is_valid_and_small_on_the_wire() {
        // A side that is allowed: the file decodes, so the oversized case below fails because of the limit and not because of the file.
        let tiff = packbits_tiff(300);
        assert!(tiff.len() < 100_000);
        assert!(tiff_to_png(&tiff).is_some());
    }

    #[test]
    fn a_tiff_bomb_is_refused_from_its_header_without_decoding() {
        // 12 000 x 12 000 RGB is 432 MB decoded (it passes the memory limit and the side limit): over the 50 megapixel conversion limit.
        let tiff = packbits_tiff(12_000);
        assert!(
            tiff.len() < 8 * 1024 * 1024,
            "{} bytes on the wire",
            tiff.len()
        );
        let started = Instant::now();
        assert!(tiff_to_png(&tiff).is_none());
        assert!(
            started.elapsed() < Duration::from_millis(500),
            "{:?}",
            started.elapsed()
        );
    }

    #[test]
    fn a_side_over_the_decoder_limit_is_refused() {
        assert!(tiff_to_png(&packbits_tiff(30_001)).is_none());
    }

    #[test]
    fn garbage_is_none_and_never_a_panic() {
        for bytes in [
            &b""[..],
            b"II*\0",
            b"MM\0*",
            b"II*\0\x08\0\0\0\xff\xff",
            b"not a tiff at all",
        ] {
            assert!(tiff_to_png(bytes).is_none());
        }
        let mut cut = tiff_of(64, 64);
        cut.truncate(cut.len() / 2);
        assert!(tiff_to_png(&cut).is_none());
    }
}
