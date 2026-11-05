//! The envelope: round trips, the limits, and that no open byte can be changed, moved or re-encoded (appendix §B items 2 and 3).
//! All inputs are synthetic and obviously fake.

use crate::envelope::{
    self, FORMAT_VERSION, Layout, MAX_ENVELOPE, MAX_HEADER, MAX_PAYLOAD, PAD_BLOCK, PREFIX_LEN,
    peek,
};
use crate::error::CryptoError;
use crate::header::{GroupStamp, Header, Kind};
use crate::ids::AccountId;
use crate::keys::Dek;
use crate::operation::{Operation, open_operation, seal_operation};
use crate::secret::DekKey;
use std::collections::BTreeMap;

const ACCOUNT: &str = "11111111-1111-4111-8111-111111111111";
const OTHER_ACCOUNT: &str = "22222222-2222-4222-8222-222222222222";
const ID: &str = "33333333-3333-4333-8333-333333333333";
const OTHER_ID: &str = "44444444-4444-4444-8444-444444444444";
const OP: &str = "55555555-5555-4555-8555-555555555555";
const DEVICE: &str = "66666666-6666-4666-8666-666666666666";

#[test]
fn an_upsert_round_trips_with_exact_group_bytes() {
    let operation = upsert();
    let bytes = seal_operation(&account(), &dek(1), &operation).expect("seal");
    assert_eq!(
        open_operation(&bytes, &account(), &dek(1)).expect("open"),
        operation
    );
}

#[test]
fn a_purged_round_trips_with_an_empty_payload_that_is_still_padded() {
    let operation = purged();
    let bytes = seal_operation(&account(), &dek(1), &operation).expect("seal");
    let header_len = operation.header.to_bytes().expect("header").len();
    assert_eq!(
        bytes.len(),
        PREFIX_LEN + header_len + 24 + PAD_BLOCK + 16,
        "an empty payload is one block"
    );
    assert_eq!(
        open_operation(&bytes, &account(), &dek(1)).expect("open"),
        operation
    );
}

#[test]
fn the_length_hides_the_payload_inside_a_block() {
    let one = envelope_with_payload(&[b'a'; 10]);
    let two = envelope_with_payload(&[b'a'; 1000]);
    let three = envelope_with_payload(&[b'a'; 1023]);
    let four = envelope_with_payload(&[b'a'; 1024]);
    assert_eq!(one.len(), two.len());
    assert_eq!(two.len(), three.len());
    assert_eq!(
        three.len() + PAD_BLOCK,
        four.len(),
        "a payload of a whole block gets a whole extra block"
    );
}

#[test]
fn the_limits_of_the_format_are_exact() {
    let at_limit = envelope_with_payload(&vec![b'x'; MAX_PAYLOAD]);
    assert_eq!(
        at_limit.len(),
        PREFIX_LEN + 20 + 24 + MAX_PAYLOAD + PAD_BLOCK + 16
    );
    assert!(at_limit.len() <= MAX_ENVELOPE);
    assert_eq!(
        envelope::seal(&account(), &dek(1), b"{}", &vec![b'x'; MAX_PAYLOAD + 1]).unwrap_err(),
        CryptoError::Input("payload above 1 MiB")
    );
    assert!(envelope::seal(&account(), &dek(1), &vec![b' '; MAX_HEADER], b"{}").is_ok());
    assert!(envelope::seal(&account(), &dek(1), &vec![b' '; MAX_HEADER + 1], b"{}").is_err());
    assert!(envelope::seal(&account(), &dek(1), b"", b"{}").is_err());
}

#[test]
fn every_bit_of_the_open_bytes_and_the_ciphertext_is_covered() {
    let bytes = seal_operation(&account(), &dek(1), &upsert()).expect("seal");
    for position in 0..bytes.len() {
        for bit in 0..8 {
            let mut changed = bytes.clone();
            if let Some(byte) = changed.get_mut(position) {
                *byte ^= 1 << bit;
            }
            let result = open_operation(&changed, &account(), &dek(1));
            assert!(result.is_err(), "byte {position} bit {bit} was accepted");
        }
    }
}

#[test]
fn a_changed_byte_is_refused_by_the_aead_before_anything_is_parsed() {
    let bytes = seal_operation(&account(), &dek(1), &upsert()).expect("seal");
    let header_len = upsert().header.to_bytes().expect("header").len();
    // Inside the header and the nonce and the ciphertext: the format, key id and header_len bytes have their own errors below.
    for position in PREFIX_LEN..bytes.len() {
        let mut changed = bytes.clone();
        if let Some(byte) = changed.get_mut(position) {
            *byte ^= 0x01;
        }
        assert_eq!(
            envelope::open(&changed, &account(), &dek(1)).unwrap_err(),
            CryptoError::Authentication,
            "byte {position} (the header is bytes {PREFIX_LEN}..{})",
            PREFIX_LEN + header_len
        );
    }
}

#[test]
fn each_open_field_of_the_header_is_covered_when_replaced_by_a_valid_other_value() {
    let operation = upsert();
    let bytes = seal_operation(&account(), &dek(1), &operation).expect("seal");
    let other_hlc = format!("0000000001001-00000-{DEVICE}");
    let variants: Vec<(&str, Header)> = vec![
        (
            "hlc of a group",
            with_group(&operation.header, "body", &other_hlc, 1),
        ),
        (
            "sv of a group",
            with_group(&operation.header, "body", &stamp(1000, 0), 2),
        ),
        (
            "id (moved to another record)",
            Header {
                id: OTHER_ID.to_string(),
                ..operation.header.clone()
            },
        ),
        (
            "op",
            Header {
                op: OTHER_ID.to_string(),
                ..operation.header.clone()
            },
        ),
        (
            "entity",
            Header {
                entity: "chat".to_string(),
                ..operation.header.clone()
            },
        ),
        (
            "an extra group",
            with_group(&operation.header, "flags", &stamp(1000, 0), 1),
        ),
        ("kind and purged", purged().header),
    ];
    for (what, header) in variants {
        let forged = replace_header(&bytes, &header.to_bytes().expect("header"));
        assert_eq!(
            envelope::open(&forged, &account(), &dek(1)).unwrap_err(),
            CryptoError::Authentication,
            "{what}"
        );
    }
}

#[test]
fn a_purged_cannot_be_forged_or_turned_into_an_upsert() {
    let bytes = seal_operation(&account(), &dek(1), &purged()).expect("seal");
    let forged = replace_header(&bytes, &upsert().header.to_bytes().expect("header"));
    assert_eq!(
        envelope::open(&forged, &account(), &dek(1)).unwrap_err(),
        CryptoError::Authentication
    );
    let mut weaker = purged();
    weaker.header.purged_of = Some(BTreeMap::from([("core".to_string(), stamp(5, 0))]));
    let forged = replace_header(&bytes, &weaker.header.to_bytes().expect("header"));
    assert_eq!(
        envelope::open(&forged, &account(), &dek(1)).unwrap_err(),
        CryptoError::Authentication
    );
}

#[test]
fn another_account_or_another_key_cannot_open_it() {
    let bytes = seal_operation(&account(), &dek(1), &upsert()).expect("seal");
    let other_account = AccountId::parse(OTHER_ACCOUNT).expect("account");
    assert_eq!(
        envelope::open(&bytes, &other_account, &dek(1)).unwrap_err(),
        CryptoError::Authentication
    );
    let other_key = Dek::new(1, DekKey::from_bytes([0x99; 32])).expect("dek");
    assert_eq!(
        envelope::open(&bytes, &account(), &other_key).unwrap_err(),
        CryptoError::Authentication
    );
}

#[test]
fn an_unknown_format_or_key_id_is_told_apart_from_a_forgery() {
    let bytes = seal_operation(&account(), &dek(1), &upsert()).expect("seal");
    let mut future = bytes.clone();
    future[0] = FORMAT_VERSION + 1;
    assert_eq!(
        envelope::open(&future, &account(), &dek(1)).unwrap_err(),
        CryptoError::UnknownFormat(2)
    );
    // A future format may lay its bytes out differently: even one byte long it is unknown, not "too short".
    assert_eq!(
        envelope::open(&[2], &account(), &dek(1)).unwrap_err(),
        CryptoError::UnknownFormat(2)
    );
    assert_eq!(
        envelope::open(&[0], &account(), &dek(1)).unwrap_err(),
        CryptoError::UnknownFormat(0)
    );
    let rotated = seal_operation(&account(), &dek(2), &upsert()).expect("seal");
    assert_eq!(
        envelope::open(&rotated, &account(), &dek(1)).unwrap_err(),
        CryptoError::UnknownKeyId(2)
    );
}

#[test]
fn a_giant_length_is_refused_before_anything_is_allocated() {
    // Only the 9 prefix bytes and the declared total are looked at: nothing here owns a buffer the size of the claim.
    let mut prefix = [FORMAT_VERSION, 0, 0, 0, 1, 0, 0, 0, 0];
    prefix[5..9].copy_from_slice(&u32::MAX.to_be_bytes());
    assert_eq!(
        Layout::parse(&prefix, 100).unwrap_err(),
        CryptoError::HeaderLength
    );
    assert_eq!(
        Layout::parse(&prefix, 5_000_000_000).unwrap_err(),
        CryptoError::TooLarge
    );
    assert_eq!(
        Layout::parse(&prefix, usize::MAX).unwrap_err(),
        CryptoError::TooLarge
    );
    prefix[5..9].copy_from_slice(&0u32.to_be_bytes());
    assert_eq!(
        Layout::parse(&prefix, 2000).unwrap_err(),
        CryptoError::HeaderLength
    );
    prefix[5..9].copy_from_slice(&(MAX_HEADER as u32 + 1).to_be_bytes());
    assert_eq!(
        Layout::parse(&prefix, 6000).unwrap_err(),
        CryptoError::HeaderLength
    );
    prefix[5..9].copy_from_slice(&20u32.to_be_bytes());
    let ok_total = PREFIX_LEN + 20 + 24 + PAD_BLOCK + 16;
    assert!(Layout::parse(&prefix, ok_total).is_ok());
    assert_eq!(
        Layout::parse(&prefix, ok_total - 1).unwrap_err(),
        CryptoError::CiphertextLength
    );
    assert_eq!(
        Layout::parse(&prefix, ok_total + 1).unwrap_err(),
        CryptoError::CiphertextLength
    );
    assert_eq!(
        Layout::parse(&prefix, 40).unwrap_err(),
        CryptoError::Truncated
    );
    assert_eq!(
        Layout::parse(&prefix, PREFIX_LEN + 20 + 24 + 16).unwrap_err(),
        CryptoError::CiphertextLength
    );
    assert_eq!(
        Layout::parse(&prefix[..8], 100).unwrap_err(),
        CryptoError::Truncated
    );
    let too_long = PREFIX_LEN + 20 + 24 + MAX_PAYLOAD + 2 * PAD_BLOCK + 16;
    assert_eq!(
        Layout::parse(&prefix, too_long).unwrap_err(),
        CryptoError::CiphertextLength
    );
}

#[test]
fn peek_reads_the_header_without_a_key_and_does_not_authenticate_it() {
    let bytes = seal_operation(&account(), &dek(1), &upsert()).expect("seal");
    let peeked = peek(&bytes).expect("peek");
    assert_eq!(peeked.key_id, 1);
    assert_eq!(
        Header::parse(peeked.header).expect("header"),
        upsert().header
    );
    assert!(peek(&bytes[..40]).is_err());
    assert!(peek(&[]).is_err());
}

#[test]
fn an_authentic_envelope_with_bad_padding_or_json_is_malformed_not_tampered() {
    // Authentic bytes from a faulty client: the AEAD passes, the parse fails, and the caller quarantines (§5.4).
    let header = upsert().header.to_bytes().expect("header");
    let cases: [(&str, &[u8]); 5] = [
        ("not json", b"not json"),
        (
            "duplicate group keys",
            br#"{"groups":{"body":{"a":1},"body":{"a":2}}}"#,
        ),
        (
            "a group set that differs from the header",
            br#"{"groups":{"body":{"a":1}}}"#,
        ),
        (
            "an extra top-level key",
            br#"{"groups":{"body":{"a":1},"core":{"a":1}},"x":1}"#,
        ),
        (
            "a group that is not an object",
            br#"{"groups":{"body":[1],"core":{"a":1}}}"#,
        ),
    ];
    for (what, payload) in cases {
        let bytes = envelope::seal(&account(), &dek(1), &header, payload).expect("seal");
        assert!(
            matches!(
                open_operation(&bytes, &account(), &dek(1)),
                Err(CryptoError::Malformed(_))
            ),
            "{what}"
        );
    }
    let nested = br#"{"groups":{"body":{"a":{"b":1,"b":2}},"core":{"a":1}}}"#;
    let bytes = envelope::seal(&account(), &dek(1), &header, nested).expect("seal");
    assert!(matches!(
        open_operation(&bytes, &account(), &dek(1)),
        Err(CryptoError::Malformed("duplicate key"))
    ));
}

#[test]
fn a_purged_with_a_payload_or_an_upsert_with_none_is_malformed() {
    let purged_header = purged().header.to_bytes().expect("header");
    let bytes =
        envelope::seal(&account(), &dek(1), &purged_header, br#"{"groups":{}}"#).expect("seal");
    assert!(matches!(
        open_operation(&bytes, &account(), &dek(1)),
        Err(CryptoError::Malformed(_))
    ));
    let bytes = envelope::seal(
        &account(),
        &dek(1),
        &upsert().header.to_bytes().expect("header"),
        b"",
    )
    .expect("seal");
    assert!(matches!(
        open_operation(&bytes, &account(), &dek(1)),
        Err(CryptoError::Malformed(_))
    ));
}

#[test]
fn the_opened_value_prints_no_payload() {
    let bytes = seal_operation(&account(), &dek(1), &upsert()).expect("seal");
    let opened = envelope::open(&bytes, &account(), &dek(1)).expect("open");
    let shown = format!("{opened:?}");
    assert!(!shown.contains("fake body text"), "{shown}");
    assert!(shown.contains("<redacted>"));
}

fn account() -> AccountId {
    AccountId::parse(ACCOUNT).expect("account")
}

fn dek(key_id: u32) -> Dek {
    Dek::new(key_id, DekKey::from_bytes([0x42; 32])).expect("dek")
}

fn stamp(wall: u64, counter: u32) -> String {
    format!("{wall:013}-{counter:05}-{DEVICE}")
}

fn upsert() -> Operation {
    let groups = BTreeMap::from([
        (
            "body".to_string(),
            r#"{"base":null,"body_md":"fake body text","edited_at":null,"supersedes":[]}"#
                .to_string(),
        ),
        (
            "core".to_string(),
            r#"{"created_at":1000,"origin":"user"}"#.to_string(),
        ),
    ]);
    let header = Header {
        entity: "message".to_string(),
        id: ID.to_string(),
        op: OP.to_string(),
        kind: Kind::Upsert,
        groups: BTreeMap::from([
            (
                "body".to_string(),
                GroupStamp {
                    h: stamp(1000, 1),
                    sv: 1,
                },
            ),
            (
                "core".to_string(),
                GroupStamp {
                    h: stamp(1000, 0),
                    sv: 1,
                },
            ),
        ]),
        purged_of: None,
        blob: None,
    };
    Operation { header, groups }
}

fn purged() -> Operation {
    let header = Header {
        entity: "message".to_string(),
        id: ID.to_string(),
        op: OP.to_string(),
        kind: Kind::Purged,
        groups: BTreeMap::new(),
        purged_of: Some(BTreeMap::from([
            ("body".to_string(), stamp(1000, 1)),
            ("core".to_string(), stamp(1000, 0)),
        ])),
        blob: None,
    };
    Operation {
        header,
        groups: BTreeMap::new(),
    }
}

fn with_group(header: &Header, name: &str, hlc: &str, sv: u32) -> Header {
    let mut changed = header.clone();
    changed.groups.insert(
        name.to_string(),
        GroupStamp {
            h: hlc.to_string(),
            sv,
        },
    );
    changed
}

fn envelope_with_payload(payload: &[u8]) -> Vec<u8> {
    envelope::seal(&account(), &dek(1), &[b' '; 20], payload).expect("seal")
}

/// The same nonce and ciphertext under a different open header, with `header_len` adjusted: what a hostile server could do.
fn replace_header(bytes: &[u8], header: &[u8]) -> Vec<u8> {
    let old_len = u32::from_be_bytes([bytes[5], bytes[6], bytes[7], bytes[8]]) as usize;
    let mut out = bytes[..5].to_vec();
    out.extend_from_slice(&(header.len() as u32).to_be_bytes());
    out.extend_from_slice(header);
    out.extend_from_slice(&bytes[PREFIX_LEN + old_len..]);
    out
}
