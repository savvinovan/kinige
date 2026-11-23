//! Slice 5c, a process killed in the middle of a pack download (ADR-0021): after `kill -9` there is no half pack and no file in the library;
//! the staging folder the dead process left is the sweep's, and the same link downloads again. The child is this test binary run again.

#![cfg(test)]

mod common;

use common::bot_env::*;
use common::fake_bot::*;
use kinige_lib::stickers::secret::{MemorySecrets, SecretStore};
use kinige_lib::stickers::{BotClient, JobContext, run_job};
use kinige_store::{Store, StoreOptions};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

const DIR_ENV: &str = "KINIGE_KILL_JOB_DIR";
const BASE_ENV: &str = "KINIGE_KILL_JOB_BASE";
const NAME: &str = "a_pack_job_killed_in_the_middle_leaves_no_pack_and_the_sweep_clears_the_rest";

fn store_in(dir: &Path) -> Store {
    let mut options = StoreOptions::new(dir.join("kinige.db"), Some(dir.join("mirror")));
    options.start_mirror = false;
    options.start_maintenance = false;
    Store::open(options).unwrap()
}

/// The server: the set and every `getFile` answer at once, the first file comes, the second never does.
fn stalling_second_file() -> FakeBot {
    FakeBot::start(Arc::new(|request| {
        let target = request.target.as_str();
        if target.contains("/getStickerSet?") {
            Reply::Json(200, set_json("Killed", 3, &[]))
        } else if target.contains("/getFile?") {
            Reply::Json(
                200,
                file_json(target.split("file_id=").nth(1).unwrap_or_default()),
            )
        } else if target.contains("/S0.webp") {
            Reply::Raw(200, vec![], sticker_for(target))
        } else {
            Reply::Stall
        }
    }))
}

#[test]
fn a_pack_job_killed_in_the_middle_leaves_no_pack_and_the_sweep_clears_the_rest() {
    if let (Ok(dir), Ok(base)) = (std::env::var(DIR_ENV), std::env::var(BASE_ENV)) {
        // The child: it runs the job and never finishes, the parent kills it while the second file is awaited.
        let secrets = Arc::new(MemorySecrets::default());
        secrets.set(TOKEN).unwrap();
        let store = store_in(&PathBuf::from(dir));
        store.set_telegram_token_saved(true).unwrap();
        let context = JobContext {
            store,
            client: Arc::new(BotClient::with_base(&base, false).unwrap()),
            secrets,
            cancel: Arc::default(),
            progress: Arc::new(|_| {}),
        };
        let _ = tauri::async_runtime::block_on(run_job(&context, LINK));
        return;
    }
    let data = tempfile::tempdir().unwrap();
    // The store of the parent is opened and dropped first so that the database exists and migrated.
    drop(store_in(data.path()));
    let server = stalling_second_file();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([NAME, "--exact", "--nocapture", "--test-threads=1"])
        .env(DIR_ENV, data.path())
        .env(BASE_ENV, server.base())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    while !server.seen().iter().any(|r| r.target.contains("/S1.webp")) {
        assert!(
            Instant::now() < deadline,
            "the child never asked for the second file"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    child.kill().unwrap();
    child.wait().unwrap();

    let store = store_in(data.path());
    assert!(
        store.sticker_library(false).unwrap().packs.is_empty(),
        "no half pack"
    );
    let library = std::fs::read_dir(data.path().join("stickers"))
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
                .count()
        })
        .unwrap_or(0);
    assert_eq!(library, 0, "no file in the library");
    assert!(
        std::fs::read_dir(data.path().join("stickers/.tmp"))
            .unwrap()
            .count()
            >= 1,
        "the dead process's staging folder is what is left"
    );
    let report = store.sweep_stickers().unwrap();
    assert!(report.stale_dirs >= 1, "{report:?}");
    assert_eq!(
        std::fs::read_dir(data.path().join("stickers/.tmp"))
            .unwrap()
            .count(),
        0
    );
    // The same link downloads again, whole.
    let healthy = healthy(3);
    let secrets = Arc::new(MemorySecrets::default());
    secrets.set(TOKEN).unwrap();
    store.set_telegram_token_saved(true).unwrap();
    let context = JobContext {
        store: store.clone(),
        client: Arc::new(BotClient::with_base(&healthy.base(), false).unwrap()),
        secrets,
        cancel: Arc::default(),
        progress: Arc::new(|_| {}),
    };
    let added = tauri::async_runtime::block_on(run_job(&context, LINK)).unwrap();
    assert_eq!(added.added, 3);
}
