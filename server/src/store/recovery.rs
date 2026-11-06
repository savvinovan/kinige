//! Recovery and new words (ADR-0001 §8.6, appendix §A.5): the word check, the one atomic replacement authorized by the old `rc_auth_key`,
//! and reissuing the words. Ported from `spikes/S4b-keys/src/server.rs`; the shared helpers are in `accounts.rs`.

use super::accounts::{NewPassword, hash_of, locked, profile_of, refuse_downgrade};
use super::sessions::{self, Token};
use super::{credentials, fixed, lock, to_u64};
use crate::clock::Clock;
use crate::error::ApiError;
use deadpool_postgres::Pool;
use kinige_crypto::ids::verify_auth;
use kinige_crypto::keys::SALT_LEN;
use kinige_crypto::secret::{AuthKey, RcAuthKey};
use libsodium_rs::utils;
use tokio_postgres::Row;

pub struct Fetched {
    pub account_id: String,
    pub key_id: u32,
    pub wrap_rc: Vec<u8>,
    pub profile: u8,
}

/// What a recovery replaces in one transaction (§8.6 item 4).
pub struct NewRecovery {
    pub password: NewPassword,
    pub rc_auth_key: RcAuthKey,
    pub wrap_rc: Vec<u8>,
}

/// Step 1 of a recovery (§8.6): the account's `wrap_rc` for a valid `rc_auth_key`, without a session.
pub async fn recovery_fetch(pool: &Pool, login: &str, rc: &RcAuthKey) -> Result<Fetched, ApiError> {
    let client = pool.get().await?;
    let row = client
        .query_opt("SELECT * FROM accounts WHERE login = $1", &[&login])
        .await?;
    let row = rc_checked(row, rc)?.ok_or_else(|| {
        log::info!("recovery fetch failed");
        ApiError::Auth
    })?;
    log::info!("recovery fetch ok");
    Ok(Fetched {
        account_id: row.try_get("id")?,
        key_id: u32::try_from(row.try_get::<_, i32>("key_id")?).unwrap_or(0),
        wrap_rc: row.try_get("wrap_rc")?,
        profile: profile_of(&row)?,
    })
}

/// Step 4 of a recovery (§8.6): one atomic replacement authorized by the old `rc_auth_key`, which ends every session. Idempotent by the
/// new `rc_auth_key` **and** equality of the rest of the package (§A.5): the same package again is a success, another package behind
/// an `rc_auth_key` already applied is a separate error. Returns a new session and the new credential version.
pub async fn recover(
    pool: &Pool,
    clock: &dyn Clock,
    login: &str,
    old: &RcAuthKey,
    new: &NewRecovery,
) -> Result<(Token, u64), ApiError> {
    let new_rc = hash_of(new.rc_auth_key.as_bytes())?;
    let new_pw = hash_of(new.password.auth_key.as_bytes())?;
    let mut client = pool.get().await?;
    let tx = client.transaction().await?;
    lock::shared(&tx).await?;
    let now_ms = lock::stamp(&*tx, clock).await?;
    let row = tx
        .query_opt(
            "SELECT * FROM accounts WHERE login = $1 FOR UPDATE",
            &[&login],
        )
        .await?;
    let Some(row) = row else {
        // An unknown login costs what a known one does.
        let _ = verify_auth(&[0; 32], old.as_bytes());
        log::info!("recovery failed");
        return Err(ApiError::Auth);
    };
    let account: String = row.try_get("id")?;
    let version = to_u64(row.try_get("cred_version")?);
    if utils::memcmp(&fixed::<32>(&row, "rc_hash")?, &new_rc) {
        // The new words are already in force: the same package is a repeat, another one is a clash (§A.5).
        let same = fixed::<SALT_LEN>(&row, "salt")? == new.password.salt
            && profile_of(&row)? == new.password.profile
            && utils::memcmp(&fixed::<32>(&row, "auth_hash")?, &new_pw)
            && row.try_get::<_, Vec<u8>>("wrap_pw")? == new.password.wrap_pw
            && row.try_get::<_, Vec<u8>>("wrap_rc")? == new.wrap_rc;
        if !same {
            log::info!("recovery refused: key already applied by another package");
            return Err(ApiError::Conflict("package_mismatch"));
        }
        log::info!("recovery repeated: same package");
        let token = sessions::open(&tx, &account, now_ms).await?;
        tx.commit().await?;
        return Ok((token, version));
    }
    if !verify_auth(&fixed(&row, "rc_hash")?, old.as_bytes()).map_err(|_| ApiError::Internal)? {
        log::info!("recovery failed");
        return Err(ApiError::Auth);
    }
    refuse_downgrade(&row, new.password.profile)?;
    tx.execute(
        "UPDATE accounts SET salt = $2, profile = $3, auth_hash = $4, wrap_pw = $5, rc_hash = $6, wrap_rc = $7, \
         cred_version = cred_version + 1 WHERE id = $1",
        &[
            &account,
            &new.password.salt.as_slice(),
            &i16::from(new.password.profile),
            &new_pw.as_slice(),
            &new.password.wrap_pw,
            &new_rc.as_slice(),
            &new.wrap_rc,
        ],
    )
    .await?;
    credentials::record(&tx, &account, now_ms).await?;
    // Recovery ends every session, the new one is opened after (decision 7).
    tx.execute("DELETE FROM sessions WHERE account_id = $1", &[&account])
        .await?;
    let token = sessions::open(&tx, &account, now_ms).await?;
    tx.commit().await?;
    log::info!("recovery ok: sessions revoked");
    Ok((token, version.saturating_add(1)))
}

/// New words (§8.6, "Reissuing the words"): with a session and the password's `auth_key`; idempotent by the new `rc_auth_key`.
pub async fn reissue_words(
    pool: &Pool,
    clock: &dyn Clock,
    account: &str,
    auth: &AuthKey,
    new_rc: &RcAuthKey,
    wrap_rc: &[u8],
) -> Result<(), ApiError> {
    let mut client = pool.get().await?;
    let tx = client.transaction().await?;
    lock::shared(&tx).await?;
    let changed_ms = lock::stamp(&*tx, clock).await?;
    let row = locked(&tx, account).await?;
    if !verify_auth(&fixed(&row, "auth_hash")?, auth.as_bytes()).map_err(|_| ApiError::Internal)? {
        return Err(ApiError::Auth);
    }
    let new_hash = hash_of(new_rc.as_bytes())?;
    if utils::memcmp(&fixed::<32>(&row, "rc_hash")?, &new_hash) {
        let same = row.try_get::<_, Vec<u8>>("wrap_rc")? == wrap_rc;
        return if same {
            Ok(())
        } else {
            Err(ApiError::Conflict("package_mismatch"))
        };
    }
    tx.execute(
        "UPDATE accounts SET rc_hash = $2, wrap_rc = $3 WHERE id = $1",
        &[&account, &new_hash.as_slice(), &wrap_rc],
    )
    .await?;
    credentials::record(&tx, account, changed_ms).await?;
    tx.commit().await?;
    log::info!("words reissued");
    Ok(())
}

/// The row if it exists and `rc` is its words' key; compared against a hash of nothing when there is no row.
fn rc_checked(row: Option<Row>, rc: &RcAuthKey) -> Result<Option<Row>, ApiError> {
    let stored: [u8; 32] = match &row {
        Some(row) => fixed(row, "rc_hash")?,
        None => [0; 32],
    };
    let matches = verify_auth(&stored, rc.as_bytes()).map_err(|_| ApiError::Internal)?;
    Ok(row.filter(|_| matches))
}
