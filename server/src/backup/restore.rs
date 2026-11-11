//! `kinige-server restore <backup directory>` (ADR-0020). One code path for "in place" and "into a clean installation", with the server
//! stopped: a running server holds a session advisory lock (`store::lock::ServerLock`) and a restore that cannot take it exclusively
//! refuses to run. The target database is migrated (a clean one gets its schema), then in **one transaction** its tables are emptied and
//! loaded from the dump, a fresh generation is appended to the lineage with the backup's cut time, the journals are re-applied (accounts
//! deleted after the backup are deleted again, credentials changed after it are put back), and the sessions are gone.
//!
//! **The restore's lock and its transaction are on one connection** (KINIG-102). A session lock dies with its connection; if the
//! transaction were on another one, a server could start in the gap, take the lock and serve while the restore went on replacing the
//! tables under it. On one connection, the lock and the transaction end together: the transaction rolls back. Whether the connection is
//! lost is told by a probe statement (`SELECT 1`) after a failure, not by `Client::is_closed`, which stays false for a while after the
//! server terminated the backend. A loss before the commit is [`BackupError::LockLost`] (the objects copied are removed); a loss during
//! the commit is [`BackupError::OutcomeUnknown`] (they are kept); a loss after it only ends the scan for orphan objects.
//!
//! Before the database is touched the files of the dump are checked against the manifest; a **blob** file that is missing or damaged
//! does not stop the restore (it would make every backup unrestorable for one bad file): its row is marked deleted, its device finds
//! `missing`, and the restore ends with an error line and a non-zero exit. The blobs are then copied into the store (an object that is
//! already there is kept only if its hash is the manifest's; objects a failed restore put are removed again). After the commit, every
//! object that no restored row refers to is adopted as a deleted blob: it leaves the store after the retention like any other.

use super::BackupError;
use super::journal::{Applied, Journals, apply, mark_damaged};
use super::manifest::{Entry, LINEAGE_FILE, LineageRow, Manifest, TABLES, table_path};
use super::outcome::{Failed, settle};
use crate::blobs::objects::Objects;
use crate::blobs::parse_key;
use crate::blobs::sweep::adopt_orphans;
use crate::clock::Clock;
use crate::rules::pull::MAX_LINEAGE;
use crate::store::{self, lock, to_i64};
use axum::body::Bytes;
use deadpool_postgres::{Object, Pool, Transaction};
use futures_util::SinkExt;
use libsodium_rs::random;
use std::path::Path;
use tokio::fs::{self, File};
use tokio::io::AsyncReadExt;

/// How far ahead of the database's clock a backup's cut time may be before the restore says so (round 2 of the review of S4c-2).
const FUTURE_CUT_MS: u64 = 60 * 60 * 1000;

/// What a restore did, for the operator's log and the tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// The generation the restore appended to the lineage.
    pub generation: u64,
    pub cut_ms: u64,
    /// Accounts removed again because the deletion journal says they were deleted after the cut time.
    pub deletions_applied: usize,
    /// Accounts whose credentials were put back to what the credential journal says they were after the cut time.
    pub credentials_applied: usize,
    /// Objects in the store that no restored row refers to, now deleted blobs.
    pub orphans: usize,
    /// The scan for such objects failed after the commit. The restore is done; the next sweep adopts them.
    pub orphan_scan_failed: bool,
    /// Blobs of the backup whose file was missing or damaged: their rows are marked deleted (`missing` for a device). The command
    /// reports it as an error and exits non-zero, with the database restored.
    pub damaged_blobs: usize,
}

pub async fn restore(
    pool: &Pool,
    objects: &Objects,
    clock: &dyn Clock,
    dir: &Path,
) -> Result<Report, BackupError> {
    // Held for the whole restore, and the load transaction runs on this same connection: a server on this database holds the same lock,
    // shared.
    let mut guard = pool.get().await?;
    if !lock::try_restore(&guard).await? {
        return Err(BackupError::ServerRunning);
    }
    let result = run(&mut guard, pool, objects, clock, dir).await;
    let _ = lock::release_restore(&guard).await;
    result
}

async fn run(
    guard: &mut Object,
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
    manifest.verify_dump(dir).await?;
    let damaged = damaged_for_good(objects, &manifest, dir).await;
    // Everything that can be judged without the database is judged first: a dump file that is valid by its hash and wrong in content
    // is refused here, before an object is copied.
    let lineage = parse_lineage(read_json(&dir.join(LINEAGE_FILE)).await?)?;
    let mut journals = Journals::of_backup(dir, &manifest).await?;
    journals.extend(Journals::of_newest_neighbour(dir).await);
    store::migrate(pool).await?;
    let mut copied = Vec::new();
    let plan = Plan {
        dir,
        manifest: &manifest,
        lineage: &lineage,
        damaged: &damaged,
    };
    let loaded = load(guard, objects, clock, &plan, journals, &mut copied).await;
    let (generation, applied, now_ms) = match loaded {
        Ok(done) => done,
        Err(failed) => return Err(settle(guard, objects, copied, failed).await),
    };
    // The restore is done; a failure of this scan is a warning and not a failure of the restore.
    let (orphans, orphan_scan_failed) = match adopt_orphans(pool, objects, now_ms, 0).await {
        Ok(count) => (count, false),
        Err(_) => {
            log::warn!(
                "restore done, but the scan for objects without a row failed; the next sweep does it"
            );
            (0, true)
        }
    };
    if !damaged.is_empty() {
        log::error!(
            "{} blobs of the backup were missing or damaged: their rows are marked deleted and their devices will find them missing",
            damaged.len()
        );
    }
    log::info!(
        "restore done, {} deletions and {} credential changes applied again, {orphans} orphan objects adopted",
        applied.deletions,
        applied.credentials
    );
    Ok(Report {
        generation,
        cut_ms: manifest.cut_ms,
        deletions_applied: applied.deletions,
        credentials_applied: applied.credentials,
        orphans,
        orphan_scan_failed,
        damaged_blobs: damaged.len(),
    })
}

/// `(account_id, blob_id)` of the blobs whose file in the backup is missing or damaged **and** which the store does not hold intact
/// either. In place, the store usually still has the object the backup's file was a copy of: its row is kept (round 2 of the review),
/// and the restore does not copy over it (`copy_blob_in` finds the manifest's hash). Only a blob that exists nowhere is marked deleted.
async fn damaged_for_good(
    objects: &Objects,
    manifest: &Manifest,
    dir: &Path,
) -> Vec<(String, String)> {
    let mut gone = Vec::new();
    for path in manifest.damaged_blobs(dir).await {
        let Some(key) = path.strip_prefix("blobs/") else {
            continue;
        };
        let Some(ids) = parse_key(key) else {
            continue;
        };
        let held = objects.hash_of(key).await.ok().flatten();
        let intact = manifest.blobs.iter().any(|entry| {
            entry.path == path
                && held
                    .as_ref()
                    .is_some_and(|(size, hash)| *size == entry.bytes && *hash == entry.hash)
        });
        if !intact {
            gone.push(ids);
        }
    }
    gone
}

/// What a restore loads, checked.
struct Plan<'a> {
    dir: &'a Path,
    manifest: &'a Manifest,
    lineage: &'a [Gen],
    /// `(account_id, blob_id)` of the blobs whose file is missing or damaged.
    damaged: &'a [(String, String)],
}

/// The blobs into the store, then the database in one transaction on the lock's connection. Returns the new generation, what the
/// journals did and the time.
async fn load(
    guard: &mut Object,
    objects: &Objects,
    clock: &dyn Clock,
    plan: &Plan<'_>,
    journals: Journals,
    copied: &mut Vec<String>,
) -> Result<(u64, Applied, u64), Failed> {
    let skipped = |entry: &Entry| {
        plan.damaged.iter().any(|(account, blob)| {
            entry.path.strip_prefix("blobs/") == Some(format!("{account}/{blob}").as_str())
        })
    };
    for entry in plan.manifest.blobs.iter().filter(|e| !skipped(e)) {
        if let Some(key) = copy_blob_in(objects, plan.dir, entry).await? {
            copied.push(key);
        }
    }
    let tx = guard.transaction().await?;
    let (generation, applied, now_ms) = replace_tables(&tx, clock, plan, journals).await?;
    tx.commit().await.map_err(|error| Failed {
        error: error.into(),
        at_commit: true,
    })?;
    Ok((generation, applied, now_ms))
}

/// Everything the restore does to the tables, in the open transaction.
async fn replace_tables(
    tx: &Transaction<'_>,
    clock: &dyn Clock,
    plan: &Plan<'_>,
    mut journals: Journals,
) -> Result<(u64, Applied, u64), BackupError> {
    tx.execute("SELECT pg_advisory_xact_lock($1)", &[&lock::BACKUP_LOCK])
        .await?;
    // The journals of the installation being replaced, read before it is emptied: what happened since the backup is in them.
    journals.extend(Journals::of_installation(tx).await?);
    tx.batch_execute(
        "TRUNCATE accounts, envelopes, ops, entity_known, purged_entities, drops, sessions, blobs, deletions, credential_changes, lineage CASCADE",
    )
    .await?;
    for table in TABLES {
        load_table(tx, plan.dir, table).await?;
    }
    let generation = append_generation(tx, plan.lineage, plan.manifest.cut_ms).await?;
    // After the generation is in: the stamp is raised above the newest cut time of the lineage, the backup's included, whatever this
    // host's clock says.
    let now_ms = lock::stamp(&**tx, clock).await?;
    // A cut time well ahead of this database's own clock (a backup from a host whose clock was wrong, or a restore on a host that is
    // behind) makes the stamps run ahead of real time until the clock catches up; ages are not affected (they use the unfloored
    // clock, `lock::now`), but an operator should know. Kind only, no ids.
    if plan.manifest.cut_ms > lock::now(&**tx, clock).await?.saturating_add(FUTURE_CUT_MS) {
        log::error!(
            "the backup's cut time is more than an hour ahead of this database's clock: stamps run ahead of real time until it catches up"
        );
    }
    let applied = apply(tx, &journals, plan.manifest.cut_ms, now_ms).await?;
    mark_damaged(tx, plan.damaged, now_ms).await?;
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
                cut_ms: i64::try_from(row.cut_ms).map_err(|_| {
                    BackupError::Manifest("a cut time in lineage.json is too large")
                })?,
            })
        })
        .collect()
}

/// Puts a backed-up object into the store unless the store already has an object under that key with the **manifest's hash** (an
/// in-place restore): the same size is not enough, a damaged object of the right size must be repaired. Returns the key if it put one.
async fn copy_blob_in(
    objects: &Objects,
    dir: &Path,
    entry: &Entry,
) -> Result<Option<String>, BackupError> {
    let key = entry.path.strip_prefix("blobs/").unwrap_or(&entry.path);
    let held = objects.hash_of(key).await.map_err(|_| BackupError::Store)?;
    if held.is_some_and(|(size, hash)| size == entry.bytes && hash == entry.hash) {
        return Ok(None);
    }
    objects
        .put_file(key, &dir.join(&entry.path))
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

/// The restored lineage and one generation more: a random 63-bit id not in it, cut at the backup's time (ADR-0001 §6.3). The lineage is
/// kept to its newest `MAX_LINEAGE` generations: one restore adds one, and a device sends its lineage with every pull.
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
    let keep = i64::try_from(MAX_LINEAGE).unwrap_or(64);
    tx.execute(
        "DELETE FROM lineage WHERE pos <= (SELECT pos FROM lineage ORDER BY pos DESC OFFSET $1 LIMIT 1)",
        &[&keep],
    )
    .await?;
    Ok(u64::try_from(generation).unwrap_or(0))
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
