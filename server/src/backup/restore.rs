//! `kinige-server restore <backup directory>` (ADR-0020). One code path for "in place" and "into a clean installation": the target
//! database is migrated (a clean one gets its schema), then in **one transaction** its tables are emptied and loaded from the dump, a
//! fresh generation is appended to the lineage with the backup's cut time, the deletions newer than the cut time are applied again, and
//! the sessions are gone. The server must be stopped: nothing here coordinates with a running one beyond the backup lock.
//!
//! Before the database is touched the whole directory is checked against its manifest, then the blobs are copied into the store
//! (objects never change, so extra ones are harmless and a failed restore has changed nothing a client can see). After the commit, every
//! object that no restored row refers to is adopted as a deleted blob: it leaves the store after the retention like any other.

use super::BackupError;
use super::manifest::{
    DELETIONS_FILE, Deletion, LINEAGE_FILE, LineageRow, Manifest, TABLES, table_path,
};
use crate::blobs::objects::Objects;
use crate::blobs::sweep::adopt_orphans;
use crate::clock::Clock;
use crate::hashing::{account_hash, hex};
use crate::store::{self, blobs, lock, to_i64};
use axum::body::Bytes;
use deadpool_postgres::{Pool, Transaction};
use futures_util::SinkExt;
use libsodium_rs::random;
use std::collections::HashMap;
use std::path::Path;
use tokio::fs::{self, File};
use tokio::io::AsyncReadExt;

/// What a restore did, for the operator's log and the tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// The generation the restore appended to the lineage.
    pub generation: u64,
    pub cut_ms: u64,
    /// Accounts removed again because the deletion journal says they were deleted after the cut time.
    pub deletions_applied: usize,
    /// Objects in the store that no restored row refers to, now deleted blobs.
    pub orphans: usize,
}

pub async fn restore(
    pool: &Pool,
    objects: &Objects,
    clock: &dyn Clock,
    dir: &Path,
) -> Result<Report, BackupError> {
    let manifest = Manifest::read(dir).await?;
    let server = store::MIGRATIONS.last().map_or(0, |(version, _)| *version);
    if manifest.schema != server {
        return Err(BackupError::Schema {
            backup: manifest.schema,
            server,
        });
    }
    manifest.verify(dir).await?;
    // Everything that can be judged without the database is judged first: a dump file that is valid by its hash and wrong in content
    // is refused here, before an object is copied.
    let lineage = parse_lineage(read_json(&dir.join(LINEAGE_FILE)).await?)?;
    let mut journal = checked(read_json(&dir.join(DELETIONS_FILE)).await?)?;
    // A neighbour with a journal that is not one is passed over: it must not block the restore of this backup.
    journal.extend(checked(newest_other_journal(dir).await).unwrap_or_default());
    store::migrate(pool).await?;
    let mut copied = Vec::new();
    let loaded = load(
        pool,
        objects,
        clock.now_ms(),
        dir,
        &manifest,
        &lineage,
        journal,
        &mut copied,
    )
    .await;
    let (generation, deletions_applied, now_ms) = match loaded {
        Ok(done) => done,
        Err(error) => {
            // The objects this restore put into the store are removed again; the database rolled back with its transaction.
            for key in copied {
                let _ = objects.delete(&key).await;
            }
            return Err(error);
        }
    };
    let orphans = adopt_orphans(pool, objects, now_ms, 0).await?;
    log::info!(
        "restore done, {deletions_applied} deletions applied, {orphans} orphan objects adopted"
    );
    Ok(Report {
        generation,
        cut_ms: manifest.cut_ms,
        deletions_applied,
        orphans,
    })
}

/// The blobs into the store, then the database in one transaction. Returns the new generation, the deletions applied again and the time.
#[allow(clippy::too_many_arguments)]
async fn load(
    pool: &Pool,
    objects: &Objects,
    now_ms: u64,
    dir: &Path,
    manifest: &Manifest,
    lineage: &[Gen],
    mut journal: Vec<Deletion>,
    copied: &mut Vec<String>,
) -> Result<(u64, usize, u64), BackupError> {
    for entry in &manifest.blobs {
        if let Some(key) = copy_blob_in(objects, dir, &entry.path, entry.bytes).await? {
            copied.push(key);
        }
    }
    let mut client = pool.get().await?;
    let tx = client.transaction().await?;
    tx.execute("SELECT pg_advisory_xact_lock($1)", &[&lock::BACKUP_LOCK])
        .await?;
    // The journal of the installation being replaced, read before it is emptied: the deletions made since the backup are in it.
    journal.extend(current_journal(&tx).await?);
    tx.batch_execute(
        "TRUNCATE accounts, envelopes, ops, entity_known, purged_entities, drops, sessions, blobs, deletions, lineage CASCADE",
    )
    .await?;
    for table in TABLES {
        load_table(&tx, dir, table).await?;
    }
    let generation = append_generation(&tx, lineage, manifest.cut_ms).await?;
    let applied = apply_journal(&tx, &journal, manifest.cut_ms, now_ms).await?;
    tx.commit().await?;
    Ok((generation, applied, now_ms))
}

/// One row of `lineage.json`, parsed.
struct Gen {
    pos: i32,
    generation: i64,
    cut_ms: i64,
}

fn parse_lineage(rows: Vec<LineageRow>) -> Result<Vec<Gen>, BackupError> {
    rows.into_iter()
        .map(|row| {
            Ok(Gen {
                pos: row.pos,
                generation: row.generation.parse().map_err(|_| {
                    BackupError::Manifest("a generation in lineage.json is not a number")
                })?,
                cut_ms: i64::try_from(row.cut_ms)
                    .map_err(|_| BackupError::Manifest("a cut time in lineage.json is too large"))?,
            })
        })
        .collect()
}

/// The journal entries whose hash is a hash and whose time is a time; any other entry makes the whole journal unusable.
fn checked(journal: Vec<Deletion>) -> Result<Vec<Deletion>, BackupError> {
    let fine = journal.iter().all(|entry| {
        entry.account_hash.len() == 64
            && crate::hashing::unhex(&entry.account_hash).is_some()
            && i64::try_from(entry.deleted_ms).is_ok()
    });
    if fine {
        Ok(journal)
    } else {
        Err(BackupError::Manifest("a journal entry is not a hash"))
    }
}

/// Puts a backed-up object into the store unless the store already has an object of that size under that key (an in-place restore).
/// Returns the key if it put one.
async fn copy_blob_in(
    objects: &Objects,
    dir: &Path,
    path: &str,
    bytes: u64,
) -> Result<Option<String>, BackupError> {
    let key = path.strip_prefix("blobs/").unwrap_or(path);
    if objects.size(key).await.map_err(|_| BackupError::Store)? == Some(bytes) {
        return Ok(None);
    }
    objects
        .put_file(key, &dir.join(path))
        .await
        .map_err(|_| BackupError::Store)?;
    Ok(Some(key.to_string()))
}

/// `COPY <table> FROM STDIN` of the dump file, in one transaction with everything else.
async fn load_table(tx: &Transaction<'_>, dir: &Path, table: &str) -> Result<(), BackupError> {
    let sink = tx
        .copy_in::<_, Bytes>(format!("COPY {table} FROM STDIN").as_str())
        .await?;
    let mut sink = Box::pin(sink);
    let mut file = File::open(dir.join(table_path(table)))
        .await
        .map_err(|_| BackupError::Missing(table_path(table)))?;
    let mut buffer = vec![0u8; 1 << 20];
    loop {
        let read = file
            .read(&mut buffer)
            .await
            .map_err(|_| BackupError::Io("read a table"))?;
        let Some(filled) = buffer.get(..read).filter(|filled| !filled.is_empty()) else {
            break;
        };
        sink.send(Bytes::copy_from_slice(filled)).await?;
    }
    sink.as_mut().finish().await?;
    Ok(())
}

/// The restored lineage and one generation more: a random 63-bit id not in it, cut at the backup's time (ADR-0001 §6.3).
async fn append_generation(
    tx: &Transaction<'_>,
    lineage: &[Gen],
    cut_ms: u64,
) -> Result<u64, BackupError> {
    let mut held = Vec::new();
    let mut last_pos = 0;
    for row in lineage {
        tx.execute(
            "INSERT INTO lineage (pos, generation, cut_ms) VALUES ($1, $2, $3)",
            &[&row.pos, &row.generation, &row.cut_ms],
        )
        .await?;
        held.push(row.generation);
        last_pos = last_pos.max(row.pos);
    }
    let mut generation = random_generation();
    while held.contains(&generation) {
        generation = generation.wrapping_add(1).max(1);
    }
    tx.execute(
        "INSERT INTO lineage (pos, generation, cut_ms) VALUES ($1, $2, $3)",
        &[
            &last_pos.saturating_add(1),
            &generation,
            &to_i64(cut_ms, "cut")?,
        ],
    )
    .await?;
    Ok(u64::try_from(generation).unwrap_or(0))
}

/// Re-applies the journal: for each entry newer than the cut time whose account is in the restored data, the account is deleted again
/// (its blobs get the delayed deletion), and every entry is kept in the restored journal (the earliest time per account).
async fn apply_journal(
    tx: &Transaction<'_>,
    journal: &[Deletion],
    cut_ms: u64,
    now_ms: u64,
) -> Result<usize, BackupError> {
    let mut earliest: HashMap<&str, u64> = HashMap::new();
    for entry in journal {
        let slot = earliest
            .entry(entry.account_hash.as_str())
            .or_insert(u64::MAX);
        *slot = (*slot).min(entry.deleted_ms);
    }
    for (hash, deleted_ms) in &earliest {
        let Some(bytes) = crate::hashing::unhex(hash) else {
            return Err(BackupError::Manifest("a journal entry is not a hash"));
        };
        tx.execute(
            "INSERT INTO deletions (account_hash, deleted_ms) VALUES ($1, $2) ON CONFLICT (account_hash) DO NOTHING",
            &[&bytes, &to_i64(*deleted_ms, "deleted")?],
        )
        .await?;
    }
    let mut applied = 0;
    for row in tx.query("SELECT id FROM accounts", &[]).await? {
        let id: String = row.try_get("id")?;
        let Some(hash) = account_hash(&id) else {
            continue;
        };
        // `>=`: a deletion stamped at the cut time itself may not be in the dump (`store::lock`).
        if earliest
            .get(hex(&hash).as_str())
            .is_some_and(|ms| *ms >= cut_ms)
        {
            blobs::mark_account_deleted(tx, &id, now_ms).await?;
            tx.execute("DELETE FROM accounts WHERE id = $1", &[&id])
                .await?;
            applied += 1;
        }
    }
    Ok(applied)
}

/// The journal of the installation in the target database (empty for a clean one).
async fn current_journal(tx: &Transaction<'_>) -> Result<Vec<Deletion>, BackupError> {
    let rows = tx
        .query("SELECT account_hash, deleted_ms FROM deletions", &[])
        .await?;
    rows.iter()
        .map(|row| {
            Ok(Deletion {
                account_hash: hex(&row.try_get::<_, Vec<u8>>("account_hash")?),
                deleted_ms: crate::store::to_u64(row.try_get("deleted_ms")?),
            })
        })
        .collect::<Result<_, tokio_postgres::Error>>()
        .map_err(BackupError::from)
}

/// The journal of the newest backup next to `dir` (the backup being restored is not counted): when an older backup is restored into a
/// clean installation this is where the later deletions are. A neighbour that is not a valid backup is passed over.
async fn newest_other_journal(dir: &Path) -> Vec<Deletion> {
    let Some(parent) = dir.parent() else {
        return Vec::new();
    };
    let Ok(mut entries) = fs::read_dir(parent).await else {
        return Vec::new();
    };
    let mut best: Option<(u64, Vec<Deletion>)> = None;
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if path == dir {
            continue;
        }
        let Ok(manifest) = Manifest::read(&path).await else {
            continue;
        };
        if best
            .as_ref()
            .is_some_and(|(cut, _)| *cut >= manifest.cut_ms)
        {
            continue;
        }
        if let Ok(journal) = read_json::<Vec<Deletion>>(&path.join(DELETIONS_FILE)).await {
            best = Some((manifest.cut_ms, journal));
        }
    }
    best.map(|(_, journal)| journal).unwrap_or_default()
}

async fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, BackupError> {
    let bytes = fs::read(path).await.map_err(|_| {
        BackupError::Missing(
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
        )
    })?;
    serde_json::from_slice(&bytes).map_err(|_| BackupError::Manifest("a dump file is not valid"))
}

fn random_generation() -> i64 {
    let bytes = random::bytes(8);
    let value = <[u8; 8]>::try_from(bytes.as_slice()).map_or(1, u64::from_be_bytes);
    i64::try_from(value >> 1).unwrap_or(1).max(1)
}
