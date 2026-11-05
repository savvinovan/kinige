//! The operation envelope, `format_version = 1` (ADR-0001 §5, appendix §A.1). The server stores and serves these bytes; the client
//! seals and opens them.
//!
//! ```text
//! 0      1       format_version = 0x01
//! 1      4       key_id (u32 BE)
//! 5      4       header_len H (u32 BE), 1 <= H <= 4096
//! 9      H       header: UTF-8 JSON, open
//! 9+H    24      nonce = randombytes_buf(24)
//! 33+H   N+16    AEAD(m = sodium_pad(payload, 1024), ad = AD_env, npub = nonce, k = K_env[key_id])
//! AD_env = "kinige.env.v1" || account_id(16) || bytes[0 .. 9+H)
//! ```
//!
//! The AD is the open bytes **as received**, so no field of the header can be changed, moved or re-encoded without the AEAD
//! failing, and a canonical form of the header is not needed. Opening follows the order of §A.4: the size cap, the format, the
//! lengths, the key id, the AEAD, and only then the unpadding and the parse of the header and payload (`header::Header::parse`).

use crate::aead::{self, NONCE_LEN, TAG_LEN};
use crate::error::CryptoError;
use crate::ids::AccountId;
use crate::keys::Dek;
use crate::secret::{SecretBytes, random_array};
use libsodium_rs::utils;

pub const FORMAT_VERSION: u8 = 1;
/// `format_version(1) + key_id(4) + header_len(4)`: the bytes that say how long everything else is.
pub const PREFIX_LEN: usize = 9;
pub const MAX_HEADER: usize = 4096;
/// `sodium_pad` block (§5.1).
pub const PAD_BLOCK: usize = 1024;
/// The limit of the payload before padding: 1 MiB (§5.3).
pub const MAX_PAYLOAD: usize = 1 << 20;
/// A padded payload is at most this long: a payload of exactly 1 MiB gets a whole extra block.
pub const MAX_PADDED: usize = MAX_PAYLOAD + PAD_BLOCK;
/// The largest envelope of any format: checked first, whatever the format byte says (§A.4 step 1).
pub const MAX_ENVELOPE: usize = PREFIX_LEN + MAX_HEADER + NONCE_LEN + MAX_PADDED + TAG_LEN;

const LABEL: &[u8] = b"kinige.env.v1";

/// Where the parts of an envelope are, worked out from the 9 prefix bytes and the length the transport declared, before a single
/// byte is allocated from anything the sender wrote (B item 3). A streaming reader calls this with the first 9 bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    pub key_id: u32,
    pub header_len: usize,
    /// `N + 16`.
    pub ciphertext_len: usize,
}

impl Layout {
    /// Checks, in this order: the size cap; the format byte (an unknown one is `UnknownFormat`, its own layout is not ours to
    /// judge); that there are 9 bytes; `1 <= H <= 4096`; that the declared length fits the header, the nonce and one block; that the ciphertext
    /// is a multiple of 1024 plus 16 and at most `1 MiB + 1024 + 16`.
    pub fn parse(prefix: &[u8], total_len: usize) -> Result<Layout, CryptoError> {
        if total_len > MAX_ENVELOPE {
            return Err(CryptoError::TooLarge);
        }
        // The format comes before the other lengths: a future format may lay its bytes out differently, and it must reach
        // `pending_unknown` (§A.4 step 2) instead of being refused by the rules of this one.
        let format = *prefix.first().ok_or(CryptoError::Truncated)?;
        if format != FORMAT_VERSION {
            return Err(CryptoError::UnknownFormat(format));
        }
        let prefix: &[u8; PREFIX_LEN] = prefix
            .get(..PREFIX_LEN)
            .and_then(|bytes| <&[u8; PREFIX_LEN]>::try_from(bytes).ok())
            .ok_or(CryptoError::Truncated)?;
        let [_, k0, k1, k2, k3, h0, h1, h2, h3] = *prefix;
        let key_id = u32::from_be_bytes([k0, k1, k2, k3]);
        let header_len = usize::try_from(u32::from_be_bytes([h0, h1, h2, h3]))
            .map_err(|_| CryptoError::HeaderLength)?;
        if header_len == 0 || header_len > MAX_HEADER {
            return Err(CryptoError::HeaderLength);
        }
        let fixed = PREFIX_LEN + header_len + NONCE_LEN;
        let ciphertext_len = total_len.checked_sub(fixed).ok_or(CryptoError::Truncated)?;
        let plain_len = ciphertext_len
            .checked_sub(TAG_LEN)
            .ok_or(CryptoError::CiphertextLength)?;
        if !(PAD_BLOCK..=MAX_PADDED).contains(&plain_len) || plain_len % PAD_BLOCK != 0 {
            return Err(CryptoError::CiphertextLength);
        }
        Ok(Layout {
            key_id,
            header_len,
            ciphertext_len,
        })
    }
}

/// The open part of an envelope, read without any key: for the server, which stores the bytes and reads the header (§5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Peeked<'a> {
    pub key_id: u32,
    /// The header's bytes, not yet parsed: parse them with `Header::parse` only if you trust them (after `open`), or to route
    /// them (the server) knowing they are unauthenticated.
    pub header: &'a [u8],
}

/// Lengths and the open header of `bytes`, no key needed. The header is **not** authenticated here.
pub fn peek(bytes: &[u8]) -> Result<Peeked<'_>, CryptoError> {
    let layout = Layout::parse(bytes, bytes.len())?;
    let header = bytes
        .get(PREFIX_LEN..PREFIX_LEN + layout.header_len)
        .ok_or(CryptoError::Truncated)?;
    Ok(Peeked {
        key_id: layout.key_id,
        header,
    })
}

/// A decrypted, unpadded envelope. The payload is zeroed when dropped and never printed.
pub struct Opened {
    pub key_id: u32,
    /// The header's bytes, authentic: they are the ones the AEAD covered.
    pub header: Vec<u8>,
    pub payload: SecretBytes,
}

impl std::fmt::Debug for Opened {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Opened(key_id {}, header {} bytes, payload <redacted>)",
            self.key_id,
            self.header.len()
        )
    }
}

/// Builds an envelope: `header` (its JSON bytes, 1..=4096) and `payload` (0..=1 MiB, empty for a `purged`) sealed under
/// `K_env[dek.key_id()]` for `account` with a fresh random nonce.
pub fn seal(
    account: &AccountId,
    dek: &Dek,
    header: &[u8],
    payload: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    seal_with_nonce(account, dek, header, payload, random_array())
}

/// The nonce is the caller's: only for the test vectors. Reusing a nonce with a key breaks XChaCha20-Poly1305.
pub(crate) fn seal_with_nonce(
    account: &AccountId,
    dek: &Dek,
    header: &[u8],
    payload: &[u8],
    nonce: [u8; NONCE_LEN],
) -> Result<Vec<u8>, CryptoError> {
    if header.is_empty() || header.len() > MAX_HEADER {
        return Err(CryptoError::Input("header length"));
    }
    if payload.len() > MAX_PAYLOAD {
        return Err(CryptoError::Input("payload above 1 MiB"));
    }
    let header_len =
        u32::try_from(header.len()).map_err(|_| CryptoError::Input("header length"))?;
    let mut out = Vec::with_capacity(
        PREFIX_LEN + header.len() + NONCE_LEN + payload.len() + PAD_BLOCK + TAG_LEN,
    );
    out.push(FORMAT_VERSION);
    out.extend_from_slice(&dek.key_id().to_be_bytes());
    out.extend_from_slice(&header_len.to_be_bytes());
    out.extend_from_slice(header);
    let ad = ad_env(account, &out);
    out.extend_from_slice(&nonce);
    let mut padded = utils::pad(payload, PAD_BLOCK).map_err(|_| CryptoError::Sodium)?;
    let sealed = aead::encrypt(dek.env_key().as_bytes(), &nonce, &ad, &padded);
    utils::memzero(&mut padded);
    out.extend_from_slice(&sealed?);
    Ok(out)
}

/// Opens an envelope in the order of §A.4: size cap, format, lengths (`Layout::parse`), `key_id` (an unknown one is
/// `UnknownKeyId`: it goes to `pending_unknown`), the AEAD (`Authentication`: the page is not applied), and the unpadding
/// (`Padding`: authentic but built wrongly, quarantine). Nothing of the header or the payload is parsed here.
pub fn open(bytes: &[u8], account: &AccountId, dek: &Dek) -> Result<Opened, CryptoError> {
    let layout = Layout::parse(bytes, bytes.len())?;
    if layout.key_id != dek.key_id() {
        return Err(CryptoError::UnknownKeyId(layout.key_id));
    }
    let header_end = PREFIX_LEN + layout.header_len;
    let open_bytes = bytes.get(..header_end).ok_or(CryptoError::Truncated)?;
    let nonce_end = header_end + NONCE_LEN;
    let nonce = bytes
        .get(header_end..nonce_end)
        .ok_or(CryptoError::Truncated)?;
    let nonce = <[u8; NONCE_LEN]>::try_from(nonce).map_err(|_| CryptoError::Truncated)?;
    let ciphertext = bytes.get(nonce_end..).ok_or(CryptoError::Truncated)?;
    let mut padded = aead::decrypt(
        dek.env_key().as_bytes(),
        &nonce,
        &ad_env(account, open_bytes),
        ciphertext,
    )?;
    let payload = utils::unpad(&padded, PAD_BLOCK).map_err(|_| CryptoError::Padding);
    utils::memzero(&mut padded);
    let header = bytes
        .get(PREFIX_LEN..header_end)
        .ok_or(CryptoError::Truncated)?
        .to_vec();
    Ok(Opened {
        key_id: layout.key_id,
        header,
        payload: SecretBytes::new(payload?),
    })
}

/// `"kinige.env.v1" || account_id(16) || bytes[0 .. 9+H)`.
pub(crate) fn ad_env(account: &AccountId, open_bytes: &[u8]) -> Vec<u8> {
    let mut ad = Vec::with_capacity(LABEL.len() + 16 + open_bytes.len());
    ad.extend_from_slice(LABEL);
    ad.extend_from_slice(account.as_bytes());
    ad.extend_from_slice(open_bytes);
    ad
}
