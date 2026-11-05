//! The idempotency records of the account server (ADR-0001 §A.5): what registration, a recovery and a reissue sent, kept **as hashes**,
//! so that the same request again can be told from another one without the server ever holding an `auth_key` or an `rc_auth_key`
//! (§9). No field is a `kinige-crypto` secret type: their `Debug` is redacted, and `tests/logs.rs` relies on a dump of the server showing
//! plain bytes.

use crate::wire::{Package, Recovery};
use kinige_crypto::ids::{AUTH_HASH_LEN, normalize_login, verify_auth};
use kinige_crypto::keys::SALT_LEN;
use libsodium_rs::utils;

/// What registration sent, as hashes: enough to tell the same package again from another one (§A.5) and no key.
#[derive(Debug)]
pub(crate) struct PackageRecord {
    pub(crate) account_id: [u8; 16],
    pub(crate) login: String,
    pub(crate) salt: [u8; SALT_LEN],
    pub(crate) profile: u8,
    pub(crate) auth_hash: [u8; AUTH_HASH_LEN],
    pub(crate) rc_hash: [u8; AUTH_HASH_LEN],
    pub(crate) wrap_pw: Vec<u8>,
    pub(crate) wrap_rc: Vec<u8>,
    pub(crate) key_id: u32,
}

/// The last recovery applied, as hashes (§A.5).
#[derive(Debug)]
pub(crate) struct RecoveryRecord {
    pub(crate) salt: [u8; SALT_LEN],
    pub(crate) profile: u8,
    pub(crate) auth_hash: [u8; AUTH_HASH_LEN],
    pub(crate) wrap_pw: Vec<u8>,
    pub(crate) rc_hash: [u8; AUTH_HASH_LEN],
    pub(crate) wrap_rc: Vec<u8>,
}

/// The last reissue applied: the hash of the new `rc_auth_key` and its `wrap_rc` (§A.5).
#[derive(Debug)]
pub(crate) struct ReissueRecord {
    pub(crate) rc_hash: [u8; AUTH_HASH_LEN],
    pub(crate) wrap_rc: Vec<u8>,
}

/// Whether `package` is the one that was registered: the keys are compared through their hashes (`verify_auth`, `sodium_memcmp`).
pub(crate) fn package_matches(record: &PackageRecord, package: &Package) -> bool {
    record.account_id == *package.account_id.as_bytes()
        && Some(&record.login) == normalize_login(&package.login).as_ref()
        && record.salt == package.salt
        && record.profile == package.profile
        && verify_auth(&record.auth_hash, package.auth_key.as_bytes()).unwrap_or(false)
        && verify_auth(&record.rc_hash, package.rc_auth_key.as_bytes()).unwrap_or(false)
        && record.wrap_pw == package.wrap_pw
        && record.wrap_rc == package.wrap_rc
        && record.key_id == package.key_id
}

/// Whether `recovery` (whose hashes the caller made) is the one that was applied.
pub(crate) fn recovery_matches(
    record: &RecoveryRecord,
    recovery: &Recovery,
    auth: &[u8; AUTH_HASH_LEN],
    rc: &[u8; AUTH_HASH_LEN],
) -> bool {
    record.salt == recovery.password.salt
        && record.profile == recovery.password.profile
        && utils::memcmp(&record.auth_hash, auth)
        && utils::memcmp(&record.rc_hash, rc)
        && record.wrap_pw == recovery.password.wrap_pw
        && record.wrap_rc == recovery.wrap_rc
}
