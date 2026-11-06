//! With the `real-envelope` feature (S4b) every envelope of the simulation is sealed by `kinige-crypto`. These tests pin what that
//! changes at the protocol's edge: a header changed on the way is a forgery, and an envelope is resent as the same bytes or not at all.
#![cfg(feature = "real-envelope")]

use kinige_domain::sync::{Group, Snapshot, Versioned};
use s4a_sync::envelope::{Entity, Envelope, EnvelopeError, Key};
use s4a_sync::server::{PushResponse, Server};
use s4a_sync::switches::Switches;
use std::sync::Arc;

const DEVICE: &str = "66666666-6666-4666-8666-666666666666";
const MESSAGE: &str = "33333333-3333-4333-8333-333333333333";
const OP: &str = "55555555-5555-4555-8555-555555555555";
const NOW: u64 = 1_762_300_000_000;

fn hlc(wall: u64, counter: u32) -> String {
    format!("{wall:013}-{counter:05}-{DEVICE}")
}

fn key() -> Key {
    (Entity::Message, MESSAGE.to_string())
}

fn record() -> Versioned {
    let mut record = Versioned::new();
    record.set(
        "body",
        Group::new(
            &hlc(NOW, 1),
            1,
            r#"{"base":null,"body_md":"fake","edited_at":null,"supersedes":[]}"#,
        )
        .expect("group"),
    );
    record.set(
        "core",
        Group::new(&hlc(NOW, 0), 1, r#"{"created_at":1,"origin":"user"}"#).expect("group"),
    );
    record
}

#[test]
fn a_real_envelope_opens_to_the_record_it_was_made_from() {
    let envelope = Envelope::upsert(&key(), OP.to_string(), &record());
    assert_eq!(envelope.open().expect("open"), record());
    let purged = Envelope::purged(
        &key(),
        OP.to_string(),
        Snapshot::from([("body".to_string(), hlc(NOW, 1))]),
    );
    assert!(purged.open().expect("open").groups().next().is_none());
}

#[test]
fn a_header_changed_after_sealing_is_a_forgery() {
    let envelope = Envelope::upsert(&key(), OP.to_string(), &record());
    let forgeries: Vec<(&str, Envelope)> = vec![
        (
            "id",
            envelope
                .with_header_changed(|h| h.id = "44444444-4444-4444-8444-444444444444".to_string()),
        ),
        (
            "op id",
            envelope.with_header_changed(|h| {
                h.op_id = "44444444-4444-4444-8444-444444444444".to_string()
            }),
        ),
        (
            "entity",
            envelope.with_header_changed(|h| h.entity = Entity::Chat),
        ),
        (
            "group sv",
            envelope.with_header_changed(|h| {
                h.groups.insert("body".to_string(), (hlc(NOW, 1), 2));
            }),
        ),
        (
            "group hlc",
            envelope.with_header_changed(|h| {
                h.groups.insert("body".to_string(), (hlc(NOW + 1, 1), 1));
            }),
        ),
        (
            "a dropped group",
            envelope.with_header_changed(|h| {
                h.groups.remove("core");
            }),
        ),
    ];
    for (what, forged) in forgeries {
        assert_eq!(
            forged.open().unwrap_err(),
            EnvelopeError::Tampered,
            "{what}"
        );
    }
}

#[test]
fn the_same_envelope_is_a_duplicate_and_a_rebuilt_one_is_not_the_same_bytes() {
    let mut server = Server::new(Switches::default());
    let sent = Arc::new(Envelope::upsert(&key(), OP.to_string(), &record()));
    assert!(matches!(server.push(&sent, NOW), PushResponse::Ok(_)));
    assert!(
        matches!(server.push(&sent, NOW), PushResponse::Duplicate(_)),
        "the stored bytes, resent: a duplicate"
    );
    // The same record under the same op id, sealed again: another random nonce, other bytes. A client must keep the bytes of an
    // outbox row and resend those (appendix §A.1: "stored in this same form in change_outbox.envelope").
    let rebuilt = Arc::new(Envelope::upsert(&key(), OP.to_string(), &record()));
    assert_eq!(server.push(&rebuilt, NOW), PushResponse::OpConflict);
}
