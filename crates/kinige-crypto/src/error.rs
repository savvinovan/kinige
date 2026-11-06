//! One error type for the whole crate. Variants carry no secret and no content: a number, a position or a fixed reason at most,
//! so formatting an error is always safe to log (CLAUDE.md, code style 1).

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CryptoError {
    /// The envelope is shorter than its own layout needs (§A.1).
    Truncated,
    /// `header_len` is 0 or above 4096 (§A.1).
    HeaderLength,
    /// The ciphertext is not a multiple of 1024 plus 16, or is above `1 MiB + 1024 + 16` (§A.1).
    CiphertextLength,
    /// The whole envelope is above the cap that is applied before anything else is read (§A.4 step 1).
    TooLarge,
    /// `format_version` this client does not know: the envelope goes to `pending_unknown` (§4, §A.4 step 2).
    UnknownFormat(u8),
    /// `key_id` this client has no key for: also `pending_unknown` (§A.4 step 2).
    UnknownKeyId(u32),
    /// `wrap_version` this client does not know (§A.1).
    UnknownWrapVersion(u8),
    /// The AEAD refused: a changed byte, another account, another key. The page is not applied (§5.4).
    Authentication,
    /// The envelope is authentic but `sodium_unpad` refused it: built by a faulty client, quarantine `malformed` (§5.4).
    Padding,
    /// The envelope is authentic but its header or payload does not parse: quarantine `malformed` (§5.4).
    Malformed(&'static str),
    /// A refused Argon2id profile (§8.2).
    Profile(ProfileError),
    /// The 24 words do not parse (§8.1).
    Words(WordsError),
    /// A damaged attachment blob (§A.2).
    Blob(BlobError),
    /// The caller's own input is outside the limits (a payload above 1 MiB, a wrong length): a bug of the caller, not of the wire.
    Input(&'static str),
    /// The binding failed where it cannot for valid input (out of memory in Argon2id, for one).
    Sodium,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileError {
    /// A profile number outside the closed set: the upper memory bound (§8.2).
    Unknown(u8),
    /// A profile lower than the one this device has seen: refused before deriving (§8.2, decision 8).
    LowerThanSeen { sent: u8, seen: u8 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordsError {
    /// Not exactly 24 words (a 12-word phrase is a valid BIP-39 phrase but has 128 bits, which is not our key).
    WordCount(usize),
    /// A word that is not in the English list, by its position (B item 9).
    UnknownWord(usize),
    /// A dictionary word was changed for another: caught by the 8 checksum bits.
    Checksum,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlobError {
    /// The blob is shorter than a stream header and one chunk.
    Truncated,
    /// The stream ended before a chunk tagged `TAG_FINAL` (§A.2).
    MissingFinal,
    /// Bytes follow the chunk tagged `TAG_FINAL` (§A.2).
    TrailingBytes,
    /// A chunk did not authenticate.
    Chunk,
}

impl CryptoError {
    /// A stable category for the log (`StoreError::category` style): no content, no numbers.
    pub fn category(&self) -> &'static str {
        match self {
            CryptoError::Truncated
            | CryptoError::HeaderLength
            | CryptoError::CiphertextLength
            | CryptoError::TooLarge => "length",
            CryptoError::UnknownFormat(_)
            | CryptoError::UnknownKeyId(_)
            | CryptoError::UnknownWrapVersion(_) => "unknown_version",
            CryptoError::Authentication => "authentication",
            CryptoError::Padding | CryptoError::Malformed(_) => "malformed",
            CryptoError::Profile(_) => "profile",
            CryptoError::Words(_) => "words",
            CryptoError::Blob(_) => "blob",
            CryptoError::Input(_) => "input",
            CryptoError::Sodium => "sodium",
        }
    }
}

impl fmt::Display for CryptoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CryptoError::Malformed(why) | CryptoError::Input(why) => {
                write!(f, "{}: {why}", self.category())
            }
            other => f.write_str(other.category()),
        }
    }
}

impl std::error::Error for CryptoError {}
