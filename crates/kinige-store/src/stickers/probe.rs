//! The probe of a sticker file (slice 5c, ADR-0014 and ADR-0021): every downloaded, imported or library file is untrusted input. It must be
//! a WebP or a PNG that does not animate (an animated WebP and an APNG are refused), whose header fits in 512 by 512 pixels, and at most
//! 2 MiB. Only the header is read; every codec call runs under `catch_unwind`.

use image::{ImageDecoder, ImageReader};
use kinige_domain::attachment::ImageFormat;
use kinige_domain::attachment::sticker::MAX_SIDE;
use kinige_domain::stickers::MAX_STICKER_BYTES;
use std::fs::{self, File};
use std::io::BufReader;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;

/// What the header of a sticker says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StickerProbe {
    /// `WebP` or `Png` only.
    pub format: ImageFormat,
    pub width: u32,
    pub height: u32,
}

/// The facts of a file that passes the sticker rules, or `None`.
pub(crate) fn probe_sticker(path: &Path) -> Option<StickerProbe> {
    catch_unwind(AssertUnwindSafe(|| probe(path))).unwrap_or(None)
}

fn probe(path: &Path) -> Option<StickerProbe> {
    let meta = fs::symlink_metadata(path).ok()?;
    if !meta.is_file() || meta.len() > MAX_STICKER_BYTES {
        return None;
    }
    let reader = ImageReader::new(BufReader::new(File::open(path).ok()?))
        .with_guessed_format()
        .ok()?;
    let format = reader.format()?;
    let (format, width, height) = match format {
        image::ImageFormat::WebP => {
            let decoder = image::codecs::webp::WebPDecoder::new(reader.into_inner()).ok()?;
            if decoder.has_animation() {
                return None;
            }
            let (width, height) = decoder.dimensions();
            (ImageFormat::WebP, width, height)
        }
        image::ImageFormat::Png => {
            let decoder = image::codecs::png::PngDecoder::new(reader.into_inner()).ok()?;
            if decoder.is_apng().ok()? {
                return None;
            }
            let (width, height) = decoder.dimensions();
            (ImageFormat::Png, width, height)
        }
        _ => return None,
    };
    ((1..=MAX_SIDE).contains(&width) && (1..=MAX_SIDE).contains(&height)).then_some(StickerProbe {
        format,
        width,
        height,
    })
}
