//! The envelope: round trips, the limits and what is malformed. Forgery of the open bytes is in `forgery_tests`.
//! All inputs are synthetic and obviously fake.

use crate::envelope::{self, MAX_ENVELOPE, MAX_HEADER, MAX_PAYLOAD, PAD_BLOCK, PREFIX_LEN};
use crate::error::CryptoError;
use crate::operation::{open_operation, seal_operation};
use crate::test_support::{account, dek, purged, upsert};

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
fn an_authentic_envelope_with_bad_json_is_malformed_not_tampered() {
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

fn envelope_with_payload(payload: &[u8]) -> Vec<u8> {
    envelope::seal(&account(), &dek(1), &[b' '; 20], payload).expect("seal")
}
