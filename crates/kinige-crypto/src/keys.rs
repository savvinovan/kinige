//! The key hierarchy of ADR-0001 §8.1: password and 24 words to the roots, the roots to `auth_key` and the KEK, the DEK to `K_env`
//! and `K_rev`. Only `crypto_pwhash` (Argon2id13) and `crypto_kdf_derive_from_key`; the contexts and subkey ids are the table of §8.1.

use crate::error::CryptoError;
use crate::profile::Profile;
use crate::secret::{
    AuthKey, DekKey, EnvKey, KEY_LEN, KekPw, KekRc, Password, PwRoot, RcAuthKey, RcRoot, RevKey,
    take_key,
};
use libsodium_rs::crypto_kdf;
use libsodium_rs::crypto_pwhash::argon2id;

/// Length of the Argon2id salt (appendix §D).
pub const SALT_LEN: usize = argon2id::SALTBYTES;

const CONTEXT_PW: &[u8; 8] = b"kinigePW";
const CONTEXT_RC: &[u8; 8] = b"kinigeRC";
const CONTEXT_DEK: &[u8; 8] = b"kinigeDK";

/// `crypto_pwhash(ARGON2ID13, salt 16, profile, out 32)` (§8.1). Takes tens of milliseconds to seconds and `memlimit` bytes of memory:
/// the caller runs it off the UI thread. The `profile` comes from [`Profile::from_server`] or [`Profile::for_new_password`].
pub fn derive_pw_root(
    password: &Password,
    salt: &[u8; SALT_LEN],
    profile: Profile,
) -> Result<PwRoot, CryptoError> {
    let out = argon2id::pwhash(
        KEY_LEN,
        password.as_slice(),
        salt,
        profile.ops_limit(),
        profile.mem_limit(),
    )
    .map_err(|_| CryptoError::Sodium)?;
    Ok(PwRoot::from_bytes(take_key(out)?))
}

/// Subkey 1 of `pw_root`: the key sent to the server for login (§8.1).
pub fn pw_auth_key(root: &PwRoot) -> Result<AuthKey, CryptoError> {
    Ok(AuthKey::from_bytes(subkey(root.as_bytes(), 1, CONTEXT_PW)?))
}

/// Subkey 2 of `pw_root`: wraps the DEK for the password.
pub fn pw_kek(root: &PwRoot) -> Result<KekPw, CryptoError> {
    Ok(KekPw::from_bytes(subkey(root.as_bytes(), 2, CONTEXT_PW)?))
}

/// Subkey 1 of `rc_root`: the key sent to the server to authorize a recovery.
pub fn rc_auth_key(root: &RcRoot) -> Result<RcAuthKey, CryptoError> {
    Ok(RcAuthKey::from_bytes(subkey(
        root.as_bytes(),
        1,
        CONTEXT_RC,
    )?))
}

/// Subkey 2 of `rc_root`: wraps the DEK for the words.
pub fn rc_kek(root: &RcRoot) -> Result<KekRc, CryptoError> {
    Ok(KekRc::from_bytes(subkey(root.as_bytes(), 2, CONTEXT_RC)?))
}

/// `DEK[key_id]` and the two subkeys derived from it (§8.1): `K_env` (subkey 1) and `K_rev` (subkey 3; subkey 2 is reserved).
#[derive(Clone)]
pub struct Dek {
    key_id: u32,
    key: DekKey,
    env: EnvKey,
    rev: RevKey,
}

impl Dek {
    /// A new random DEK (`randombytes_buf`) for `key_id` (1 for the first one).
    pub fn generate(key_id: u32) -> Result<Dek, CryptoError> {
        Dek::new(key_id, DekKey::random())
    }

    pub fn new(key_id: u32, key: DekKey) -> Result<Dek, CryptoError> {
        let env = EnvKey::from_bytes(subkey(key.as_bytes(), 1, CONTEXT_DEK)?);
        let rev = RevKey::from_bytes(subkey(key.as_bytes(), 3, CONTEXT_DEK)?);
        Ok(Dek {
            key_id,
            key,
            env,
            rev,
        })
    }

    pub fn key_id(&self) -> u32 {
        self.key_id
    }

    /// The DEK itself, for the wraps.
    pub fn key(&self) -> &DekKey {
        &self.key
    }

    pub fn env_key(&self) -> &EnvKey {
        &self.env
    }

    pub fn rev_key(&self) -> &RevKey {
        &self.rev
    }
}

impl std::fmt::Debug for Dek {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Dek(key_id {}, <redacted>)", self.key_id)
    }
}

/// `crypto_kdf_derive_from_key(32, id, context, master)`.
pub(crate) fn subkey(
    master: &[u8; KEY_LEN],
    id: u64,
    context: &[u8; 8],
) -> Result<[u8; KEY_LEN], CryptoError> {
    let master = crypto_kdf::Key::from_slice(master).map_err(|_| CryptoError::Sodium)?;
    let bytes = crypto_kdf::derive_from_key(KEY_LEN, id, context, &master)
        .map_err(|_| CryptoError::Sodium)?;
    take_key(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subkeys_differ_by_id_and_by_context() {
        let master = [9u8; KEY_LEN];
        let a = subkey(&master, 1, CONTEXT_PW).expect("a");
        assert_ne!(a, subkey(&master, 2, CONTEXT_PW).expect("b"));
        assert_ne!(a, subkey(&master, 1, CONTEXT_RC).expect("c"));
        assert_eq!(a, subkey(&master, 1, CONTEXT_PW).expect("again"));
    }

    #[test]
    fn the_dek_debug_names_no_bytes() {
        let dek = Dek::new(1, DekKey::from_bytes([0xcd; KEY_LEN])).expect("dek");
        let shown = format!("{dek:?}");
        assert_eq!(shown, "Dek(key_id 1, <redacted>)");
        assert!(!shown.to_lowercase().contains("cd"));
    }
}
