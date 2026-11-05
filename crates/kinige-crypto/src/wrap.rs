//! The DEK wraps `wrap_pw` and `wrap_rc` (ADR-0001 §8.1, appendix §A.1): 73 bytes, `wrap_version || nonce(24) || AEAD(KEK, DEK)`,
//! with an `AD_wrap` that binds the account, the `key_id`, the kind and, for the password wrap, the salt and the Argon2id profile.
//! So a server that substitutes the salt or the profile without a password change makes the wrap fail to open.

use crate::aead::{self, NONCE_LEN, TAG_LEN};
use crate::error::CryptoError;
use crate::ids::AccountId;
use crate::keys::{Dek, SALT_LEN};
use crate::profile::Profile;
use crate::secret::{DekKey, KEY_LEN, KekPw, KekRc, random_array};

pub const WRAP_VERSION: u8 = 1;
/// `1 + 24 + 32 + 16`.
pub const WRAP_LEN: usize = 1 + NONCE_LEN + KEY_LEN + TAG_LEN;

const LABEL: &[u8] = b"kinige.wrap.v1";
const KIND_PASSWORD: u8 = 1;
const KIND_RECOVERY: u8 = 2;

/// `wrap_pw = AEAD(KEK_pw, DEK)` under the account, the salt and the profile the password was derived with (§8.3, §8.5).
pub fn wrap_pw(
    dek: &Dek,
    kek: &KekPw,
    account: &AccountId,
    salt: &[u8; SALT_LEN],
    profile: Profile,
) -> Result<Vec<u8>, CryptoError> {
    let ad = ad_wrap(account, dek.key_id(), Kind::Password { salt, profile });
    seal(dek.key(), kek.as_bytes(), &ad, random_array())
}

/// `wrap_rc = AEAD(KEK_rc, DEK)` (§8.3, §8.6).
pub fn wrap_rc(dek: &Dek, kek: &KekRc, account: &AccountId) -> Result<Vec<u8>, CryptoError> {
    let ad = ad_wrap(account, dek.key_id(), Kind::Recovery);
    seal(dek.key(), kek.as_bytes(), &ad, random_array())
}

/// Opens `wrap_pw`. `key_id`, `salt` and `profile` are what the server sent next to it; a wrong one, like a wrong password, is
/// `Authentication`.
pub fn unwrap_pw(
    wrap: &[u8],
    kek: &KekPw,
    account: &AccountId,
    key_id: u32,
    salt: &[u8; SALT_LEN],
    profile: Profile,
) -> Result<Dek, CryptoError> {
    let ad = ad_wrap(account, key_id, Kind::Password { salt, profile });
    open(wrap, kek.as_bytes(), &ad, key_id)
}

/// Opens `wrap_rc`.
pub fn unwrap_rc(
    wrap: &[u8],
    kek: &KekRc,
    account: &AccountId,
    key_id: u32,
) -> Result<Dek, CryptoError> {
    let ad = ad_wrap(account, key_id, Kind::Recovery);
    open(wrap, kek.as_bytes(), &ad, key_id)
}

/// The associated data of §A.1, exposed for the test vectors.
pub(crate) enum Kind<'a> {
    Password {
        salt: &'a [u8; SALT_LEN],
        profile: Profile,
    },
    Recovery,
}

/// `"kinige.wrap.v1" || wrap_version || account_id(16) || key_id(u32 BE) || kind(u8) || if password: salt(16) || profile(u8)`.
pub(crate) fn ad_wrap(account: &AccountId, key_id: u32, kind: Kind<'_>) -> Vec<u8> {
    let mut ad = Vec::with_capacity(LABEL.len() + 1 + 16 + 4 + 1 + SALT_LEN + 1);
    ad.extend_from_slice(LABEL);
    ad.push(WRAP_VERSION);
    ad.extend_from_slice(account.as_bytes());
    ad.extend_from_slice(&key_id.to_be_bytes());
    match kind {
        Kind::Password { salt, profile } => {
            ad.push(KIND_PASSWORD);
            ad.extend_from_slice(salt);
            ad.push(profile.number());
        }
        Kind::Recovery => ad.push(KIND_RECOVERY),
    }
    ad
}

pub(crate) fn seal(
    dek: &DekKey,
    kek: &[u8; KEY_LEN],
    ad: &[u8],
    nonce: [u8; NONCE_LEN],
) -> Result<Vec<u8>, CryptoError> {
    let sealed = aead::encrypt(kek, &nonce, ad, dek.as_bytes())?;
    let mut wrap = Vec::with_capacity(WRAP_LEN);
    wrap.push(WRAP_VERSION);
    wrap.extend_from_slice(&nonce);
    wrap.extend_from_slice(&sealed);
    Ok(wrap)
}

/// Lengths and the version byte first, then the AEAD, then the plaintext must be exactly 32 bytes.
fn open(wrap: &[u8], kek: &[u8; KEY_LEN], ad: &[u8], key_id: u32) -> Result<Dek, CryptoError> {
    if wrap.len() != WRAP_LEN {
        return Err(CryptoError::CiphertextLength);
    }
    let (version, rest) = wrap.split_first().ok_or(CryptoError::Truncated)?;
    if *version != WRAP_VERSION {
        return Err(CryptoError::UnknownWrapVersion(*version));
    }
    let (nonce, sealed) = rest
        .split_at_checked(NONCE_LEN)
        .ok_or(CryptoError::Truncated)?;
    let nonce = <[u8; NONCE_LEN]>::try_from(nonce).map_err(|_| CryptoError::Truncated)?;
    let plain = aead::decrypt(kek, &nonce, ad, sealed)?;
    let key =
        crate::secret::take_key(plain).map_err(|_| CryptoError::Malformed("wrapped key length"))?;
    Dek::new(key_id, DekKey::from_bytes(key))
}
