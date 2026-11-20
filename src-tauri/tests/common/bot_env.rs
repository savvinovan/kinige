//! Shared by the slice 5c tests of the Bot API client and the pack job: stickers drawn by code, a store with an in-memory token store, a job
//! runner against a fake server, and a pack written into the library of a fixture.

#![allow(dead_code)]

use super::Fixture;
use super::fake_bot::*;
use image::ImageEncoder;
use kinige_domain::stickers::{StickerPackAdded, StickerPackProgress};
use kinige_domain::{ApiError, ErrorCode};
use kinige_lib::stickers::secret::{MemorySecrets, SecretStore};
use kinige_lib::stickers::{BotClient, JobContext, run_job};
use kinige_store::{NewPack, PackItem, Store, StoreOptions};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tauri::Manager;

/// The test token: it passes our loose rule and does not have the shape of a real one.
pub const TOKEN: &str = "42:fake-token-for-tests";
pub const LINK: &str = "https://t.me/addstickers/TestPack";

/// A sticker drawn by code.
pub fn webp(width: u32, height: u32) -> Vec<u8> {
    let image = image::RgbaImage::from_fn(width, height, |x, y| {
        image::Rgba([(x % 256) as u8, (y % 256) as u8, 90, 255])
    });
    let mut bytes = Vec::new();
    image::codecs::webp::WebPEncoder::new_lossless(&mut bytes)
        .write_image(
            image.as_raw(),
            width,
            height,
            image::ExtendedColorType::Rgba8,
        )
        .unwrap();
    bytes
}

/// A sticker whose size depends on the request target, so that two files of a pack never have the same content.
pub fn sticker_for(target: &str) -> Vec<u8> {
    let sum: u32 = target.bytes().map(u32::from).sum();
    webp(64 + sum % 200, 40 + sum % 7)
}

pub struct Env {
    pub _dir: tempfile::TempDir,
    pub store: Store,
    pub secrets: Arc<MemorySecrets>,
    pub cancel: Arc<AtomicBool>,
    pub progress: Arc<Mutex<Vec<StickerPackProgress>>>,
}

pub fn env() -> Env {
    let dir = tempfile::tempdir().unwrap();
    let mut options = StoreOptions::new(
        dir.path().join("kinige.db"),
        Some(dir.path().join("mirror")),
    );
    options.start_mirror = false;
    options.start_maintenance = false;
    let secrets = Arc::new(MemorySecrets::default());
    secrets.set(TOKEN).unwrap();
    Env {
        store: Store::open(options).unwrap(),
        _dir: dir,
        secrets,
        cancel: Arc::default(),
        progress: Arc::default(),
    }
}

impl Env {
    pub fn run(&self, base: &str, link: &str) -> Result<StickerPackAdded, ApiError> {
        self.run_with(BotClient::with_base(base, false).unwrap(), link)
    }

    pub fn run_with(&self, client: BotClient, link: &str) -> Result<StickerPackAdded, ApiError> {
        let progress = self.progress.clone();
        let context = JobContext {
            store: self.store.clone(),
            client: Arc::new(client),
            secrets: self.secrets.clone(),
            cancel: self.cancel.clone(),
            progress: Arc::new(move |p| progress.lock().unwrap().push(p)),
        };
        tauri::async_runtime::block_on(run_job(&context, link))
    }

    /// No pack in the library and no staging folder left.
    pub fn nothing_written(&self) {
        assert!(self.store.sticker_library(false).unwrap().packs.is_empty());
        let stage = self.store.data_dir().join("stickers/.tmp");
        let leftovers = std::fs::read_dir(stage)
            .map(|processes| {
                processes
                    .flatten()
                    .flat_map(|p| std::fs::read_dir(p.path()).into_iter().flatten().flatten())
                    .count()
            })
            .unwrap_or(0);
        assert_eq!(leftovers, 0, "no staging folder is left");
    }
}

/// The server of a healthy pack of `count` stickers.
pub fn healthy(count: usize) -> FakeBot {
    FakeBot::start(Arc::new(move |request| {
        let target = request.target.as_str();
        if target.contains("/getStickerSet?") {
            Reply::Json(200, set_json("Test Pack", count, &[]))
        } else if target.contains("/getFile?") {
            let id = target.split("file_id=").nth(1).unwrap_or_default();
            Reply::Json(200, file_json(id))
        } else if target.starts_with("/file/") {
            Reply::Raw(200, vec![], sticker_for(target))
        } else {
            Reply::Json(404, "{}".into())
        }
    }))
}

pub fn code(error: ApiError) -> ErrorCode {
    error.code
}

/// A pack of two drawn stickers in the library of the fixture's store; the id of the first sticker.
pub fn add_pack(fixture: &Fixture) -> String {
    let store = fixture.app.state::<Store>();
    let stage = store.pack_stage().unwrap();
    for (index, size) in [(0usize, 100u32), (1, 120)] {
        std::fs::write(stage.file(index), webp(size, 80)).unwrap();
    }
    store
        .add_sticker_pack(
            stage,
            NewPack {
                source: kinige_domain::stickers::PackSource::TelegramLink,
                link_name: Some("TestPack".into()),
                title: "Test Pack".into(),
                items: (0..2)
                    .map(|index| PackItem {
                        index,
                        emoji: Some("😺".into()),
                    })
                    .collect(),
            },
        )
        .unwrap();
    store.sticker_library(false).unwrap().packs[0].stickers[0]
        .id
        .clone()
}
