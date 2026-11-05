//! Fixtures shared by the envelope tests: synthetic ids and a pair of operations. Nothing here is a real value.

use crate::header::{GroupStamp, Header, Kind};
use crate::ids::AccountId;
use crate::keys::Dek;
use crate::operation::Operation;
use crate::secret::DekKey;
use std::collections::BTreeMap;

pub(crate) const ACCOUNT: &str = "11111111-1111-4111-8111-111111111111";
pub(crate) const OTHER_ACCOUNT: &str = "22222222-2222-4222-8222-222222222222";
pub(crate) const ID: &str = "33333333-3333-4333-8333-333333333333";
pub(crate) const OTHER_ID: &str = "44444444-4444-4444-8444-444444444444";
pub(crate) const OP: &str = "55555555-5555-4555-8555-555555555555";
pub(crate) const DEVICE: &str = "66666666-6666-4666-8666-666666666666";

pub(crate) fn account() -> AccountId {
    AccountId::parse(ACCOUNT).expect("account")
}

pub(crate) fn dek(key_id: u32) -> Dek {
    Dek::new(key_id, DekKey::from_bytes([0x42; 32])).expect("dek")
}

pub(crate) fn stamp(wall: u64, counter: u32) -> String {
    format!("{wall:013}-{counter:05}-{DEVICE}")
}

pub(crate) fn upsert() -> Operation {
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

pub(crate) fn purged() -> Operation {
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

pub(crate) fn with_group(header: &Header, name: &str, hlc: &str, sv: u32) -> Header {
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

/// The same nonce and ciphertext under a different open header, with `header_len` adjusted: what a hostile server could do.
pub(crate) fn replace_header(bytes: &[u8], header: &[u8]) -> Vec<u8> {
    let old_len = u32::from_be_bytes([bytes[5], bytes[6], bytes[7], bytes[8]]) as usize;
    let mut out = bytes[..5].to_vec();
    out.extend_from_slice(&(header.len() as u32).to_be_bytes());
    out.extend_from_slice(header);
    out.extend_from_slice(&bytes[crate::envelope::PREFIX_LEN + old_len..]);
    out
}
