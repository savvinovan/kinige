//! Synthetic files for the attachment tests: images drawn with the `image` crate, a JPEG with an EXIF orientation, a PNG whose
//! header claims 100,000 x 100,000 pixels. No third-party assets.

use image::{ImageEncoder, Rgb, RgbImage, Rgba, RgbaImage};
use std::path::{Path, PathBuf};

pub fn jpeg(width: u32, height: u32) -> Vec<u8> {
    let image = RgbImage::from_fn(width, height, |x, y| {
        Rgb([(x * 255 / width.max(1)) as u8, (y * 255 / height.max(1)) as u8, 90])
    });
    let mut bytes = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 80)
        .encode_image(&image)
        .unwrap();
    bytes
}

pub fn png(width: u32, height: u32) -> Vec<u8> {
    let image = RgbImage::from_fn(width, height, |x, y| Rgb([(x % 256) as u8, (y % 256) as u8, 30]));
    let mut bytes = Vec::new();
    image::codecs::png::PngEncoder::new(&mut bytes)
        .write_image(image.as_raw(), width, height, image::ExtendedColorType::Rgb8)
        .unwrap();
    bytes
}

/// A PNG with a transparent corner.
pub fn png_with_alpha(width: u32, height: u32) -> Vec<u8> {
    let image = RgbaImage::from_fn(width, height, |x, y| {
        Rgba([200, 40, 40, if x < width / 2 && y < height / 2 { 0 } else { 255 }])
    });
    let mut bytes = Vec::new();
    image::codecs::png::PngEncoder::new(&mut bytes)
        .write_image(image.as_raw(), width, height, image::ExtendedColorType::Rgba8)
        .unwrap();
    bytes
}

pub fn gif(width: u32, height: u32) -> Vec<u8> {
    let image = RgbaImage::from_fn(width, height, |x, _| Rgba([(x % 256) as u8, 120, 60, 255]));
    let mut bytes = Vec::new();
    image::codecs::gif::GifEncoder::new(&mut bytes)
        .encode(image.as_raw(), width, height, image::ExtendedColorType::Rgba8)
        .unwrap();
    bytes
}

/// A JPEG with an Exif block that says orientation = `orientation` (1..=8) right after the start marker.
pub fn jpeg_with_orientation(width: u32, height: u32, orientation: u8) -> Vec<u8> {
    let plain = jpeg(width, height);
    let mut exif: Vec<u8> = vec![0xFF, 0xE1, 0x00, 0x22];
    exif.extend_from_slice(b"Exif\0\0");
    exif.extend_from_slice(&[0x49, 0x49, 0x2A, 0x00, 0x08, 0x00, 0x00, 0x00]);
    exif.extend_from_slice(&[0x01, 0x00]);
    exif.extend_from_slice(&[0x12, 0x01, 0x03, 0x00, 0x01, 0x00, 0x00, 0x00, orientation, 0x00, 0x00, 0x00]);
    exif.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
    let mut out = plain[..2].to_vec();
    out.extend_from_slice(&exif);
    out.extend_from_slice(&plain[2..]);
    out
}

/// A PNG whose header claims `width` x `height` and whose pixel data is a few bytes: a decompression bomb for the thumbnail.
pub fn png_bomb(width: u32, height: u32) -> Vec<u8> {
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &[0x78, 0x9c, 0x63, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01]);
    chunk(&mut out, b"IEND", &[]);
    out
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let mut body = kind.to_vec();
    body.extend_from_slice(data);
    out.extend_from_slice(&body);
    out.extend_from_slice(&crc32(&body).to_be_bytes());
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

/// Writes `bytes` to `dir/name` and returns the path.
pub fn write(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, bytes).unwrap();
    path
}

/// The files directly inside a directory (names), sorted; an absent directory is empty.
pub fn names(dir: &Path) -> Vec<String> {
    let mut found: Vec<String> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    found.sort();
    found
}
