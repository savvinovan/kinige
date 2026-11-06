//! The credential journal (ADR-0001 §8.9, round 1 of the review of S4c-2). A restore replaces the `accounts` rows with the backup's, which
//! would put back the password, the words and the wraps an account had before a change made after the backup: a password changed because
//! it leaked would work again. So every change of credentials (a password change, a recovery, new words) writes, in the same
//! transaction and under the shared backup lock, a row with **the whole credential state after the change** and the time: nothing that
//! `accounts` does not hold already. The journal is dumped with a backup, and a restore re-applies the newest entry at or after the
//! backup's cut time to each restored account, from the same three sources as the deletion journal.
//!
//! The account is named by `crypto_generichash(account_id)`, like the deletion journal; the rows of an account that is deleted are
//! deleted with it.

use crate::error::ApiError;
use crate::hashing::account_hash;
use deadpool_postgres::Transaction;

/// Records the credentials `account` has now, stamped `changed_ms`. Call it after the `UPDATE` of `accounts`, in the same transaction.
pub async fn record(tx: &Transaction<'_>, account: &str, changed_ms: u64) -> Result<(), ApiError> {
    let hash = account_hash(account).ok_or(ApiError::Internal)?;
    tx.execute(
        "INSERT INTO credential_changes \
             (account_hash, cred_version, salt, profile, auth_hash, rc_hash, key_id, wrap_pw, wrap_rc, changed_ms) \
         SELECT $2, cred_version, salt, profile, auth_hash, rc_hash, key_id, wrap_pw, wrap_rc, $3 FROM accounts WHERE id = $1 \
         ON CONFLICT DO NOTHING",
        &[
            &account,
            &hash.as_slice(),
            &super::to_i64(changed_ms, "clock")?,
        ],
    )
    .await?;
    Ok(())
}
