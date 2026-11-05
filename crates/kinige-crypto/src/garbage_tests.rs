//! Garbage into every opener and parser (CLAUDE.md rule 12): the envelope, the wrap, the blob, the words, the header JSON and the
//! payload JSON. The test is that none of them panics and none accepts; a panic fails the test. The generator is a fixed LCG, so a
//! failure repeats.

use crate::blob::{self, CHUNK};
use crate::envelope::{self, Layout, peek};
use crate::header::{Header, Kind, parse_payload};
use crate::ids::AccountId;
use crate::keys::Dek;
use crate::secret::{BlobKey, DekKey, KekPw, KekRc};
use crate::words::Words;
use crate::wrap::{self, WRAP_LEN};
use crate::{Profile, open_operation};

const SALT: [u8; 16] = [3; 16];

#[test]
fn random_bytes_never_open_as_an_envelope() {
    let (account, dek) = (account(), dek());
    let mut random = Lcg(1);
    for length in (0..64).chain([100, 500, 1047, 1048, 1049, 1100, 2000, 5000]) {
        for _ in 0..20 {
            let mut bytes = random.bytes(length);
            assert!(open_operation(&bytes, &account, &dek).is_err());
            let _ = peek(&bytes);
            // A plausible prefix so that the lengths pass and the AEAD is what refuses.
            if let Some(first) = bytes.first_mut() {
                *first = 1;
            }
            if bytes.len() > 8 {
                bytes[1..5].copy_from_slice(&1u32.to_be_bytes());
                bytes[5..9].copy_from_slice(&20u32.to_be_bytes());
            }
            assert!(open_operation(&bytes, &account, &dek).is_err());
        }
    }
}

#[test]
fn every_truncation_and_extension_of_a_valid_envelope_is_refused() {
    let (account, dek) = (account(), dek());
    let bytes = envelope::seal(&account, &dek, br#"{"entity":"x"}"#, b"{}").expect("seal");
    for cut in 0..bytes.len() {
        assert!(
            envelope::open(&bytes[..cut], &account, &dek).is_err(),
            "cut at {cut}"
        );
    }
    let mut longer = bytes.clone();
    longer.push(0);
    assert!(envelope::open(&longer, &account, &dek).is_err());
    longer.extend_from_slice(&[0; 1023]);
    assert!(envelope::open(&longer, &account, &dek).is_err());
}

#[test]
fn layout_parse_survives_every_prefix_and_total() {
    let mut random = Lcg(2);
    for _ in 0..20_000 {
        let extra = random.next() as usize % 3;
        let prefix = random.bytes(9 + extra);
        let total = random.next() as usize;
        let _ = Layout::parse(&prefix, total);
        let _ = Layout::parse(&prefix, total % 3000);
        let _ = Layout::parse(
            &prefix[..(random.next() as usize % prefix.len())],
            total % 3000,
        );
    }
}

#[test]
fn random_bytes_never_unwrap() {
    let account = account();
    let (kek_pw, kek_rc) = (KekPw::from_bytes([5; 32]), KekRc::from_bytes([6; 32]));
    let mut random = Lcg(3);
    for length in [0, 1, 24, 25, 72, 73, 74, 100, 1000] {
        for _ in 0..50 {
            let mut bytes = random.bytes(length);
            assert!(wrap::unwrap_pw(&bytes, &kek_pw, &account, 1, &SALT, Profile::P1).is_err());
            assert!(wrap::unwrap_rc(&bytes, &kek_rc, &account, 1).is_err());
            if let Some(first) = bytes.first_mut() {
                *first = 1;
            }
            assert!(wrap::unwrap_pw(&bytes, &kek_pw, &account, 1, &SALT, Profile::P1).is_err());
        }
    }
    assert_eq!(WRAP_LEN, 73);
}

#[test]
fn random_bytes_never_open_as_a_blob() {
    let key = BlobKey::from_bytes([9; 32]);
    let mut random = Lcg(4);
    for length in [0, 1, 23, 24, 25, 40, 41, 1000, 1041, 1042, 5000] {
        for _ in 0..30 {
            assert!(blob::open_blob(&random.bytes(length), &key).is_err());
        }
    }
    // A blob cut from a valid one, at every length around the chunk boundary, is refused too (the vectors test the long ones).
    let valid = blob::seal_blob(&[7u8; 3000], &key).expect("seal");
    for cut in 0..valid.len() {
        assert!(
            blob::open_blob(&valid[..cut], &key).is_err(),
            "cut at {cut}"
        );
    }
    assert!(valid.len() < CHUNK);
}

#[test]
fn nonsense_words_are_refused_and_never_panic() {
    let mut random = Lcg(5);
    let samples = [
        String::new(),
        " ".to_string(),
        "abandon".to_string(),
        "abandon ".repeat(24),
        "abandon ".repeat(23) + "zzzzzz",
        "\u{0}".repeat(100),
        "\u{44f}\u{44f}\u{44f} ".repeat(30),
        "ABANDON ".repeat(25),
        "a\u{301} ".repeat(24),
        "word ".repeat(10_000),
    ];
    for sample in &samples {
        assert!(
            Words::parse(sample).is_err(),
            "{:?}",
            &sample.chars().take(20).collect::<String>()
        );
    }
    for _ in 0..200 {
        let text: String = random
            .bytes(200)
            .iter()
            .map(|b| char::from(b % 96 + 32))
            .collect();
        assert!(Words::parse(&text).is_err());
    }
}

#[test]
fn nonsense_headers_and_payloads_are_malformed_and_never_panic() {
    let header = valid_header();
    let mut random = Lcg(6);
    let deep = "[".repeat(5000);
    let samples: [&[u8]; 14] = [
        b"",
        b"{}",
        b"null",
        b"[]",
        b"{\"entity\":1}",
        b"\xff\xfe",
        b"{\"entity\":\"message\",\"entity\":\"chat\"}",
        br#"{"entity":"message","id":"nope","op":"nope","kind":"upsert","groups":{}}"#,
        br#"{"entity":"message","id":"33333333-3333-4333-8333-333333333333","op":"55555555-5555-4555-8555-555555555555","kind":"merged"}"#,
        br#"{"entity":"message","id":"33333333-3333-4333-8333-333333333333","op":"55555555-5555-4555-8555-555555555555","kind":"upsert","groups":{"core":{"h":"x","sv":1}}}"#,
        br#"{"entity":"message","id":"33333333-3333-4333-8333-333333333333","op":"55555555-5555-4555-8555-555555555555","kind":"upsert","groups":{"core":{"h":"0000000001000-00000-66666666-6666-4666-8666-666666666666","sv":0}}}"#,
        br#"{"entity":"message","id":"33333333-3333-4333-8333-333333333333","op":"55555555-5555-4555-8555-555555555555","kind":"purged","groups":{}}"#,
        br#"{"entity":"message","id":"33333333-3333-4333-8333-333333333333","op":"55555555-5555-4555-8555-555555555555","kind":"purged","purged_of":{}}"#,
        deep.as_bytes(),
    ];
    for sample in samples {
        assert!(
            Header::parse(sample).is_err(),
            "{:?}",
            String::from_utf8_lossy(&sample[..sample.len().min(40)])
        );
        assert!(parse_payload(sample, &header).is_err());
    }
    for _ in 0..500 {
        let length = random.next() as usize % 300;
        let bytes = random.bytes(length);
        assert!(Header::parse(&bytes).is_err());
        assert!(parse_payload(&bytes, &header).is_err());
    }
    assert_eq!(header.kind, Kind::Upsert);
}

fn account() -> AccountId {
    AccountId::from_bytes([0x11; 16])
}

fn dek() -> Dek {
    Dek::new(1, DekKey::from_bytes([0x42; 32])).expect("dek")
}

fn valid_header() -> Header {
    let text = br#"{"entity":"message","id":"33333333-3333-4333-8333-333333333333","op":"55555555-5555-4555-8555-555555555555","kind":"upsert","groups":{"core":{"h":"0000000001000-00000-66666666-6666-4666-8666-666666666666","sv":1}}}"#;
    Header::parse(text).expect("a valid header")
}

/// A fixed linear congruential generator: the bytes are arbitrary, not secret, and the same on every run.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 24
    }

    fn bytes(&mut self, length: usize) -> Vec<u8> {
        (0..length).map(|_| self.next() as u8).collect()
    }
}
