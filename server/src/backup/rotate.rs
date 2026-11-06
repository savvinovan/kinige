//! Rotation of backups (PRD §10.3, Q14 = 30 days): `kinige-server backup <dir> --prune`.
//!
//! **The newest backup is kept whatever its age**, so a server that has not been backed up for longer than the retention still has
//! its last one. The promise to the user ("backups are kept for 30 days and then rotated") therefore holds from the next backup on:
//! until the next backup is taken, the newest one stays (ADR-0001 §8.9, PRD §10.3, `deploy/backup.cron.example`).

use super::BackupError;
use super::manifest::Manifest;
use std::path::{Path, PathBuf};
use tokio::fs;

/// A day: a half-written backup directory older than this is removed by the next prune.
const TMP_MAX_AGE_MS: u64 = 24 * 60 * 60 * 1000;

/// What `prune` removed.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Pruned {
    pub backups: Vec<String>,
    pub unfinished: usize,
}

/// Removes the backups under `root` whose cut time is older than `retention_ms`, **except the newest one**: a server that has not been
/// backed up for longer than the retention keeps its last backup. Half-written directories older than a day go too. A directory that
/// is not a valid backup is never removed.
pub async fn prune(root: &Path, now_ms: u64, retention_ms: u64) -> Result<Pruned, BackupError> {
    let mut pruned = Pruned::default();
    let mut valid = Vec::new();
    let mut entries = fs::read_dir(root).await.map_err(io)?;
    while let Some(entry) = entries.next_entry().await.map_err(io)? {
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        if name.starts_with(".tmp-") {
            let old = fs::metadata(&path)
                .await
                .ok()
                .and_then(|meta| meta.modified().ok())
                .and_then(|modified| std::time::SystemTime::now().duration_since(modified).ok())
                .is_some_and(|age| age.as_millis() >= u128::from(TMP_MAX_AGE_MS));
            if old && fs::remove_dir_all(&path).await.is_ok() {
                pruned.unfinished += 1;
            }
        } else if let Ok(manifest) = Manifest::read(&path).await {
            valid.push((manifest.cut_ms, path, name));
        }
    }
    valid.sort();
    let newest = valid.len().saturating_sub(1);
    for (index, (cut_ms, path, name)) in valid.into_iter().enumerate() {
        if index != newest
            && now_ms.saturating_sub(cut_ms) > retention_ms
            && fs::remove_dir_all(&path).await.is_ok()
        {
            pruned.backups.push(name);
        }
    }
    Ok(pruned)
}

/// The newest valid backup under `root`: the one whose blobs the next backup may link to.
pub async fn newest_valid(root: &Path) -> Option<(PathBuf, Manifest)> {
    let mut entries = fs::read_dir(root).await.ok()?;
    let mut best: Option<(PathBuf, Manifest)> = None;
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if entry.file_name().to_string_lossy().starts_with(".tmp-") {
            continue;
        }
        if let Ok(manifest) = Manifest::read(&path).await
            && best
                .as_ref()
                .is_none_or(|(_, held)| manifest.cut_ms > held.cut_ms)
        {
            best = Some((path, manifest));
        }
    }
    best
}

fn io(error: std::io::Error) -> BackupError {
    log::error!("backup: read the backup directory: {:?}", error.kind());
    BackupError::Io("read the backup directory")
}
