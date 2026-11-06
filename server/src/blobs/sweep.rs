//! The delayed physical deletion of blobs (ADR-0001 §7, PRD §10.1, ADR-0020). A deleted blob is only marked; its object leaves the store
//! once it is older than the backup retention plus one backup interval, so every backup that can still be restored finds its blobs.
//! The same pass removes unfinished uploads nobody has touched for `orphan_upload_ms`, and adopts objects that no row refers to (a crash
//! between the `put` and the row) as deleted blobs, which then go the same delayed way. It also removes the retired entries of the
//! credential journal on the same window (`store::credentials::prune`): a retired password's wrap lives as long as a deleted blob.

use super::objects::{Listed, Objects};
use super::{Blobs, object_key, parse_key};
use crate::clock::Clock;
use crate::config::Config;
use crate::error::ApiError;
use crate::rules::blob::removable;
use crate::store::{blobs, credentials, lock};
use deadpool_postgres::Pool;
use std::collections::HashSet;
use std::time::Duration;

/// What one pass did, for the log and the tests.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Swept {
    /// Objects removed from the store and their rows.
    pub removed: usize,
    /// Unfinished uploads removed.
    pub uploads: usize,
    /// Objects without a row that were adopted as deleted blobs.
    pub adopted: usize,
    /// Retired entries of the credential journal removed (older than the retention plus one interval, like a deleted blob).
    pub journal: u64,
}

pub async fn run(
    pool: &Pool,
    blobs: &Blobs,
    config: &Config,
    clock: &dyn Clock,
) -> Result<Swept, ApiError> {
    // The age of a deleted blob and of a journal entry is judged on the clock itself, never on the stamp's floor (round 2).
    let now_ms = {
        let client = pool.get().await?;
        lock::now(&**client, clock).await?
    };
    let mut swept = Swept::default();
    let keep_ms = config
        .backup_retention_ms
        .saturating_add(config.backup_interval_ms);
    swept.journal = credentials::prune(pool, now_ms.saturating_sub(keep_ms)).await?;
    for row in blobs::deleted_before(pool, now_ms.saturating_sub(keep_ms)).await? {
        let Some(deleted_ms) = row.deleted_ms else {
            continue;
        };
        if !removable(
            deleted_ms,
            now_ms,
            config.backup_retention_ms,
            config.backup_interval_ms,
        ) {
            continue;
        }
        let key = object_key(&row.account_id, &row.blob_id);
        if blobs.objects.delete(&key).await.is_ok() {
            blobs::remove_row(pool, &row.account_id, &row.blob_id).await?;
            swept.removed += 1;
        }
    }
    swept.uploads = blobs
        .staging
        .remove_older_than(Duration::from_millis(config.orphan_upload_ms))
        .await;
    swept.adopted = adopt_orphans(pool, &blobs.objects, now_ms, config.orphan_upload_ms).await?;
    Ok(swept)
}

/// Objects in the store that no row refers to and that are older than `min_age_ms` become rows marked deleted now. A restore calls this
/// with an age of 0: nothing is being published while it runs.
pub async fn adopt_orphans(
    pool: &Pool,
    objects: &Objects,
    now_ms: u64,
    min_age_ms: u64,
) -> Result<usize, ApiError> {
    let Ok(listed) = objects.list().await else {
        log::warn!("the object store could not be listed");
        return Ok(0);
    };
    let known: HashSet<String> = blobs::all(pool)
        .await?
        .into_iter()
        .map(|row| object_key(&row.account_id, &row.blob_id))
        .collect();
    let mut adopted = 0;
    for Listed {
        key,
        size,
        modified_ms,
    } in listed
    {
        if known.contains(&key) || now_ms.saturating_sub(modified_ms) < min_age_ms {
            continue;
        }
        if let Some((account, blob)) = parse_key(&key) {
            blobs::adopt_orphan(pool, &account, &blob, size, now_ms).await?;
            adopted += 1;
        }
    }
    Ok(adopted)
}
