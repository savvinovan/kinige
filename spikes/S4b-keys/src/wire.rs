//! What the client and the server hand each other: the requests and answers of the account flows, as plain data (ADR-0001 §8.3-§8.6,
//! appendix §A.5). The HTTP shapes are S4c's; here a call is a function and these are its arguments and results.

use kinige_crypto::AccountId;
use kinige_crypto::ids::AUTH_HASH_LEN;
use kinige_crypto::keys::SALT_LEN;
use kinige_crypto::secret::{AuthKey, RcAuthKey};

/// A session token: 32 random bytes the server hands out and keeps only as a hash (§8.4). Never printed.
#[derive(Clone)]
pub struct Token([u8; 32]);

impl Token {
    pub(crate) fn random() -> Token {
        Token(
            libsodium_rs::random::bytes(32)
                .try_into()
                .unwrap_or([0; 32]),
        )
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl Drop for Token {
    fn drop(&mut self) {
        libsodium_rs::utils::memzero(&mut self.0);
    }
}

impl std::fmt::Debug for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Token(<redacted>)")
    }
}

/// Everything a registration sends (§8.3), also what the client keeps as its prepared package (§A.5). `Debug` is written by hand: it
/// names no login and no key.
#[derive(Clone)]
pub struct Package {
    pub account_id: AccountId,
    pub login: String,
    pub salt: [u8; SALT_LEN],
    pub profile: u8,
    pub auth_key: AuthKey,
    pub rc_auth_key: RcAuthKey,
    pub wrap_pw: Vec<u8>,
    pub wrap_rc: Vec<u8>,
    pub key_id: u32,
}

impl std::fmt::Debug for Package {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Package")
            .field("account_id", &self.account_id)
            .field("profile", &self.profile)
            .field("key_id", &self.key_id)
            .finish_non_exhaustive()
    }
}

/// A new password: salt, profile, `auth_key` and `wrap_pw` (§8.5).
#[derive(Clone)]
pub struct PasswordPart {
    pub salt: [u8; SALT_LEN],
    pub profile: u8,
    pub auth_key: AuthKey,
    pub wrap_pw: Vec<u8>,
}

impl std::fmt::Debug for PasswordPart {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PasswordPart")
            .field("profile", &self.profile)
            .finish_non_exhaustive()
    }
}

/// What a recovery replaces in one transaction (§8.6 item 4): the password part and the words part.
#[derive(Clone)]
pub struct Recovery {
    pub password: PasswordPart,
    pub rc_auth_key: RcAuthKey,
    pub wrap_rc: Vec<u8>,
}

impl std::fmt::Debug for Recovery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Recovery")
            .field("password", &self.password)
            .finish_non_exhaustive()
    }
}

pub struct Session {
    pub token: Token,
}

pub struct Params {
    pub salt: [u8; SALT_LEN],
    pub profile: u8,
}

pub struct LoginOk {
    pub token: Token,
    pub account_id: AccountId,
    pub key_id: u32,
    pub wrap_pw: Vec<u8>,
}

pub struct RecoveryFetch {
    pub account_id: AccountId,
    pub key_id: u32,
    pub wrap_rc: Vec<u8>,
    pub profile: u8,
}

/// What the server holds of an account's keys: the salt and profile, which it must send, and the hashes of the two auth keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredView {
    pub salt: [u8; SALT_LEN],
    pub profile: u8,
    pub auth_hash: [u8; AUTH_HASH_LEN],
    pub rc_hash: [u8; AUTH_HASH_LEN],
}

/// "No such login" and "wrong key" are the same value (§8.6): nothing in it says which.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthFailed;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterError {
    /// `account_id` is taken by another package: the client makes a new id and rebuilds the wraps (§A.5).
    AccountIdTaken,
    LoginTaken,
    /// The login, a wrap or the profile is not what §8.3 and §A.1 allow.
    Invalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoverError {
    AuthFailed,
    /// The new `rc_auth_key` was already applied by a different package (§A.5).
    PackageMismatch,
    Invalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionError {
    /// The token or the key is not valid.
    Unauthorized,
    Invalid,
}
