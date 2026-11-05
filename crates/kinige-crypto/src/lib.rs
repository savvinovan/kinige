//! Kinige's envelope and key scheme (ADR-0001 §5, §7, §8, §11; appendix §A.1, §A.2) on libsodium, through `libsodium-rs`
//! (S4b, ADR-0018). Native only: the web client will use libsodium.js and is checked against the same test vectors.
//!
//! Only the primitives of ADR-0001 §11 are used, each through the binding: `crypto_aead_xchacha20poly1305_ietf`,
//! `crypto_secretstream_xchacha20poly1305`, `crypto_pwhash` (Argon2id13), `crypto_kdf_derive_from_key`, `crypto_generichash`,
//! `sodium_pad`/`sodium_unpad`, `randombytes_buf`, `sodium_memcmp`, `sodium_memzero`. There is no cryptography of our own here:
//! the byte formats, the order of checks and the key hierarchy are.
//!
//! Secrets are typed ([`secret`]), zeroed on drop and never printed. Every parser is total: any input gives a [`CryptoError`],
//! never a panic.

pub mod blob;
pub mod envelope;
pub mod header;
pub mod ids;
pub mod keys;
pub mod operation;
pub mod profile;
pub mod secret;
pub mod words;
pub mod wrap;

mod aead;
mod error;
mod strict_json;

pub use error::{BlobError, CryptoError, ProfileError, WordsError};
pub use ids::AccountId;
pub use keys::Dek;
pub use operation::{Operation, open_operation, seal_operation};
pub use profile::Profile;

#[cfg(test)]
mod blob_vector_tests;
#[cfg(test)]
mod envelope_tests;
#[cfg(test)]
mod forgery_tests;
#[cfg(test)]
mod garbage_tests;
#[cfg(test)]
mod key_tests;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod vectors_tests;
