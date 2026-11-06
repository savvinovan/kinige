//! Account deletion (ADR-0001 §8.9, PRD §10.3). One transaction: the account row goes, and with it, by `ON DELETE CASCADE`, its wraps and
//! hashes, its sessions, its envelopes, its `op_id`s, its indexes and its drops. Its blobs are marked deleted and leave the store after the
//! backup retention, like any deleted blob. The journal gets `crypto_generichash(account_id)` and the time, and nothing else: a restore of
//! an older backup applies the entries newer than its cut time, so the account does not come back.
//!
//! The deletion takes the backup lock first and reads the clock after (`store::lock`), so that its stamp is at or above the cut time of
//! every backup that does not contain it.

use super::accounts::locked;
use super::{blobs, fixed, lock, to_i64};
use crate::clock::Clock;
use crate::error::ApiError;
use crate::hashing::account_hash;
use deadpool_postgres::Pool;
use kinige_crypto::ids::verify_auth;
use kinige_crypto::secret::AuthKey;

/// Deletes `account` after the password's `auth_key` is checked again (a stolen session alone cannot do this).
pub async fn delete_account(
    pool: &Pool,
    clock: &dyn Clock,
    account: &str,
    auth: &AuthKey,
) -> Result<(), ApiError> {
    let mut client = pool.get().await?;
    let tx = client.transaction().await?;
    lock::shared(&tx).await?;
    let now_ms = clock.now_ms();
    let row = locked(&tx, account).await?;
    if !verify_auth(&fixed(&row, "auth_hash")?, auth.as_bytes()).map_err(|_| ApiError::Internal)? {
        log::info!("account deletion refused: wrong key");
        return Err(ApiError::Auth);
    }
    let hash = account_hash(account).ok_or(ApiError::Internal)?;
    tx.execute(
        "INSERT INTO deletions (account_hash, deleted_ms) VALUES ($1, $2) ON CONFLICT (account_hash) DO NOTHING",
        &[&hash.as_slice(), &to_i64(now_ms, "clock")?],
    )
    .await?;
    blobs::mark_account_deleted(&tx, account, now_ms).await?;
    tx.execute("DELETE FROM accounts WHERE id = $1", &[&account])
        .await?;
    tx.commit().await?;
    log::info!("account deleted");
    Ok(())
}
