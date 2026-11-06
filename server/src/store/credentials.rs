//! The credential journal (ADR-0001 §8.9, round 1 of the review of S4c-2). A restore replaces the `accounts` rows with the backup's, which
//! would put back the password, the words and the wraps an account had before a change made after the backup: a password changed because
//! it leaked would work again. So every change of credentials (a password change, a recovery, new words) writes, in the same
//! transaction and under the shared backup lock, a row with **the whole credential state after the change** and the time: nothing that
//! `accounts` does not hold already. The journal is dumped with a backup, and a restore re-applies the newest entry at or after the
//! backup's cut time to each restored account, from the same three sources as the deletion journal.
//!
//! The account is named by `crypto_generichash(account_id)`, like the deletion journal; the rows of an account that is deleted are
//! deleted with it.
//!
//! **The stamp of an entry** (round 2): read after the account's row is locked, and raised above the account's newest entry. Two
//! changes of one account then never share a key, whatever the clock does (a clock that does not move, a floor that holds it up), and
//! the later change is the later entry. Should two ever meet, the insert replaces: the newer state wins, never the first.
//!
//! **Retired credentials do not live forever** (round 2): an entry older than the backup retention plus one backup interval can never
//! be applied by a restore that is not the newest backup (every other restorable backup was cut after it), and it holds a `wrap_pw` that a password from that time unwraps, so the sweep
//! removes it ([`prune`]), except the newest entry of an account, which holds only what the account's row holds now.

use super::{lock, to_i64, to_u64};
use crate::clock::Clock;
use crate::error::ApiError;
use crate::hashing::account_hash;
use deadpool_postgres::{Pool, Transaction};

/// Records the credentials `account` has now. Call it after the `UPDATE` of `accounts`, in the same transaction, which holds the
/// account's row lock and the shared backup lock.
pub async fn record(
    tx: &Transaction<'_>,
    account: &str,
    clock: &dyn Clock,
) -> Result<(), ApiError> {
    let hash = account_hash(account).ok_or(ApiError::Internal)?;
    let newest = tx
        .query_one(
            "SELECT COALESCE(max(changed_ms), -1) FROM credential_changes WHERE account_hash = $1",
            &[&hash.as_slice()],
        )
        .await?
        .try_get::<_, i64>(0)?;
    let stamped = lock::stamp(&**tx, clock).await?;
    let changed_ms = stamped.max(to_u64(newest.saturating_add(1)));
    tx.execute(
        "INSERT INTO credential_changes \
             (account_hash, cred_version, salt, profile, auth_hash, rc_hash, key_id, wrap_pw, wrap_rc, changed_ms) \
         SELECT $2, cred_version, salt, profile, auth_hash, rc_hash, key_id, wrap_pw, wrap_rc, $3 FROM accounts WHERE id = $1 \
         ON CONFLICT (account_hash, cred_version, changed_ms) DO UPDATE SET \
             salt = EXCLUDED.salt, profile = EXCLUDED.profile, auth_hash = EXCLUDED.auth_hash, rc_hash = EXCLUDED.rc_hash, \
             key_id = EXCLUDED.key_id, wrap_pw = EXCLUDED.wrap_pw, wrap_rc = EXCLUDED.wrap_rc",
        &[
            &account,
            &hash.as_slice(),
            &to_i64(changed_ms, "clock")?,
        ],
    )
    .await?;
    Ok(())
}

/// Removes the entries stamped before `cutoff_ms` (the database's clock minus the backup retention and one backup interval, as for a
/// deleted blob), **except the newest entry of each account**. Returns how many went. The retired ones are the point: they hold the
/// wraps and hashes of passwords that were replaced. The newest entry holds what the account's row holds now, so keeping it exposes
/// nothing more, and it is what a restore of the newest backup needs when that backup is older than the window (rotation keeps the
/// newest backup whatever its age, ADR-0001 §8.9): the backup's own `accounts` row is older than the entry.
pub async fn prune(pool: &Pool, cutoff_ms: u64) -> Result<u64, ApiError> {
    let client = pool.get().await?;
    Ok(client
        .execute(
            "DELETE FROM credential_changes old WHERE old.changed_ms < $1 AND EXISTS ( \
                 SELECT 1 FROM credential_changes newer \
                 WHERE newer.account_hash = old.account_hash AND newer.changed_ms > old.changed_ms)",
            &[&to_i64(cutoff_ms, "clock")?],
        )
        .await?)
}
