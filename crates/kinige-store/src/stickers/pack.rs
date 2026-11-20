//! Writing a pack (slice 5c, ADR-0021): all or nothing. The downloader (`src-tauri`) streams each file into a staging folder
//! `stickers/.tmp/<process>/<job>/` ([`PackStage`]); [`Store::add_sticker_pack`] probes every file, then one writer job renames the files into
//! `stickers/` first and writes the rows in one transaction last. A crash between the two leaves files without rows (the sweep removes them),
//! never rows without files. A pack replaced by name is deleted in the same transaction and its old files after the commit.

use super::probe::{StickerProbe, probe_sticker};
use super::{delete_library_files, pack_files};
use crate::attachment::{Dirs, Faults, create_private, io_error, make_dir, sync_dir};
use crate::error::{read_err, write_err};
use crate::{Store, StoreError};
use kinige_domain::attachment::{hex_lower, sticker::clean_emoji};
use kinige_domain::stickers::{
    EXPORT_PACK_TITLE, MAX_PACK_STICKERS, PackSource, StickerPackAdded, clean_title,
};
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// The staging folder of one pack download: the files go in as `<index>.part`. Dropping it removes the folder, whatever happened.
#[derive(Debug)]
pub struct PackStage {
    dir: PathBuf,
}

impl PackStage {
    /// Where the file of item `index` is written. The caller streams into it and counts the bytes (2 MiB a file, 64 MiB a pack).
    pub fn file(&self, index: usize) -> PathBuf {
        self.dir.join(format!("{index}.part"))
    }

    /// A new private file for item `index` (mode 0600 on Unix); it must not exist.
    pub fn create(&self, index: usize) -> std::io::Result<fs::File> {
        create_private(&self.file(index))
    }
}

impl Drop for PackStage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

/// A downloaded sticker waiting for the write: its file is `stage.file(index)`.
#[derive(Debug, Clone)]
pub struct PackItem {
    pub index: usize,
    /// What Telegram says; anything but a valid emoji sequence is dropped here.
    pub emoji: Option<String>,
}

/// A pack to add.
#[derive(Debug, Clone)]
pub struct NewPack {
    pub source: PackSource,
    /// The pack name of a link; a pack with this name already in the library is replaced.
    pub link_name: Option<String>,
    pub title: String,
    pub items: Vec<PackItem>,
}

impl Store {
    /// A fresh staging folder for one pack download.
    pub fn pack_stage(&self) -> Result<PackStage, StoreError> {
        let dir = self.dirs().stickers_tmp().join(Uuid::new_v4().to_string());
        make_dir(&dir).map_err(io_error)?;
        Ok(PackStage { dir })
    }

    /// Adds the pack in one writer job, all or nothing. A file that fails the sticker probe (WebP or PNG, not animated, at most 512 by 512
    /// and 2 MiB) is left out and counted in `skipped`, and so is a second file with the same content. When nothing is left, `StickerUnreadable`
    /// and nothing is written. The staging folder is removed in every case.
    pub fn add_sticker_pack(
        &self,
        stage: PackStage,
        pack: NewPack,
    ) -> Result<StickerPackAdded, StoreError> {
        if pack.items.len() > MAX_PACK_STICKERS {
            return Err(StoreError::StickerUnreadable);
        }
        let mut ready = Vec::new();
        let mut seen = HashSet::new();
        let mut skipped = 0u32;
        for item in &pack.items {
            let path = stage.file(item.index);
            let Some(file) = Ready::read(&path, item.emoji.as_deref()) else {
                skipped = skipped.saturating_add(1);
                continue;
            };
            if seen.insert(file.sha256.clone()) {
                ready.push(file);
            } else {
                skipped = skipped.saturating_add(1);
            }
        }
        if ready.is_empty() {
            return Err(StoreError::StickerUnreadable);
        }
        let title = clean_title(&pack.title);
        let added = u32::try_from(ready.len()).unwrap_or(u32::MAX);
        let (dirs, faults) = (self.dirs(), self.inner.faults.clone());
        let (source, link_name, job_title) = (pack.source, pack.link_name, title.clone());
        self.write(move |conn| {
            write_pack(
                conn,
                &dirs,
                &faults,
                source,
                link_name.as_deref(),
                &job_title,
                &ready,
            )
        })?;
        drop(stage);
        Ok(StickerPackAdded {
            title,
            added,
            skipped,
        })
    }
}

/// A file that passed the probe, ready to be renamed into the library.
struct Ready {
    id: String,
    ext: &'static str,
    sha256: String,
    byte_size: i64,
    probe: StickerProbe,
    emoji: Option<String>,
    path: PathBuf,
}

impl Ready {
    fn read(path: &Path, emoji: Option<&str>) -> Option<Ready> {
        let probe = probe_sticker(path)?;
        let bytes = fs::read(path).ok()?;
        Some(Ready {
            id: Uuid::new_v4().to_string(),
            ext: probe.format.ext(),
            sha256: hex_lower(Sha256::digest(&bytes).as_slice()),
            byte_size: i64::try_from(bytes.len()).ok()?,
            probe,
            emoji: emoji.and_then(clean_emoji).map(str::to_owned),
            path: path.to_path_buf(),
        })
    }
}

fn write_pack(
    conn: &mut rusqlite::Connection,
    dirs: &Dirs,
    faults: &Faults,
    source: PackSource,
    link_name: Option<&str>,
    title: &str,
    ready: &[Ready],
) -> Result<(), StoreError> {
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(write_err)?;
    let old: Option<(String, i64)> = match link_name {
        Some(name) => tx
            .query_row(
                "SELECT id, position FROM sticker_pack WHERE link_name = ?1",
                [name],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(read_err)?,
        None => None,
    };
    let old_files = match &old {
        Some((id, _)) => pack_files(&tx, id)?,
        None => Vec::new(),
    };
    let position = match &old {
        Some((_, position)) => *position,
        None => next_position(&tx)?,
    };
    make_dir(&dirs.stickers()).map_err(io_error)?;
    let mut renamed: Vec<PathBuf> = Vec::new();
    for file in ready {
        let target = dirs
            .library_file(&format!("{}.{}", file.id, file.ext))
            .ok_or_else(|| StoreError::Internal("bad library file name".into()))?;
        if let Err(error) = fs::rename(&file.path, &target) {
            remove_all(&renamed);
            return Err(io_error(error));
        }
        renamed.push(target);
    }
    let written = (|| {
        faults.run_pack_after_rename();
        if let Some((old_id, _)) = &old {
            tx.execute("DELETE FROM sticker WHERE pack_id = ?1", [old_id])
                .map_err(write_err)?;
            tx.execute("DELETE FROM sticker_pack WHERE id = ?1", [old_id])
                .map_err(write_err)?;
        }
        let pack_id = Uuid::new_v4().to_string();
        tx.execute(
            "INSERT INTO sticker_pack (id, source, link_name, title, position) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![pack_id, source.as_str(), link_name, title, position],
        )
        .map_err(write_err)?;
        for (order, file) in ready.iter().enumerate() {
            insert_sticker(
                &tx,
                &pack_id,
                i64::try_from(order).unwrap_or(i64::MAX),
                file,
            )?;
        }
        faults.run_pack_before_commit();
        tx.commit().map_err(write_err)
    })();
    match written {
        Ok(()) => {
            sync_dir(&dirs.stickers());
            delete_library_files(dirs, faults, &old_files);
            Ok(())
        }
        Err(error) => {
            remove_all(&renamed);
            Err(error)
        }
    }
}

fn insert_sticker(
    tx: &Transaction<'_>,
    pack_id: &str,
    position: i64,
    file: &Ready,
) -> Result<(), StoreError> {
    tx.execute(
        "INSERT INTO sticker (id, pack_id, position, emoji, sha256, ext, byte_size, width, height) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        rusqlite::params![
            file.id,
            pack_id,
            position,
            file.emoji,
            file.sha256,
            file.ext,
            file.byte_size,
            file.probe.width,
            file.probe.height
        ],
    )
    .map_err(write_err)?;
    Ok(())
}

fn next_position(tx: &Transaction<'_>) -> Result<i64, StoreError> {
    tx.query_row(
        "SELECT COALESCE(MAX(position), -1) + 1 FROM sticker_pack",
        [],
        |row| row.get(0),
    )
    .map_err(read_err)
}

fn remove_all(paths: &[PathBuf]) {
    for path in paths {
        let _ = fs::remove_file(path);
    }
}

/// In the transaction of an import batch: the file of a sticker message (already placed in `attachments/`) is copied into the library
/// once per content, into the pack "From Telegram", which is made on first use. The copy comes first, the rows last; a file left without
/// its row by a failure is the sweep's. `placed` is the file the attachment owns: the library keeps a copy of its own.
pub(crate) fn add_to_export_pack(
    tx: &Transaction<'_>,
    dirs: &Dirs,
    placed: &Path,
    probe: &StickerProbe,
    sha256: &str,
    byte_size: i64,
    emoji: Option<&str>,
) -> Result<(), StoreError> {
    let existing: Option<String> = tx
        .query_row(
            "SELECT id FROM sticker_pack WHERE source = 'telegram_export'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(read_err)?;
    let pack_id = match existing {
        Some(id) => id,
        None => {
            let id = Uuid::new_v4().to_string();
            tx.execute(
                "INSERT INTO sticker_pack (id, source, link_name, title, position) VALUES (?1, 'telegram_export', NULL, ?2, ?3)",
                rusqlite::params![id, EXPORT_PACK_TITLE, next_position(tx)?],
            )
            .map_err(write_err)?;
            id
        }
    };
    let known: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sticker WHERE pack_id = ?1 AND sha256 = ?2)",
            (&pack_id, sha256),
            |row| row.get(0),
        )
        .map_err(read_err)?;
    if known {
        return Ok(());
    }
    let id = Uuid::new_v4().to_string();
    let target = dirs
        .library_file(&format!("{id}.{}", probe.format.ext()))
        .ok_or_else(|| StoreError::Internal("bad library file name".into()))?;
    make_dir(&dirs.stickers()).map_err(io_error)?;
    fs::copy(placed, &target).map_err(io_error)?;
    let position: i64 = tx
        .query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM sticker WHERE pack_id = ?1",
            [&pack_id],
            |row| row.get(0),
        )
        .map_err(read_err)?;
    let file = Ready {
        id,
        ext: probe.format.ext(),
        sha256: sha256.to_string(),
        byte_size,
        probe: *probe,
        emoji: emoji.and_then(clean_emoji).map(str::to_owned),
        path: placed.to_path_buf(),
    };
    if let Err(error) = insert_sticker(tx, &pack_id, position, &file) {
        let _ = fs::remove_file(&target);
        return Err(error);
    }
    Ok(())
}
