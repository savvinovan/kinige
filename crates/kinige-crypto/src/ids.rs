//! Identifiers and hashes that sit around the keys: the account id as 16 raw bytes (§A.1), the id of a `conflict` revision (§1),
//! and the server's side of authentication (§8.3, §8.4, §9).

use crate::error::CryptoError;
use crate::secret::{KEY_LEN, RevKey, ServerSecret};
use kinige_domain::sync::conflict::{ConflictRevision, uuid_bytes, uuid_from_bytes};
use libsodium_rs::crypto_generichash::generichash;
use libsodium_rs::utils;

/// Output of the salt for an unknown login, equal to the salt of an Argon2id password hash (§8.4).
pub const UNKNOWN_SALT_LEN: usize = 16;
/// `crypto_generichash` default output: the length of a stored `auth_key` hash (§8.3).
pub const AUTH_HASH_LEN: usize = 32;

/// The account's UUID as the 16 raw bytes that go into `AD_env` and `AD_wrap` (§A.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccountId([u8; 16]);

impl AccountId {
    pub fn from_bytes(bytes: [u8; 16]) -> AccountId {
        AccountId(bytes)
    }

    /// A lower-case UUID of 36 characters.
    pub fn parse(text: &str) -> Option<AccountId> {
        uuid_bytes(text).map(AccountId)
    }

    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

/// The id of the `conflict` revision of the losing version P (§1): the first 16 bytes of `crypto_generichash(K_rev, input)`
/// (32-byte output, the default of the function), with the version and variant bits of a v4 UUID. The input is
/// `message_id[16] || group[1] || hlc(P)[56]`, made by the domain (`ConflictRevision::id_input`); the domain has no keys.
pub fn conflict_revision_id(
    k_rev: &RevKey,
    revision: &ConflictRevision,
) -> Result<String, CryptoError> {
    let mut hash = generichash(&revision.id_input, Some(k_rev.as_bytes()), KEY_LEN)
        .map_err(|_| CryptoError::Sodium)?;
    let mut first = [0u8; 16];
    for (target, source) in first.iter_mut().zip(hash.iter()) {
        *target = *source;
    }
    utils::memzero(&mut hash);
    Ok(uuid_from_bytes(first))
}

/// What the server stores for an `auth_key` or an `rc_auth_key`: `crypto_generichash(key)`, 32 bytes. A fast hash is enough, the
/// input is 256 uniform bits and not a password (§8.3).
pub fn auth_hash(key: &[u8; KEY_LEN]) -> Result<[u8; AUTH_HASH_LEN], CryptoError> {
    let hash = generichash(key, None, AUTH_HASH_LEN).map_err(|_| CryptoError::Sodium)?;
    <[u8; AUTH_HASH_LEN]>::try_from(hash.as_slice()).map_err(|_| CryptoError::Sodium)
}

/// Whether a presented key matches the stored hash, compared with `sodium_memcmp` (§8.3).
pub fn verify_auth(
    stored: &[u8; AUTH_HASH_LEN],
    presented: &[u8; KEY_LEN],
) -> Result<bool, CryptoError> {
    Ok(utils::memcmp(stored, &auth_hash(presented)?))
}

/// The salt the server returns for a login it does not know: `crypto_generichash(outlen 16, key = server_secret, login)` (§8.4), so
/// the same unknown login always gets the same salt and cannot be told from a known one by repeating the request.
pub fn unknown_login_salt(
    secret: &ServerSecret,
    login: &str,
) -> Result<[u8; UNKNOWN_SALT_LEN], CryptoError> {
    let salt = generichash(login.as_bytes(), Some(secret.as_bytes()), UNKNOWN_SALT_LEN)
        .map_err(|_| CryptoError::Sodium)?;
    <[u8; UNKNOWN_SALT_LEN]>::try_from(salt.as_slice()).map_err(|_| CryptoError::Sodium)
}

/// The login of §8.3: ASCII only, `[a-z0-9._@+-]`, lower-cased and trimmed. `None` for an empty login or any other character.
pub fn normalize_login(input: &str) -> Option<String> {
    let login = input.trim().to_ascii_lowercase();
    let allowed = |c: char| {
        c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '@' | '+' | '-')
    };
    (!login.is_empty() && login.chars().all(allowed)).then_some(login)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logins_are_ascii_lowercase_and_trimmed() {
        assert_eq!(
            normalize_login("  Fake.User+x@Example.test "),
            Some("fake.user+x@example.test".to_string())
        );
        for bad in [
            "",
            "   ",
            "ivan petrov",
            "ivan\u{44f}",
            "a/b",
            "a;b",
            "a\nb",
            "a\u{0}b",
            "caf\u{e9}",
        ] {
            assert_eq!(normalize_login(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn account_ids_parse_only_lower_case_uuids() {
        assert!(AccountId::parse("5f0c3e2a-9b1d-4c7e-8a64-0d2f1b3c4e5f").is_some());
        for bad in [
            "",
            "5F0C3E2A-9B1D-4C7E-8A64-0D2F1B3C4E5F",
            "5f0c3e2a9b1d4c7e8a640d2f1b3c4e5f",
            "not a uuid",
        ] {
            assert_eq!(AccountId::parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn the_unknown_login_salt_depends_on_the_secret_and_the_login() {
        let one = ServerSecret::from_bytes([1; KEY_LEN]);
        let two = ServerSecret::from_bytes([2; KEY_LEN]);
        let a = unknown_login_salt(&one, "fake@example.test").expect("a");
        assert_eq!(
            a,
            unknown_login_salt(&one, "fake@example.test").expect("again")
        );
        assert_ne!(
            a,
            unknown_login_salt(&one, "other@example.test").expect("login")
        );
        assert_ne!(
            a,
            unknown_login_salt(&two, "fake@example.test").expect("secret")
        );
    }

    #[test]
    fn auth_hash_verifies_the_key_it_was_made_from_only() {
        let key = [7u8; KEY_LEN];
        let stored = auth_hash(&key).expect("hash");
        assert_eq!(verify_auth(&stored, &key), Ok(true));
        assert_eq!(verify_auth(&stored, &[8u8; KEY_LEN]), Ok(false));
    }
}
