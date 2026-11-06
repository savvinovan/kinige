//! `crypto_aead_xchacha20poly1305_ietf` (ADR-0001 §5.1, §11) with the binding's key and nonce types built for one call only: the
//! binding's `Key` derives `Debug` and would print its bytes, so it never leaves this file.

use crate::error::CryptoError;
use crate::secret::KEY_LEN;
use libsodium_rs::crypto_aead::xchacha20poly1305 as xchacha;

/// Length of the nonce: 24 random bytes per message (§5.1).
pub const NONCE_LEN: usize = xchacha::NPUBBYTES;
/// Length of the Poly1305 tag.
pub const TAG_LEN: usize = xchacha::ABYTES;

/// `ciphertext || tag`.
pub(crate) fn encrypt(
    key: &[u8; KEY_LEN],
    nonce: &[u8; NONCE_LEN],
    ad: &[u8],
    plaintext: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    let key = xchacha::Key::from_bytes(key).map_err(|_| CryptoError::Sodium)?;
    xchacha::encrypt(
        plaintext,
        Some(ad),
        &xchacha::Nonce::from_bytes(*nonce),
        &key,
    )
    .map_err(|_| CryptoError::Sodium)
}

/// Any failure of the AEAD (a changed byte of the ciphertext, the associated data, the nonce, a wrong key) is `Authentication`:
/// the cause is not told apart.
pub(crate) fn decrypt(
    key: &[u8; KEY_LEN],
    nonce: &[u8; NONCE_LEN],
    ad: &[u8],
    ciphertext: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    let key = xchacha::Key::from_bytes(key).map_err(|_| CryptoError::Sodium)?;
    xchacha::decrypt(
        ciphertext,
        Some(ad),
        &xchacha::Nonce::from_bytes(*nonce),
        &key,
    )
    .map_err(|_| CryptoError::Authentication)
}
