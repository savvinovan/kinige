//! The `blobs` table (ADR-0020): one row per published blob, with the size and, once the blob is deleted, the time it was marked. The
//! bytes are in the object store; an unfinished upload has no row.

use super::{to_i64, to_u64};
use crate::error::ApiError;
use crate::rules::blob::Known;
use deadpool_postgres::{Pool, Transaction};

/// A row as the sweep and the restore read it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub account_id: String,
    pub blob_id: String,
    pub size: u64,
    pub deleted_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Published {
    /// A new row.
    New,
    /// The id was published already (or deleted): the caller's object is the one to drop, if it was put.
    AlreadyThere(Known),
    /// The account does not exist any more (deleted while the upload ran).
    AccountGone,
}

/// What the table says about an id, with the size of a published one.
pub async fn state(
    pool: &Pool,
    account: &str,
    blob: &str,
) -> Result<(Known, Option<u64>), ApiError> {
    let client = pool.get().await?;
    let row = client
        .query_opt(
            "SELECT size, deleted_ms FROM blobs WHERE account_id = $1 AND blob_id = $2",
            &[&account, &blob],
        )
        .await?;
    let Some(row) = row else {
        return Ok((Known::Absent, None));
    };
    let size = to_u64(row.try_get("size")?);
    let deleted: Option<i64> = row.try_get("deleted_ms")?;
    Ok(match deleted {
        Some(_) => (Known::Deleted, None),
        None => (Known::Published, Some(size)),
    })
}

/// Records a published blob, once, for an account that still exists.
pub async fn publish(
    pool: &Pool,
    account: &str,
    blob: &str,
    size: u64,
    now_ms: u64,
) -> Result<Published, ApiError> {
    let mut client = pool.get().await?;
    let tx = client.transaction().await?;
    let exists = tx
        .query_opt(
            "SELECT deleted_ms FROM blobs WHERE account_id = $1 AND blob_id = $2 FOR UPDATE",
            &[&account, &blob],
        )
        .await?;
    if let Some(row) = exists {
        let deleted: Option<i64> = row.try_get("deleted_ms")?;
        let known = deleted.map_or(Known::Published, |_| Known::Deleted);
        return Ok(Published::AlreadyThere(known));
    }
    let inserted = tx
        .execute(
            "INSERT INTO blobs (account_id, blob_id, size, published_ms) \
             SELECT id, $2, $3, $4 FROM accounts WHERE id = $1 ON CONFLICT DO NOTHING",
            &[
                &account,
                &blob,
                &to_i64(size, "size")?,
                &to_i64(now_ms, "clock")?,
            ],
        )
        .await?;
    if inserted == 1 {
        tx.commit().await?;
        return Ok(Published::New);
    }
    // Nothing was inserted: either the account is gone, or the row appeared between the look above and the insert (another process
    // published the id). The two must not be confused: the caller deletes the object of an account that is gone and never the object
    // of a row that exists (round 1 of the review of S4c-2).
    let row_exists = tx
        .query_opt(
            "SELECT deleted_ms FROM blobs WHERE account_id = $1 AND blob_id = $2",
            &[&account, &blob],
        )
        .await?;
    tx.commit().await?;
    Ok(match row_exists {
        Some(row) => {
            let deleted: Option<i64> = row.try_get("deleted_ms")?;
            Published::AlreadyThere(deleted.map_or(Known::Published, |_| Known::Deleted))
        }
        None => Published::AccountGone,
    })
}

/// Reserves an upload in the quota **atomically**: the account's row is locked (the lock of a push, so a push of the account waits
/// for it for the time of a file creation and no longer), the published blobs are summed, and `open` creates the staging file whose name
/// declares the size, all before the lock is let go; a second reservation of the same account then sees this one. Without the lock two
/// concurrent starts each saw the room the other was about to take. `Quota` if the account's room is not enough, `Auth` if it is gone.
pub async fn reserve<T, F, Fut>(
    pool: &Pool,
    account: &str,
    size: u64,
    quota: u64,
    declared_by_others: impl FnOnce() -> F,
    open: impl FnOnce() -> Fut,
) -> Result<T, ApiError>
where
    F: std::future::Future<Output = u64>,
    Fut: std::future::Future<Output = Result<T, ApiError>>,
{
    let mut client = pool.get().await?;
    let tx = client.transaction().await?;
    tx.query_opt(
        "SELECT 1 FROM accounts WHERE id = $1 FOR UPDATE",
        &[&account],
    )
    .await?
    .ok_or(ApiError::Auth)?;
    let live = tx
        .query_one(
            "SELECT COALESCE(sum(size), 0)::bigint FROM blobs WHERE account_id = $1 AND deleted_ms IS NULL",
            &[&account],
        )
        .await?;
    let live = to_u64(live.try_get(0)?);
    let others = declared_by_others().await;
    if live.saturating_add(others).saturating_add(size) > quota {
        return Err(ApiError::Quota);
    }
    let opened = open().await?;
    tx.commit().await?;
    Ok(opened)
}

/// Marks a blob of the account deleted (the delayed deletion starts here). True if a live blob was marked.
pub async fn mark_deleted(
    pool: &Pool,
    account: &str,
    blob: &str,
    now_ms: u64,
) -> Result<bool, ApiError> {
    let client = pool.get().await?;
    let marked = client
        .execute(
            "UPDATE blobs SET deleted_ms = $3 WHERE account_id = $1 AND blob_id = $2 AND deleted_ms IS NULL",
            &[&account, &blob, &to_i64(now_ms, "clock")?],
        )
        .await?;
    Ok(marked == 1)
}

/// Marks every live blob of an account deleted, in the transaction of the account's deletion or of a restore that re-applies it.
pub async fn mark_account_deleted(
    tx: &Transaction<'_>,
    account: &str,
    now_ms: u64,
) -> Result<u64, ApiError> {
    Ok(tx
        .execute(
            "UPDATE blobs SET deleted_ms = $2 WHERE account_id = $1 AND deleted_ms IS NULL",
            &[&account, &to_i64(now_ms, "clock")?],
        )
        .await?)
}

/// The rows marked deleted at or before `cutoff_ms`: the ones the sweep may remove.
pub async fn deleted_before(pool: &Pool, cutoff_ms: u64) -> Result<Vec<Row>, ApiError> {
    let client = pool.get().await?;
    let rows = client
        .query(
            "SELECT account_id, blob_id, size, deleted_ms FROM blobs WHERE deleted_ms IS NOT NULL AND deleted_ms <= $1",
            &[&to_i64(cutoff_ms, "clock")?],
        )
        .await?;
    rows.iter().map(row_of).collect()
}

pub async fn remove_row(pool: &Pool, account: &str, blob: &str) -> Result<(), ApiError> {
    let client = pool.get().await?;
    client
        .execute(
            "DELETE FROM blobs WHERE account_id = $1 AND blob_id = $2 AND deleted_ms IS NOT NULL",
            &[&account, &blob],
        )
        .await?;
    Ok(())
}

/// Every row, for the orphan scan.
pub async fn all(pool: &Pool) -> Result<Vec<Row>, ApiError> {
    let client = pool.get().await?;
    let rows = client
        .query(
            "SELECT account_id, blob_id, size, deleted_ms FROM blobs",
            &[],
        )
        .await?;
    rows.iter().map(row_of).collect()
}

/// A row for an object that no row refers to, already marked deleted: it goes through the same delayed removal (a restore, the sweep).
pub async fn adopt_orphan(
    pool: &Pool,
    account: &str,
    blob: &str,
    size: u64,
    now_ms: u64,
) -> Result<(), ApiError> {
    let client = pool.get().await?;
    client
        .execute(
            "INSERT INTO blobs (account_id, blob_id, size, published_ms, deleted_ms) VALUES ($1, $2, $3, 0, $4) ON CONFLICT DO NOTHING",
            &[
                &account,
                &blob,
                &to_i64(size, "size")?,
                &to_i64(now_ms, "clock")?,
            ],
        )
        .await?;
    Ok(())
}

fn row_of(row: &tokio_postgres::Row) -> Result<Row, ApiError> {
    let deleted: Option<i64> = row.try_get("deleted_ms")?;
    Ok(Row {
        account_id: row.try_get("account_id")?,
        blob_id: row.try_get("blob_id")?,
        size: to_u64(row.try_get("size")?),
        deleted_ms: deleted.map(to_u64),
    })
}
