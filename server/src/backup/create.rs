//! `kinige-server backup <dir>` (ADR-0020). The order is the consistency argument:
//!
//! 1. The backup lock is taken exclusively, the clock is read (the **cut time**), a `REPEATABLE READ` snapshot is opened, the lock is
//!    let go (`store::lock`). A compaction or an account deletion is either in the snapshot with a stamp below the cut time, or not in
//!    it with a stamp at or above.
//! 2. The tables are copied out of that one snapshot, and the list of live blobs with them. Nothing else is read from the database.
//! 3. The objects of those blobs are copied. A blob is immutable after publication and a deleted blob stays in the store for the backup
//!    retention plus one interval, so every object a row of the snapshot refers to is there, whatever was published or deleted while
//!    the copy ran. A blob published after the snapshot is not in it and is not copied.
//! 4. `manifest.json` is written last, the directory is renamed from `.tmp-…` to `backup-<cut>`, and the root is synced. A directory
//!    without a manifest, or still named `.tmp-…`, is a backup that did not finish and is never used.

use super::BackupError;
use super::journal::Journals;
use super::manifest::{
    CREDENTIALS_FILE, DELETIONS_FILE, Entry, FORMAT, LINEAGE_FILE, LineageRow, MANIFEST_FILE,
    Manifest, TABLES, hash_file, table_path,
};
use super::rotate::newest_valid;
use crate::blobs::object_key;
use crate::blobs::objects::{ObjectError, Objects};
use crate::clock::Clock;
use crate::hashing::{Hasher, hex};
use crate::store::{self, lock, to_u64};
use deadpool_postgres::{Pool, Transaction};
use futures_util::StreamExt;
use libsodium_rs::random;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs::{self, File};
use tokio::io::AsyncWriteExt;
use tokio_postgres::IsolationLevel;

/// Writes a backup under `root` and returns its directory and manifest. On any failure nothing is left under `root`.
pub async fn create(
    pool: &Pool,
    objects: &Objects,
    clock: &dyn Clock,
    root: &Path,
) -> Result<(PathBuf, Manifest), BackupError> {
    fs::create_dir_all(root)
        .await
        .map_err(io("create the backup directory"))?;
    let tmp = root.join(format!(".tmp-{}", hex(&random::bytes(6))));
    let built = build(pool, objects, clock, root, &tmp).await;
    match built {
        Ok(manifest) => {
            let name = unique_name(root, manifest.cut_ms).await;
            let dir = root.join(name);
            fs::rename(&tmp, &dir)
                .await
                .map_err(io("rename the backup into place"))?;
            sync_dir(root).await;
            log::info!("backup written");
            Ok((dir, manifest))
        }
        Err(error) => {
            let _ = fs::remove_dir_all(&tmp).await;
            log::error!("backup failed");
            Err(error)
        }
    }
}

async fn build(
    pool: &Pool,
    objects: &Objects,
    clock: &dyn Clock,
    root: &Path,
    tmp: &Path,
) -> Result<Manifest, BackupError> {
    fs::create_dir_all(tmp.join("tables"))
        .await
        .map_err(io("create the backup directory"))?;
    let earlier = newest_valid(root).await;
    let mut client = pool.get().await?;
    let guard = pool.get().await?;
    lock::exclusive(&guard).await?;
    // The cut time is the database's clock (or the injected one of a test), raised above the newest cut of the lineage: the same
    // stamp a compaction, a deletion and a credential change get, read under the same lock (`store::lock`).
    let connection: &tokio_postgres::Client = &guard;
    let (cut_ms, opened) = match lock::stamp(connection, clock).await {
        Ok(cut_ms) => (cut_ms, open_snapshot(&mut client).await),
        Err(error) => (0, Err(error.into())),
    };
    let released = lock::release(&guard).await;
    drop(guard);
    let tx = opened?;
    released?;
    dump_and_copy(objects, tx, tmp, cut_ms, earlier).await
}

/// Everything after the snapshot is open: the tables out of the snapshot, then the objects, then the manifest.
async fn dump_and_copy(
    objects: &Objects,
    tx: Transaction<'_>,
    tmp: &Path,
    cut_ms: u64,
    earlier: Option<(PathBuf, Manifest)>,
) -> Result<Manifest, BackupError> {
    let mut files = Vec::new();
    for table in TABLES {
        files.push(dump_table(&tx, tmp, table).await?);
    }
    files.push(dump_lineage(&tx, tmp).await?);
    // The journals: read by the code the restore reads the installation's with, in the same snapshot.
    let journals = Journals::of_installation(&tx).await?;
    files.push(json_entry(tmp, DELETIONS_FILE, &journals.deletions).await?);
    files.push(json_entry(tmp, CREDENTIALS_FILE, &journals.credentials).await?);
    let live = live_blobs(&tx).await?;
    tx.commit().await?;
    let mut blobs = Vec::new();
    let reuse: HashMap<String, (PathBuf, Entry)> = earlier
        .map(|(dir, manifest)| {
            manifest
                .blobs
                .into_iter()
                .map(|entry| (entry.path.clone(), (dir.join(&entry.path), entry)))
                .collect()
        })
        .unwrap_or_default();
    for (account, blob, size) in live {
        blobs.push(copy_blob(objects, tmp, &reuse, &account, &blob, size).await?);
    }
    let manifest = Manifest {
        format: FORMAT,
        schema: store::MIGRATIONS.last().map_or(0, |(version, _)| *version),
        cut_ms,
        server_version: env!("CARGO_PKG_VERSION").to_string(),
        files,
        blobs,
    };
    write_file(
        &tmp.join(MANIFEST_FILE),
        &serde_json::to_vec_pretty(&manifest).map_err(|_| BackupError::Io("write the manifest"))?,
    )
    .await?;
    sync_dir(tmp).await;
    Ok(manifest)
}

/// A `REPEATABLE READ`, read-only transaction whose snapshot is taken now: the first statement fixes it.
async fn open_snapshot(
    client: &mut deadpool_postgres::Object,
) -> Result<Transaction<'_>, BackupError> {
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await?;
    tx.query_one("SELECT 1", &[]).await?;
    Ok(tx)
}

/// One table as text `COPY`, streamed into a file and hashed on the way.
async fn dump_table(tx: &Transaction<'_>, tmp: &Path, table: &str) -> Result<Entry, BackupError> {
    let path = table_path(table);
    let mut stream = Box::pin(
        tx.copy_out(format!("COPY {table} TO STDOUT").as_str())
            .await?,
    );
    let mut file = File::create(tmp.join(&path))
        .await
        .map_err(io("write a table"))?;
    let mut hasher = Hasher::new();
    let mut bytes = 0u64;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        hasher.update(&chunk);
        bytes = bytes.saturating_add(u64::try_from(chunk.len()).unwrap_or(0));
        file.write_all(&chunk).await.map_err(io("write a table"))?;
    }
    file.sync_all().await.map_err(io("sync a table"))?;
    Ok(Entry {
        path,
        bytes,
        hash: hasher.finish(),
    })
}

async fn dump_lineage(tx: &Transaction<'_>, tmp: &Path) -> Result<Entry, BackupError> {
    let rows = tx
        .query(
            "SELECT pos, generation, cut_ms FROM lineage ORDER BY pos",
            &[],
        )
        .await?;
    let lineage: Vec<LineageRow> = rows
        .iter()
        .map(|row| {
            Ok(LineageRow {
                pos: row.try_get("pos")?,
                generation: row.try_get::<_, i64>("generation")?.to_string(),
                cut_ms: to_u64(row.try_get("cut_ms")?),
            })
        })
        .collect::<Result<_, tokio_postgres::Error>>()?;
    json_entry(tmp, LINEAGE_FILE, &lineage).await
}

/// The blobs that are published and not deleted at the snapshot: `(account, blob, size)`.
async fn live_blobs(tx: &Transaction<'_>) -> Result<Vec<(String, String, u64)>, BackupError> {
    let rows = tx
        .query(
            "SELECT account_id, blob_id, size FROM blobs WHERE deleted_ms IS NULL ORDER BY account_id, blob_id",
            &[],
        )
        .await?;
    rows.iter()
        .map(|row| {
            Ok((
                row.try_get("account_id")?,
                row.try_get("blob_id")?,
                to_u64(row.try_get("size")?),
            ))
        })
        .collect::<Result<_, tokio_postgres::Error>>()
        .map_err(BackupError::from)
}

/// One blob into the backup: a hard link to the same blob in the newest earlier backup when there is one (the object is immutable, so
/// the file is the same), otherwise a copy from the store.
async fn copy_blob(
    objects: &Objects,
    tmp: &Path,
    reuse: &HashMap<String, (PathBuf, Entry)>,
    account: &str,
    blob: &str,
    size: u64,
) -> Result<Entry, BackupError> {
    let path = format!("blobs/{}", object_key(account, blob));
    let target = tmp.join(&path);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .await
            .map_err(io("create a blob directory"))?;
    }
    // Linked only after the earlier backup's file has been hashed and found to be what that backup's manifest says: a damaged file
    // would otherwise be linked into every later backup and make all of them unrestorable (round 1 of the review of S4c-2). On a
    // mismatch the object is copied from the store.
    if let Some((old, entry)) = reuse.get(&path)
        && entry.bytes == size
        && hash_file(old)
            .await
            .is_ok_and(|(len, hash)| len == entry.bytes && hash == entry.hash)
        && fs::hard_link(old, &target).await.is_ok()
    {
        return Ok(Entry {
            path,
            bytes: entry.bytes,
            hash: entry.hash.clone(),
        });
    }
    let (bytes, hash) = objects
        .download(&object_key(account, blob), &target)
        .await
        .map_err(|error| {
            match error {
                ObjectError::NotFound => log::error!("a published blob is missing from the store"),
                ObjectError::Failed => log::error!("a blob could not be copied"),
            }
            BackupError::Store
        })?;
    if bytes != size {
        log::error!("a blob in the store has another size than its row");
        return Err(BackupError::Store);
    }
    Ok(Entry { path, bytes, hash })
}

/// `backup-<cut time>`, with a suffix if two backups were cut in the same millisecond.
async fn unique_name(root: &Path, cut_ms: u64) -> String {
    let base = format!("backup-{cut_ms:013}");
    let mut name = base.clone();
    let mut n = 1;
    while fs::try_exists(root.join(&name)).await.unwrap_or(false) {
        name = format!("{base}-{n}");
        n += 1;
    }
    name
}

async fn json_entry<T: serde::Serialize>(
    tmp: &Path,
    name: &str,
    value: &T,
) -> Result<Entry, BackupError> {
    let bytes = serde_json::to_vec(value).map_err(|_| BackupError::Io("write a dump file"))?;
    write_file(&tmp.join(name), &bytes).await?;
    let mut hasher = Hasher::new();
    hasher.update(&bytes);
    Ok(Entry {
        path: name.to_string(),
        bytes: u64::try_from(bytes.len()).unwrap_or(0),
        hash: hasher.finish(),
    })
}

async fn write_file(path: &Path, bytes: &[u8]) -> Result<(), BackupError> {
    let mut file = File::create(path).await.map_err(io("write a file"))?;
    file.write_all(bytes).await.map_err(io("write a file"))?;
    file.sync_all().await.map_err(io("sync a file"))
}

/// Makes a directory's entries durable (Unix; elsewhere a directory cannot be opened for it).
async fn sync_dir(dir: &Path) {
    if let Ok(handle) = File::open(dir).await {
        let _ = handle.sync_all().await;
    }
}

fn io(what: &'static str) -> impl Fn(std::io::Error) -> BackupError {
    move |error| {
        log::error!("backup: {what}: {:?}", error.kind());
        BackupError::Io(what)
    }
}
