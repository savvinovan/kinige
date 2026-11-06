//! Accounts and credentials (ADR-0001 §8.3-§8.6, appendix §A.5): registration, login, password change, recovery and new words, ported
//! from the account server of `spikes/S4b-keys`. The server holds `crypto_generichash` of the two auth keys, the salt, the profile and
//! the wraps, and compares keys through their hashes with `sodium_memcmp`. Idempotency compares the repeated request with the stored
//! columns, which are the hashes themselves: no record of a request ever holds a raw key.

use super::sessions::{self, Token};
use super::{fixed, to_i64, to_u64};
use crate::error::ApiError;
use deadpool_postgres::{Pool, Transaction};
use kinige_crypto::ids::{auth_hash, unknown_login_salt, verify_auth};
use kinige_crypto::keys::SALT_LEN;
use kinige_crypto::secret::{AuthKey, RcAuthKey, ServerSecret};
use libsodium_rs::utils;
use tokio_postgres::Row;
use tokio_postgres::error::SqlState;

/// The profile a login without an account is answered with (§8.4).
const DEFAULT_PROFILE: u8 = 1;
/// A few tries when two registrations of one account race (the loser re-reads the winner).
const REGISTER_TRIES: usize = 3;

/// Everything a registration sends (§8.3), the login already normalized.
pub struct Registration {
    pub account_id: String,
    pub login: String,
    pub salt: [u8; SALT_LEN],
    pub profile: u8,
    pub auth_key: AuthKey,
    pub rc_auth_key: RcAuthKey,
    pub wrap_pw: Vec<u8>,
    pub wrap_rc: Vec<u8>,
    pub key_id: u32,
}

/// A new password (§8.5).
pub struct NewPassword {
    pub salt: [u8; SALT_LEN],
    pub profile: u8,
    pub auth_key: AuthKey,
    pub wrap_pw: Vec<u8>,
}

pub struct LoginOk {
    pub token: Token,
    pub account_id: String,
    pub key_id: u32,
    pub wrap_pw: Vec<u8>,
    pub cred_version: u64,
}

pub struct Params {
    pub salt: [u8; SALT_LEN],
    pub profile: u8,
    pub cred_version: u64,
}

pub struct Fetched {
    pub account_id: String,
    pub key_id: u32,
    pub wrap_rc: Vec<u8>,
    pub profile: u8,
}

/// Registration (§8.3), idempotent by `account_id` (§A.5): the same package again is a success, `account_id` taken by another package
/// is its own error, and only then is the login looked at, so a repeat after a lost answer never meets "login taken".
pub async fn register(pool: &Pool, now_ms: u64, r: &Registration) -> Result<Token, ApiError> {
    let auth = hash_of(r.auth_key.as_bytes())?;
    let rc = hash_of(r.rc_auth_key.as_bytes())?;
    let now = to_i64(now_ms, "clock")?;
    let mut client = pool.get().await?;
    for _ in 0..REGISTER_TRIES {
        let tx = client.transaction().await?;
        let existing = tx
            .query_opt(
                "SELECT * FROM accounts WHERE id = $1 FOR UPDATE",
                &[&r.account_id],
            )
            .await?;
        if let Some(row) = existing {
            if !same_package(&row, r, &auth, &rc)? {
                log::info!("register refused: account id taken");
                return Err(ApiError::Conflict("account_id_taken"));
            }
            log::info!("register repeated: same package");
            let token = sessions::open(&tx, &r.account_id, now_ms).await?;
            tx.commit().await?;
            return Ok(token);
        }
        let login_taken = tx
            .query_opt("SELECT 1 FROM accounts WHERE login = $1", &[&r.login])
            .await?;
        if login_taken.is_some() {
            log::info!("register refused: login taken");
            return Err(ApiError::Conflict("login_taken"));
        }
        let inserted = tx
            .execute(
                "INSERT INTO accounts (id, login, salt, profile, auth_hash, rc_hash, key_id, wrap_pw, wrap_rc, created_ms) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
                &[
                    &r.account_id,
                    &r.login,
                    &r.salt.as_slice(),
                    &i16::from(r.profile),
                    &auth.as_slice(),
                    &rc.as_slice(),
                    &i32::try_from(r.key_id).map_err(|_| ApiError::BadRequest("key_id"))?,
                    &r.wrap_pw,
                    &r.wrap_rc,
                    &now,
                ],
            )
            .await;
        match inserted {
            Ok(_) => {
                let token = sessions::open(&tx, &r.account_id, now_ms).await?;
                tx.commit().await?;
                log::info!("register ok");
                return Ok(token);
            }
            // A concurrent registration won between the reads and the insert: look again.
            Err(error) if error.code() == Some(&SqlState::UNIQUE_VIOLATION) => continue,
            Err(error) => return Err(error.into()),
        }
    }
    Err(ApiError::Conflict("login_taken"))
}

/// The salt and profile for a login (§8.4 step 1). For an unknown login: a 16-byte salt from `server_secret`, the default profile and
/// version 1, so the answer has the same shape and repeats. An account with profile 2 differs from that: accepted in §8.4.
pub async fn login_params(
    pool: &Pool,
    secret: &ServerSecret,
    login: &str,
) -> Result<Params, ApiError> {
    let client = pool.get().await?;
    let row = client
        .query_opt(
            "SELECT salt, profile, cred_version FROM accounts WHERE login = $1",
            &[&login],
        )
        .await?;
    if let Some(row) = row {
        return Ok(Params {
            salt: fixed(&row, "salt")?,
            profile: profile_of(&row)?,
            cred_version: to_u64(row.try_get("cred_version")?),
        });
    }
    Ok(Params {
        salt: unknown_login_salt(secret, login).map_err(|_| ApiError::Internal)?,
        profile: DEFAULT_PROFILE,
        cred_version: 1,
    })
}

/// Login by `auth_key` (§8.4 steps 2-3). An unknown login and a wrong key take the same path and give the same `Auth`. If `replaces`
/// is a session of the same account it is ended in the same transaction (ADR-0018, "S4c, sessions").
pub async fn login(
    pool: &Pool,
    now_ms: u64,
    login: &str,
    key: &AuthKey,
    replaces: Option<&Token>,
) -> Result<LoginOk, ApiError> {
    let mut client = pool.get().await?;
    let tx = client.transaction().await?;
    let row = tx
        .query_opt("SELECT * FROM accounts WHERE login = $1", &[&login])
        .await?;
    // An unknown login is compared against a hash of nothing, so it costs what a known one does.
    let stored: [u8; 32] = match &row {
        Some(row) => fixed(row, "auth_hash")?,
        None => [0; 32],
    };
    let matches = verify_auth(&stored, key.as_bytes()).map_err(|_| ApiError::Internal)?;
    let Some(row) = row.filter(|_| matches) else {
        log::info!("login failed");
        return Err(ApiError::Auth);
    };
    let account_id: String = row.try_get("id")?;
    if let Some(old) = replaces {
        tx.execute(
            "DELETE FROM sessions WHERE token_hash = $1 AND account_id = $2",
            &[&old.hash()?.as_slice(), &account_id],
        )
        .await?;
    }
    let token = sessions::open(&tx, &account_id, now_ms).await?;
    tx.commit().await?;
    log::info!("login ok");
    Ok(LoginOk {
        token,
        key_id: u32::try_from(row.try_get::<_, i32>("key_id")?).unwrap_or(0),
        wrap_pw: row.try_get("wrap_pw")?,
        cred_version: to_u64(row.try_get("cred_version")?),
        account_id,
    })
}

/// A password change (§8.5): with a session and the old `auth_key`. Salt, profile, `auth_key` and `wrap_pw` are replaced together, on
/// the credential version the old key was derived against; if another change landed first it is a `VersionConflict` and nothing is
/// written. A lower profile than the account's is refused. `revoke_others` ends every other session. Returns the new version.
pub async fn change_password(
    pool: &Pool,
    account: &str,
    session: &Token,
    old: &AuthKey,
    expected_version: u64,
    new: &NewPassword,
    revoke_others: bool,
) -> Result<u64, ApiError> {
    let mut client = pool.get().await?;
    let tx = client.transaction().await?;
    let row = locked(&tx, account).await?;
    let current = to_u64(row.try_get("cred_version")?);
    if current != expected_version {
        log::info!("password change refused: version conflict");
        return Err(ApiError::VersionConflict { current });
    }
    if !verify_auth(&fixed(&row, "auth_hash")?, old.as_bytes()).map_err(|_| ApiError::Internal)? {
        log::info!("password change refused: wrong key");
        return Err(ApiError::Auth);
    }
    refuse_downgrade(&row, new.profile)?;
    let new_hash = hash_of(new.auth_key.as_bytes())?;
    tx.execute(
        "UPDATE accounts SET salt = $2, profile = $3, auth_hash = $4, wrap_pw = $5, cred_version = cred_version + 1 WHERE id = $1",
        &[
            &account,
            &new.salt.as_slice(),
            &i16::from(new.profile),
            &new_hash.as_slice(),
            &new.wrap_pw,
        ],
    )
    .await?;
    if revoke_others {
        tx.execute(
            "DELETE FROM sessions WHERE account_id = $1 AND token_hash <> $2",
            &[&account, &session.hash()?.as_slice()],
        )
        .await?;
    }
    tx.commit().await?;
    log::info!("password changed");
    Ok(current.saturating_add(1))
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

/// What a recovery replaces in one transaction (§8.6 item 4).
pub struct NewRecovery {
    pub password: NewPassword,
    pub rc_auth_key: RcAuthKey,
    pub wrap_rc: Vec<u8>,
}

/// Step 4 of a recovery (§8.6): one atomic replacement authorized by the old `rc_auth_key`, which ends every session. Idempotent by the
/// new `rc_auth_key` **and** equality of the rest of the package (§A.5): the same package again is a success, another package behind
/// an `rc_auth_key` already applied is a separate error. Returns a new session and the new credential version.
pub async fn recover(
    pool: &Pool,
    now_ms: u64,
    login: &str,
    old: &RcAuthKey,
    new: &NewRecovery,
) -> Result<(Token, u64), ApiError> {
    let new_rc = hash_of(new.rc_auth_key.as_bytes())?;
    let new_pw = hash_of(new.password.auth_key.as_bytes())?;
    let mut client = pool.get().await?;
    let tx = client.transaction().await?;
    let row = tx
        .query_opt(
            "SELECT * FROM accounts WHERE login = $1 FOR UPDATE",
            &[&login],
        )
        .await?;
    let Some(row) = row else {
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
    account: &str,
    auth: &AuthKey,
    new_rc: &RcAuthKey,
    wrap_rc: &[u8],
) -> Result<(), ApiError> {
    let mut client = pool.get().await?;
    let tx = client.transaction().await?;
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
    tx.commit().await?;
    log::info!("words reissued");
    Ok(())
}

/// The account row, locked for the rest of the transaction. `Auth` if it is gone (a session outlived its account).
async fn locked(tx: &Transaction<'_>, account: &str) -> Result<Row, ApiError> {
    tx.query_opt(
        "SELECT * FROM accounts WHERE id = $1 FOR UPDATE",
        &[&account],
    )
    .await?
    .ok_or(ApiError::Auth)
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

/// "An account's profile only grows" (§8.2): the server refuses a lower one itself, whatever the client does.
fn refuse_downgrade(row: &Row, new_profile: u8) -> Result<(), ApiError> {
    if new_profile < profile_of(row)? {
        log::info!("refused: profile downgrade");
        return Err(ApiError::Conflict("profile_downgrade"));
    }
    Ok(())
}

fn profile_of(row: &Row) -> Result<u8, ApiError> {
    Ok(u8::try_from(row.try_get::<_, i16>("profile")?).unwrap_or(0))
}

/// Whether the stored account is exactly this package (§A.5). Keys are compared through their hashes.
fn same_package(
    row: &Row,
    r: &Registration,
    auth: &[u8; 32],
    rc: &[u8; 32],
) -> Result<bool, ApiError> {
    Ok(row.try_get::<_, String>("login")? == r.login
        && fixed::<SALT_LEN>(row, "salt")? == r.salt
        && profile_of(row)? == r.profile
        && utils::memcmp(&fixed::<32>(row, "auth_hash")?, auth)
        && utils::memcmp(&fixed::<32>(row, "rc_hash")?, rc)
        && row.try_get::<_, Vec<u8>>("wrap_pw")? == r.wrap_pw
        && row.try_get::<_, Vec<u8>>("wrap_rc")? == r.wrap_rc
        && row.try_get::<_, i32>("key_id")? == i32::try_from(r.key_id).unwrap_or(-1))
}

fn hash_of(key: &[u8; 32]) -> Result<[u8; 32], ApiError> {
    auth_hash(key).map_err(|_| ApiError::Internal)
}
