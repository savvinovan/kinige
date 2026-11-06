//! The real envelope for the simulation (feature `real-envelope`, S4b): every envelope is sealed by `kinige-crypto` and opened with it,
//! and a `conflict` revision's id is the keyed hash of ADR-0001 §1. One fixed account and one fixed DEK stand for the keys of the
//! user; they are synthetic.

use crate::envelope::{Entity, EnvelopeError, Header, Kind};
use kinige_crypto::header::{GroupStamp, Header as WireHeader, Kind as WireKind};
use kinige_crypto::ids::conflict_revision_id;
use kinige_crypto::secret::DekKey;
use kinige_crypto::{AccountId, CryptoError, Dek, Operation, open_operation, seal_operation};
use kinige_domain::sync::{ConflictRevision, Group, Versioned};
use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::OnceLock;

/// Seals the record of an envelope. A sealing failure means the simulation built something the format refuses (a group that is not a JSON
/// object, an id that is not a UUID): that is a finding, so it is loud.
pub(crate) fn seal(header: &Header, payload: &BTreeMap<String, String>) -> Vec<u8> {
    let (account, dek) = keys();
    let operation = Operation {
        header: wire_header(header),
        groups: payload.clone(),
    };
    seal_operation(account, dek, &operation).unwrap_or_else(|error| {
        panic!("the simulation built an envelope the format refuses: {error}")
    })
}

/// A client's `open` (appendix §A.4 steps 3-5), then the match of the authentic header against the transport's (step 4): a server that
/// serves an envelope under another id, op or entity, or with other group stamps, is a forgery as much as a changed byte, and the
/// page is not applied. `Tampered` stands for both.
pub(crate) fn open(wire: &[u8], transport: &Header) -> Result<Versioned, EnvelopeError> {
    let (account, dek) = keys();
    // Not a model for slice 11: the simulation does not model `pending_unknown` (ADR-0017), so `UnknownFormat`, `UnknownKeyId` and every
    // other error that is not "authentic but malformed" fall into `Tampered` here. A real client sends the first two to
    // `pending_unknown` and moves the cursor (§A.4 step 2); the envelopes of this simulation are always format 1 and key 1.
    let operation = open_operation(wire, account, dek).map_err(|error| match error {
        CryptoError::Malformed(_) | CryptoError::Padding => EnvelopeError::Malformed,
        _ => EnvelopeError::Tampered,
    })?;
    if operation.header != wire_header(transport) {
        return Err(EnvelopeError::Tampered);
    }
    let mut record = Versioned::new();
    for (name, stamp) in &operation.header.groups {
        let raw = operation.groups.get(name).ok_or(EnvelopeError::Malformed)?;
        record.set(
            name,
            Group::new(&stamp.h, stamp.sv, raw).map_err(EnvelopeError::Group)?,
        );
    }
    Ok(record)
}

/// An identity of the bytes, for "same `op_id`, different bytes" (the nonce is random, so a rebuilt envelope has other bytes).
pub(crate) fn wire_id(wire: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    wire.hash(&mut hasher);
    hasher.finish()
}

/// The same bytes under another open header, the way a hostile server would serve them.
pub(crate) fn with_header(wire: &[u8], header: &Header) -> Vec<u8> {
    let new = wire_header(header).to_bytes().unwrap_or_default();
    let old_len = wire
        .get(5..9)
        .and_then(|b| <[u8; 4]>::try_from(b).ok())
        .map_or(0, |b| u32::from_be_bytes(b) as usize);
    let mut out = wire.get(..5).unwrap_or_default().to_vec();
    out.extend_from_slice(&u32::try_from(new.len()).unwrap_or(0).to_be_bytes());
    out.extend_from_slice(&new);
    out.extend_from_slice(wire.get(9 + old_len..).unwrap_or_default());
    out
}

/// The id of a `conflict` revision: `crypto_generichash(K_rev, input)` (§1).
pub fn revision_id(revision: &ConflictRevision) -> String {
    let (_, dek) = keys();
    conflict_revision_id(dek.rev_key(), revision)
        .unwrap_or_else(|error| panic!("revision id: {error}"))
}

/// The name of an entity on the wire (§5.2).
pub fn entity_name(entity: Entity) -> &'static str {
    match entity {
        Entity::Chat => "chat",
        Entity::Message => "message",
        Entity::Revision => "message_revision",
    }
}

/// The entity of a wire name; `None` for one the simulation does not model.
pub fn entity_of(name: &str) -> Option<Entity> {
    [Entity::Chat, Entity::Message, Entity::Revision]
        .into_iter()
        .find(|entity| entity_name(*entity) == name)
}

/// An envelope as a server serves it (`kinige-server`'s pull): the bytes, and the `entity`, `id` and `op` the server declares next to
/// them. The routing fields come from the declaration and the groups from the open header, so the client's `open` compares the authentic
/// header with exactly what the transport said (ADR-0001 §5.4, ADR-0018 finding 2). `None` if the bytes do not have a readable open header.
pub(crate) fn transport_header(wire: &[u8], entity: &str, id: &str, op: &str) -> Option<Header> {
    let open = kinige_crypto::envelope::peek(wire).ok()?;
    let parsed = WireHeader::parse(open.header).ok()?;
    Some(Header {
        entity: entity_of(entity)?,
        id: id.to_string(),
        op_id: op.to_string(),
        kind: match parsed.kind {
            WireKind::Upsert => Kind::Upsert,
            WireKind::Purged => Kind::Purged,
        },
        groups: parsed
            .groups
            .into_iter()
            .map(|(name, stamp)| (name, (stamp.h, stamp.sv)))
            .collect(),
        purged_of: parsed.purged_of,
    })
}

fn wire_header(header: &Header) -> WireHeader {
    WireHeader {
        entity: entity_name(header.entity).to_string(),
        id: header.id.clone(),
        op: header.op_id.clone(),
        kind: match header.kind {
            Kind::Upsert => WireKind::Upsert,
            Kind::Purged => WireKind::Purged,
        },
        groups: header
            .groups
            .iter()
            .map(|(name, (h, sv))| {
                (
                    name.clone(),
                    GroupStamp {
                        h: h.clone(),
                        sv: *sv,
                    },
                )
            })
            .collect(),
        purged_of: header.purged_of.clone(),
        blob: None,
    }
}

fn keys() -> &'static (AccountId, Dek) {
    static KEYS: OnceLock<(AccountId, Dek)> = OnceLock::new();
    KEYS.get_or_init(|| {
        let dek = Dek::new(1, DekKey::from_bytes([0x42; 32]))
            .unwrap_or_else(|error| panic!("dek: {error}"));
        (AccountId::from_bytes([0x11; 16]), dek)
    })
}
