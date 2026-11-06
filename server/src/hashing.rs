//! The hash of a backup's files and of an account id: `crypto_generichash` (BLAKE2b, 32 bytes) of libsodium, the binding that
//! `kinige-crypto` pins (ADR-0001 §11). Nothing of our own.

use libsodium_rs::crypto_generichash::{State, generichash};

pub const HASH_LEN: usize = 32;

/// Hashes a file piece by piece.
pub struct Hasher(Option<State>);

impl Hasher {
    pub fn new() -> Hasher {
        Hasher(State::new(None, HASH_LEN).ok())
    }

    pub fn update(&mut self, bytes: &[u8]) {
        if let Some(state) = &mut self.0 {
            state.update(bytes);
        }
    }

    /// Lower-case hex of the hash. Empty if libsodium could not start a hash state, which no hash then equals.
    pub fn finish(mut self) -> String {
        match self.0.as_mut() {
            Some(state) => hex(&state.finalize()),
            None => String::new(),
        }
    }
}

impl Default for Hasher {
    fn default() -> Hasher {
        Hasher::new()
    }
}

/// The 32-byte hash of `bytes` as lower-case hex; `None` if libsodium refuses.
pub fn hash_hex(bytes: &[u8]) -> Option<String> {
    generichash(bytes, None, HASH_LEN)
        .ok()
        .map(|hash| hex(&hash))
}

/// The raw hash of an account id (the deletion journal of ADR-0001 §8.9 keeps this and nothing else of a deleted account).
pub fn account_hash(account_id: &str) -> Option<Vec<u8>> {
    generichash(account_id.as_bytes(), None, HASH_LEN).ok()
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Bytes of a lower-case hex string; `None` for anything else.
pub fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    text.as_bytes()
        .chunks(2)
        .map(|pair| {
            let pair = std::str::from_utf8(pair).ok()?;
            u8::from_str_radix(pair, 16).ok()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_piecewise_hash_equals_the_whole_and_hex_round_trips() {
        let mut hasher = Hasher::new();
        hasher.update(b"fake ");
        hasher.update(b"bytes");
        assert_eq!(Some(hasher.finish()), hash_hex(b"fake bytes"));
        assert_eq!(unhex(&hex(&[0, 1, 254, 255])), Some(vec![0, 1, 254, 255]));
        for bad in ["0", "zz", "+1", "0g"] {
            assert_eq!(unhex(bad), None, "{bad}");
        }
    }
}
