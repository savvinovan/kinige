//! The attachment blob (ADR-0001 §7, appendix §A.2): `sodium_pad` of the whole file to a multiple of 1024, then
//! `crypto_secretstream_xchacha20poly1305` in 1 MiB chunks, the last one tagged `TAG_FINAL`.
//!
//! ```text
//! P    = sodium_pad(file, 1024)      # always at least one byte of padding, so |P| >= 1024; an empty file is 1024 bytes
//! blob = header(24) || for each 1 MiB chunk of P: push(chunk, ad = none, tag = MESSAGE, or FINAL for the last) -> |chunk| + 17
//! ```
//!
//! Reading takes chunks of exactly `1 MiB + 17` bytes (the last may be shorter), so a chunk cannot be re-cut. A stream that ends
//! before `TAG_FINAL`, and bytes after it, are errors. This version holds the file in memory; the chunk loop is the same one a
//! streaming reader will use (slice 11).

use crate::error::{BlobError, CryptoError};
use crate::secret::BlobKey;
use libsodium_rs::crypto_secretstream::xchacha20poly1305 as stream;
use libsodium_rs::utils;

/// The size of a plaintext chunk (§A.2).
pub const CHUNK: usize = 1 << 20;
/// A pad block (§7).
pub const PAD_BLOCK: usize = 1024;

const FRAME: usize = stream::ABYTES;
const HEADER: usize = stream::HEADERBYTES;

/// Encrypts `file` into a blob under `key`. The stream header is random, so two calls give two different blobs.
pub fn seal_blob(file: &[u8], key: &BlobKey) -> Result<Vec<u8>, CryptoError> {
    let mut padded = utils::pad(file, PAD_BLOCK).map_err(|_| CryptoError::Sodium)?;
    let sealed = push_all(&padded, key);
    utils::memzero(&mut padded);
    sealed
}

/// Decrypts a blob: header, then each chunk in order, then `TAG_FINAL` must be the last thing in the blob, then `sodium_unpad`.
/// The caller checks `sha256` and `byte_size` against the attachment's `core` (a mismatch is the `missing` state, §7).
pub fn open_blob(blob: &[u8], key: &BlobKey) -> Result<Vec<u8>, CryptoError> {
    let header = blob
        .get(..HEADER)
        .ok_or(CryptoError::Blob(BlobError::Truncated))?;
    let header =
        <[u8; HEADER]>::try_from(header).map_err(|_| CryptoError::Blob(BlobError::Truncated))?;
    let mut rest = blob
        .get(HEADER..)
        .ok_or(CryptoError::Blob(BlobError::Truncated))?;
    if rest.is_empty() {
        return Err(CryptoError::Blob(BlobError::MissingFinal));
    }
    let key = stream::Key::from_bytes(key.as_bytes()).map_err(|_| CryptoError::Sodium)?;
    let mut pull = stream::PullState::init_pull(&header, &key).map_err(|_| CryptoError::Sodium)?;
    // The plaintext is never longer than the ciphertext after the header: reserving that much means the buffer never moves, so no copy of
    // the file is left behind in freed memory when it grows (it is zeroed at the end).
    let mut padded: Vec<u8> = Vec::with_capacity(rest.len());
    let result = loop {
        if rest.is_empty() {
            break Err(CryptoError::Blob(BlobError::MissingFinal));
        }
        let (chunk, tail) = rest.split_at(rest.len().min(CHUNK + FRAME));
        let (mut plain, tag) = match pull.pull(chunk, None) {
            Ok(pulled) => pulled,
            Err(_) => break Err(CryptoError::Blob(BlobError::Chunk)),
        };
        padded.extend_from_slice(&plain);
        utils::memzero(&mut plain);
        rest = tail;
        if tag == stream::TAG_FINAL {
            break if rest.is_empty() {
                Ok(())
            } else {
                Err(CryptoError::Blob(BlobError::TrailingBytes))
            };
        }
        if plain.len() != CHUNK {
            break Err(CryptoError::Blob(BlobError::Chunk));
        }
    };
    let file =
        result.and_then(|()| utils::unpad(&padded, PAD_BLOCK).map_err(|_| CryptoError::Padding));
    utils::memzero(&mut padded);
    file
}

fn push_all(padded: &[u8], key: &BlobKey) -> Result<Vec<u8>, CryptoError> {
    let key = stream::Key::from_bytes(key.as_bytes()).map_err(|_| CryptoError::Sodium)?;
    let (mut push, header) = stream::PushState::init_push(&key).map_err(|_| CryptoError::Sodium)?;
    let chunks = padded.len().div_ceil(CHUNK);
    let mut out = Vec::with_capacity(HEADER + padded.len() + chunks * FRAME);
    out.extend_from_slice(&header);
    for (index, chunk) in padded.chunks(CHUNK).enumerate() {
        let tag = if index + 1 == chunks {
            stream::TAG_FINAL
        } else {
            stream::TAG_MESSAGE
        };
        out.extend_from_slice(
            &push
                .push(chunk, None, tag)
                .map_err(|_| CryptoError::Sodium)?,
        );
    }
    Ok(out)
}
