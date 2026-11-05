//! The 24 recovery words (ADR-0001 §8.1, decision 6): the English BIP-39 list, **entropy and not the seed**. `rc_root` is the 32
//! bytes of entropy; `to_seed` (PBKDF2 with 2048 rounds, which would only slow an attacker down by a constant) is never called.

use crate::error::{CryptoError, WordsError};
use crate::secret::{KEY_LEN, RcRoot, SecretBytes, random_array, take_key};
use bip39::{Error as Bip39Error, Language, Mnemonic};
use libsodium_rs::utils;

/// 24 words are 264 bits: 256 of entropy and 8 of checksum.
pub const WORD_COUNT: usize = 24;

/// The 24 words as a normalized phrase: lower case, single spaces. Zeroed on drop and never printed; [`Words::phrase`] is for
/// the one screen that shows them for saving.
pub struct Words(SecretBytes);

impl Words {
    /// New words from 32 bytes of `randombytes_buf` (§8.3, §8.6 item 2).
    pub fn generate() -> Result<Words, CryptoError> {
        let mut entropy = random_array::<KEY_LEN>();
        let made = Mnemonic::from_entropy_in(Language::English, &entropy);
        libsodium_rs::utils::memzero(&mut entropy);
        let mnemonic = made.map_err(|_| CryptoError::Sodium)?;
        let phrase = mnemonic.words().collect::<Vec<_>>().join(" ");
        Ok(Words(SecretBytes::new(phrase.into_bytes())))
    }

    /// The user's input (§8.1): ASCII lower case and single spaces, then `parse_in_normalized` of the English list. Exactly 24 words;
    /// a typo into a non-dictionary word is `UnknownWord(position)`, a swap with another dictionary word is caught by the checksum
    /// (with probability 1 - 2^-8, B item 9).
    pub fn parse(input: &str) -> Result<Words, CryptoError> {
        // The capacity is the input's length, and the normalized phrase is never longer (a run of whitespace becomes one byte), so
        // the buffer never moves and no copy of the words is left in freed memory.
        let mut normalized = SecretBytes::new(Vec::with_capacity(input.len()));
        let mut count = 0usize;
        for word in input.split_whitespace() {
            if count > 0 {
                normalized.push_byte(b' ');
            }
            normalized.extend_lowercase(word);
            count += 1;
        }
        if count != WORD_COUNT {
            return Err(CryptoError::Words(WordsError::WordCount(count)));
        }
        let text = std::str::from_utf8(normalized.as_slice())
            .map_err(|_| CryptoError::Words(WordsError::UnknownWord(0)))?;
        let checked = Mnemonic::parse_in_normalized(Language::English, text)
            .map(drop)
            .map_err(words_error);
        utils::stackzero(STACK_RESIDUE);
        checked?;
        Ok(Words(normalized))
    }

    /// The phrase, for showing and saving (§8.6 item 3). Empty if the bytes were somehow not UTF-8, which `parse` and `generate` rule out.
    pub fn phrase(&self) -> &str {
        std::str::from_utf8(self.0.as_slice()).unwrap_or("")
    }

    /// `rc_root`: the entropy behind the words (`to_entropy`, never `to_seed`).
    pub fn rc_root(&self) -> Result<RcRoot, CryptoError> {
        let root =
            entropy_of(self.phrase()).and_then(|entropy| take_key(entropy).map(RcRoot::from_bytes));
        // `bip39` works on arrays on its stack (the bits and the entropy) and does not clear them. Its frames are gone now, so clearing
        // the stack below this one overwrites what they left. Best effort: that it reaches all of it is not verified.
        utils::stackzero(STACK_RESIDUE);
        root
    }
}

/// How much stack to clear after `bip39` returns: its arrays are 264 + 33 + 24 bytes plus its frames; this is a generous bound.
const STACK_RESIDUE: usize = 4096;

fn entropy_of(phrase: &str) -> Result<Vec<u8>, CryptoError> {
    let mnemonic = Mnemonic::parse_in_normalized(Language::English, phrase).map_err(words_error)?;
    let entropy = mnemonic.to_entropy();
    if entropy.len() == KEY_LEN {
        Ok(entropy)
    } else {
        Err(CryptoError::Words(WordsError::WordCount(
            mnemonic.word_count(),
        )))
    }
}

impl std::fmt::Debug for Words {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Words(<redacted>)")
    }
}

fn words_error(error: Bip39Error) -> CryptoError {
    CryptoError::Words(match error {
        Bip39Error::UnknownWord(position) => WordsError::UnknownWord(position),
        Bip39Error::BadWordCount(count) => WordsError::WordCount(count),
        Bip39Error::InvalidChecksum
        | Bip39Error::BadEntropyBitCount(_)
        | Bip39Error::AmbiguousLanguages(_) => WordsError::Checksum,
    })
}
