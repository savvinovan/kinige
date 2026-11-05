//! No open byte of an envelope can be changed, moved or re-encoded (appendix §B items 2 and 3): every change is refused by the AEAD,
//! before anything is parsed. All inputs are synthetic and obviously fake.

use crate::envelope::{
    self, FORMAT_VERSION, Layout, MAX_HEADER, MAX_PAYLOAD, PAD_BLOCK, PREFIX_LEN, peek,
};
use crate::error::CryptoError;
use crate::header::Header;
use crate::ids::AccountId;
use crate::keys::Dek;
use crate::operation::{open_operation, seal_operation};
use crate::secret::DekKey;
use crate::test_support::{
    DEVICE, OTHER_ACCOUNT, OTHER_ID, account, dek, purged, replace_header, stamp, upsert,
    with_group,
};
use std::collections::BTreeMap;

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
    // Inside the header, the nonce and the ciphertext. The format, key id and header_len bytes have their own errors below.
    for position in PREFIX_LEN..bytes.len() {
        let mut changed = bytes.clone();
        if let Some(byte) = changed.get_mut(position) {
            *byte ^= 0x01;
        }
        assert_eq!(
            envelope::open(&changed, &account(), &dek(1)).unwrap_err(),
            CryptoError::Authentication,
            "byte {position}"
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
            with_group(&operation.header, "body", &stamp(1000, 1), 2),
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
