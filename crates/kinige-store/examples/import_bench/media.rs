//! Slice 7 parts of `import_bench`: the media plan of the synthetic export and the attachment measurements.

#![allow(
    clippy::print_stdout,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::string_slice
)]

use super::ms;
use kinige_domain::SearchFilters;
use kinige_store::{MediaVariant, Now, Store};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Slice 7: opening Inbox with photos, the search by file name and the first thumbnail of 200 photos (built, then read back).
pub fn attachment_benches(store: &Store, data: &Path) {
    let conn = rusqlite::Connection::open(data.join("kinige.db")).unwrap();
    let ids_and_names = |limit: i64| -> Vec<(String, String)> {
        let mut statement = conn
            .prepare("SELECT id, filename FROM attachment WHERE kind = 'image' ORDER BY random() LIMIT ?1")
            .unwrap();
        statement
            .query_map([limit], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .flatten()
            .collect()
    };
    let sample = ids_and_names(200);
    let mut times = Vec::new();
    for _ in 0..30 {
        let t = Instant::now();
        let page = store
            .list_messages(kinige_domain::groups::INBOX_ID, None, 200, &Now::system())
            .unwrap();
        times.push((
            ms(t),
            page.messages
                .iter()
                .map(|m| m.attachments.len())
                .sum::<usize>(),
        ));
    }
    let with_files = times.first().map_or(0, |(_, n)| *n);
    let mut open: Vec<f64> = times.iter().map(|(t, _)| *t).collect();
    open.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!(
        "opening Inbox with photos (a page of 200 entries, {with_files} attachments on it, 30 rounds): p50 {:.1} ms, p95 {:.1} ms, max {:.1} ms",
        open[open.len() / 2],
        open[(open.len() * 95).div_ceil(100) - 1],
        open[open.len() - 1]
    );

    let mut search = Vec::new();
    for (_, name) in &sample {
        let stem = name
            .rsplit_once('.')
            .map_or(name.as_str(), |(stem, _)| stem);
        let t = Instant::now();
        let result = store
            .search(stem, &SearchFilters::default(), None, 50)
            .unwrap();
        search.push(ms(t));
        assert!(!result.hits.is_empty(), "a file name must be found");
    }
    search.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!(
        "search by file name ({} names, page of 50): p50 {:.1} ms, p95 {:.1} ms, max {:.1} ms",
        search.len(),
        search[search.len() / 2],
        search[(search.len() * 95).div_ceil(100) - 1],
        search[search.len() - 1]
    );

    let (mut built, mut cached) = (Vec::new(), Vec::new());
    for (id, _) in &sample {
        let t = Instant::now();
        let thumb = store.media(id, MediaVariant::Thumb);
        built.push(ms(t));
        assert!(thumb.is_some(), "the thumbnail is built");
        let t = Instant::now();
        store.media(id, MediaVariant::Thumb).unwrap();
        cached.push(ms(t));
    }
    for (label, mut values) in [
        ("built (decode + resize + encode)", built),
        ("read back from the cache", cached),
    ] {
        values.sort_by(|a, b| a.partial_cmp(b).unwrap());
        println!(
            "thumbnail {label}, {} photos of 1600 x 1200: p50 {:.1} ms, p95 {:.1} ms, max {:.1} ms",
            values.len(),
            values[values.len() / 2],
            values[(values.len() * 95).div_ceil(100) - 1],
            values[values.len() - 1]
        );
    }
}

/// Photos and plain files that the synthetic export carries (slice 7).
pub struct MediaPlan {
    pub images: usize,
    pub files: usize,
    pub file_mb: usize,
}

impl MediaPlan {
    pub fn parse(args: Vec<String>) -> MediaPlan {
        let mut plan = MediaPlan {
            images: 0,
            files: 0,
            file_mb: 1,
        };
        let mut args = args.into_iter();
        while let Some(flag) = args.next() {
            let value: usize = args
                .next()
                .and_then(|v| v.parse().ok())
                .expect("a number after the flag");
            match flag.as_str() {
                "--images" => plan.images = value,
                "--files" => plan.files = value,
                "--file-mb" => plan.file_mb = value,
                other => panic!("unknown flag {other}"),
            }
        }
        plan
    }

    pub fn total(&self) -> usize {
        self.images + self.files
    }

    /// Six drawn 1600 x 1200 JPEGs, hard-linked under `images` different names; one file of `file_mb` MB under `files` names.
    pub fn lay_out(&self, root: &Path) {
        let _ = std::fs::remove_dir_all(root);
        for folder in ["photos", "files"] {
            std::fs::create_dir_all(root.join(folder)).unwrap();
        }
        let bases: Vec<PathBuf> = (0..6)
            .map(|n| {
                let path = root.join(format!("photos/base_{n}.jpg"));
                std::fs::write(&path, drawn_jpeg(1600, 1200, n)).unwrap();
                path
            })
            .collect();
        for k in 0..self.images {
            std::fs::hard_link(
                &bases[k % bases.len()],
                root.join(format!("photos/photo_{k}.jpg")),
            )
            .unwrap();
        }
        if self.files > 0 {
            let big = root.join("files/big_0.bin");
            let mut state = 0x1234_5678_9ABC_DEF1u64;
            let mut chunk = vec![0u8; 1 << 20];
            let mut out = std::io::BufWriter::new(std::fs::File::create(&big).unwrap());
            for _ in 0..self.file_mb {
                for byte in chunk.iter_mut() {
                    state ^= state << 13;
                    state ^= state >> 7;
                    state ^= state << 17;
                    *byte = state as u8;
                }
                std::io::Write::write_all(&mut out, &chunk).unwrap();
            }
            drop(out);
            for k in 1..self.files {
                std::fs::hard_link(&big, root.join(format!("files/big_{k}.bin"))).unwrap();
            }
        }
    }
}

/// A drawn gradient with stripes: it compresses like a photo of a sky (a few hundred KB at this size), not like noise.
fn drawn_jpeg(width: u32, height: u32, seed: u32) -> Vec<u8> {
    use image::ImageEncoder;
    let image = image::RgbImage::from_fn(width, height, |x, y| {
        let stripe = ((x / 40 + y / 40 + seed) % 7) as u8 * 6;
        image::Rgb([
            (x * 255 / width) as u8 / 2 + stripe,
            (y * 255 / height) as u8 / 2 + stripe,
            (60 + seed * 30) as u8,
        ])
    });
    let mut bytes = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 85)
        .write_image(
            image.as_raw(),
            width,
            height,
            image::ExtendedColorType::Rgb8,
        )
        .unwrap();
    bytes
}
