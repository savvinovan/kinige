//! Helpers of the slice 5c tests: a pack written through the same API the downloader uses, with stickers drawn by code.

#![allow(dead_code)]

use super::attach::webp;
use super::raw;
use kinige_domain::stickers::{PackSource, StickerLibrary, StickerPackAdded};
use kinige_store::{NewPack, PackItem, Store, StoreError};
use std::path::{Path, PathBuf};

/// The synthetic stickers: `(bytes, emoji)`, each with its own size so that every one has its own content.
pub fn drawn(count: u32) -> Vec<(Vec<u8>, Option<String>)> {
    (0..count)
        .map(|i| {
            (
                webp(200 + i, 160),
                Some(["😺", "🔥", "🌲", "🐟"][(i % 4) as usize].to_string()),
            )
        })
        .collect()
}

/// Adds a pack by link name `TestPack` (or another) from files written into a fresh staging folder.
pub fn add_pack(
    store: &Store,
    link: Option<&str>,
    title: &str,
    files: &[(Vec<u8>, Option<String>)],
) -> Result<StickerPackAdded, StoreError> {
    let stage = store.pack_stage()?;
    let mut items = Vec::new();
    for (index, (bytes, emoji)) in files.iter().enumerate() {
        std::fs::write(stage.file(index), bytes).unwrap();
        items.push(PackItem {
            index,
            emoji: emoji.clone(),
        });
    }
    store.add_sticker_pack(
        stage,
        NewPack {
            source: PackSource::TelegramLink,
            link_name: link.map(str::to_string),
            title: title.to_string(),
            items,
        },
    )
}

/// The ids of the stickers of the first pack that has the title.
pub fn ids_of(library: &StickerLibrary, title: &str) -> Vec<String> {
    library
        .packs
        .iter()
        .find(|pack| pack.title == title)
        .map(|pack| pack.stickers.iter().map(|s| s.id.clone()).collect())
        .unwrap_or_default()
}

/// The files directly inside `<data>/stickers/` (not the `.tmp` folder), sorted.
pub fn library_files(dir: &Path) -> Vec<String> {
    super::attach::names(&dir.join("stickers"))
        .into_iter()
        .filter(|name| !name.starts_with('.'))
        .collect()
}

/// A count from a query on the database file.
pub fn count(dir: &Path, sql: &str) -> i64 {
    raw(dir).query_row(sql, [], |r| r.get(0)).unwrap()
}

pub fn stage_dirs(dir: &Path) -> Vec<PathBuf> {
    let root = dir.join("stickers/.tmp");
    std::fs::read_dir(&root)
        .map(|entries| {
            entries
                .flatten()
                .flat_map(|process| {
                    std::fs::read_dir(process.path())
                        .into_iter()
                        .flatten()
                        .flatten()
                })
                .map(|e| e.path())
                .collect()
        })
        .unwrap_or_default()
}
