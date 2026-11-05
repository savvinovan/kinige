//! Secret values (§8.8): zeroed when dropped, never printed. Every key has its own type so that a `KEK_pw` cannot be passed where
//! an `auth_key` is expected. The binding's own key types derive `Debug` and print their bytes (libsodium-rs 0.3.0), so they are
//! built for one call from these and dropped at once; they are never stored or returned.

use crate::error::CryptoError;
use libsodium_rs::{random, utils};
use std::fmt;

pub const KEY_LEN: usize = 32;

/// A byte string that is zeroed on drop and prints as `<redacted>`: a password, the 24 words as text, a decrypted payload.
pub struct SecretBytes(Vec<u8>);

impl SecretBytes {
    pub fn new(bytes: Vec<u8>) -> SecretBytes {
        SecretBytes(bytes)
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(crate) fn push_byte(&mut self, byte: u8) {
        self.0.push(byte);
    }

    /// Appends `text` with ASCII letters lowered, byte by byte: no temporary copy of the text is made.
    pub(crate) fn extend_lowercase(&mut self, text: &str) {
        self.0
            .extend(text.bytes().map(|byte| byte.to_ascii_lowercase()));
    }
}

impl Drop for SecretBytes {
    fn drop(&mut self) {
        utils::memzero(&mut self.0);
    }
}

impl fmt::Debug for SecretBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretBytes(<redacted>)")
    }
}

pub type Password = SecretBytes;

/// Defines a 32-byte secret key type: zeroed on drop, redacted `Debug`, no `Display`, no `PartialEq` (compare with `ct_eq`).
macro_rules! secret_key {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Clone)]
        pub struct $name([u8; $crate::secret::KEY_LEN]);

        impl $name {
            pub fn from_bytes(bytes: [u8; $crate::secret::KEY_LEN]) -> $name {
                $name(bytes)
            }

            /// `None` unless the slice is exactly 32 bytes.
            pub fn from_slice(bytes: &[u8]) -> Option<$name> {
                <[u8; $crate::secret::KEY_LEN]>::try_from(bytes).ok().map($name)
            }

            /// 32 bytes from `randombytes_buf` (§11).
            pub fn random() -> $name {
                let mut bytes = [0u8; $crate::secret::KEY_LEN];
                libsodium_rs::random::fill_bytes(&mut bytes);
                $name(bytes)
            }

            pub fn as_bytes(&self) -> &[u8; $crate::secret::KEY_LEN] {
                &self.0
            }

            /// Constant-time comparison (`sodium_memcmp`).
            pub fn ct_eq(&self, other: &$name) -> bool {
                libsodium_rs::utils::memcmp(&self.0, &other.0)
            }
        }

        impl Drop for $name {
            fn drop(&mut self) {
                libsodium_rs::utils::memzero(&mut self.0);
            }
        }

        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(concat!(stringify!($name), "(<redacted>)"))
            }
        }
    };
}

secret_key!(
    /// `crypto_pwhash(password, salt, profile)`: the root of the password branch (§8.1).
    PwRoot
);
secret_key!(
    /// The 32 bytes of entropy behind the 24 words: the root of the recovery branch (§8.1). Not a BIP-39 seed.
    RcRoot
);
secret_key!(
    /// Subkey 1 of `pw_root`: goes to the server over TLS, which stores its hash (§9).
    AuthKey
);
secret_key!(
    /// Subkey 1 of `rc_root`: goes to the server over TLS, which stores its hash (§9).
    RcAuthKey
);
secret_key!(
    /// Subkey 2 of `pw_root`: wraps the DEK for the password (§8.1).
    KekPw
);
secret_key!(
    /// Subkey 2 of `rc_root`: wraps the DEK for the words (§8.1).
    KekRc
);
secret_key!(
    /// The data key `DEK[key_id]` itself, as it is wrapped (§8.1).
    DekKey
);
secret_key!(
    /// Subkey 1 of a DEK: the AEAD key of the envelopes (§5.1).
    EnvKey
);
secret_key!(
    /// Subkey 3 of a DEK: the key of the `conflict` revision id (§1).
    RevKey
);
secret_key!(
    /// The server's own secret for the salt of an unknown login (§8.4). Never leaves the server.
    ServerSecret
);
secret_key!(
    /// The per-attachment key of the blob stream (§7).
    BlobKey
);

/// Moves a key out of a `Vec` the binding returned and zeroes the `Vec`.
pub(crate) fn take_key(mut bytes: Vec<u8>) -> Result<[u8; KEY_LEN], CryptoError> {
    let key = <[u8; KEY_LEN]>::try_from(bytes.as_slice()).map_err(|_| CryptoError::Sodium);
    utils::memzero(&mut bytes);
    key
}

/// Fills a new array with random bytes (`randombytes_buf`), for nonces and ids.
pub(crate) fn random_array<const N: usize>() -> [u8; N] {
    let mut bytes = [0u8; N];
    random::fill_bytes(&mut bytes);
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_print_as_redacted() {
        let key = AuthKey::from_bytes([0xab; KEY_LEN]);
        assert_eq!(format!("{key:?}"), "AuthKey(<redacted>)");
        assert_eq!(format!("{key:#?}"), "AuthKey(<redacted>)");
        let password: Password = SecretBytes::new(b"fake-password".to_vec());
        assert_eq!(format!("{password:?}"), "SecretBytes(<redacted>)");
        assert!(!format!("{password:?}").contains("fake"));
    }

    #[test]
    fn from_slice_wants_exactly_32_bytes() {
        assert!(PwRoot::from_slice(&[0; 31]).is_none());
        assert!(PwRoot::from_slice(&[0; 33]).is_none());
        assert!(PwRoot::from_slice(&[0; 32]).is_some());
    }

    #[test]
    fn ct_eq_compares_bytes() {
        let a = EnvKey::from_bytes([1; KEY_LEN]);
        assert!(a.ct_eq(&a.clone()));
        assert!(!a.ct_eq(&EnvKey::from_bytes([2; KEY_LEN])));
    }
}
