//! Envelopes of the simulation (ADR-0001 §5): plaintext, with the same open header fields as the real ones. Encryption is S4b. The
//! AEAD binding is modelled as a seal over header and payload: the server reads the header and cannot change it unnoticed.

#[cfg(not(feature = "real-envelope"))]
use kinige_domain::sync::Group;
use kinige_domain::sync::{Snapshot, SyncError, Versioned};
use std::collections::BTreeMap;
#[cfg(not(feature = "real-envelope"))]
use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;
#[cfg(not(feature = "real-envelope"))]
use std::hash::Hasher;

/// 30 days in the trash, then a device purges (ADR-0001 §3.5).
pub const PURGE_AFTER_MS: u64 = 30 * DAY_MS;
/// The server keeps `purged` and tombstones for at least this long; an older cursor is stale (§3.5, §6.3).
pub const RETENTION_MS: u64 = 180 * DAY_MS;
pub const DAY_MS: u64 = 24 * 60 * 60 * 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Entity {
    Chat,
    Message,
    /// `message_revision`, in the simulation always `reason = conflict`.
    Revision,
}

/// An entity and its id: the key of a record.
pub type Key = (Entity, String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    Upsert,
    Purged,
}

/// The part of an envelope the server sees (§5.2).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Header {
    pub entity: Entity,
    pub id: String,
    pub op_id: String,
    pub kind: Kind,
    /// `group -> (HLC, sv)`; empty for `purged`.
    pub groups: BTreeMap<String, (String, u32)>,
    /// The HLC snapshot of a `purged` envelope.
    pub purged_of: Option<Snapshot>,
}

impl Header {
    pub fn key(&self) -> Key {
        (self.entity, self.id.clone())
    }

    /// The HLCs of the groups, as a snapshot (what the server tracks per entity).
    pub fn group_hlcs(&self) -> Snapshot {
        self.groups
            .iter()
            .map(|(g, (h, _))| (g.clone(), h.clone()))
            .collect()
    }
}

/// An immutable operation. `payload` is what the server cannot read: the group bytes (§5.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Envelope {
    pub header: Header,
    /// The plaintext groups. With the `real-envelope` feature `open` does not read them (it decrypts `wire`); they stay for the
    /// measurement binary and for the oracle's convenience.
    pub payload: BTreeMap<String, String>,
    seal: u64,
    /// The sealed bytes of ADR-0001 §A.1, made by `kinige-crypto`: what the server stores and serves, and what the outbox keeps and
    /// resends verbatim (a rebuilt envelope has a new random nonce, so the server would answer `op_conflict`).
    #[cfg(feature = "real-envelope")]
    wire: Vec<u8>,
}

impl Envelope {
    /// An `upsert` carrying the full snapshot of a record (§3.1).
    pub fn upsert(key: &Key, op_id: String, record: &Versioned) -> Envelope {
        let mut groups = BTreeMap::new();
        let mut payload = BTreeMap::new();
        for (name, group) in record.groups() {
            groups.insert(name.to_string(), (group.hlc.clone(), group.sv));
            payload.insert(name.to_string(), group.raw.clone());
        }
        Envelope::build(
            Header {
                entity: key.0,
                id: key.1.clone(),
                op_id,
                kind: Kind::Upsert,
                groups,
                purged_of: None,
            },
            payload,
        )
    }

    /// A `purged` envelope: no groups, the snapshot of the HLCs of all the record's groups.
    pub fn purged(key: &Key, op_id: String, purged_of: Snapshot) -> Envelope {
        Envelope::build(
            Header {
                entity: key.0,
                id: key.1.clone(),
                op_id,
                kind: Kind::Purged,
                groups: BTreeMap::new(),
                purged_of: Some(purged_of),
            },
            BTreeMap::new(),
        )
    }

    #[cfg(not(feature = "real-envelope"))]
    fn build(header: Header, payload: BTreeMap<String, String>) -> Envelope {
        let seal = seal_of(&header, &payload);
        Envelope {
            header,
            payload,
            seal,
        }
    }

    #[cfg(feature = "real-envelope")]
    fn build(header: Header, payload: BTreeMap<String, String>) -> Envelope {
        let wire = crate::real::seal(&header, &payload);
        Envelope {
            header,
            payload,
            seal: crate::real::wire_id(&wire),
            wire,
        }
    }

    /// The sealed bytes of ADR-0001 §A.1: what a real server stores and serves.
    #[cfg(feature = "real-envelope")]
    pub fn wire(&self) -> &[u8] {
        &self.wire
    }

    /// An envelope as a real server serves it: the bytes and the `entity`, `id` and `op` it declares next to them (feature `real-server`
    /// builds on it). `None` if the open header cannot be read or the entity is not one the simulation models.
    #[cfg(feature = "real-envelope")]
    pub fn from_wire(wire: Vec<u8>, entity: &str, id: &str, op: &str) -> Option<Envelope> {
        let header = crate::real::transport_header(&wire, entity, id, op)?;
        Some(Envelope {
            header,
            payload: BTreeMap::new(),
            seal: crate::real::wire_id(&wire),
            wire,
        })
    }

    /// The seal, for the server's "same `op_id`, different bytes" comparison.
    pub fn bytes_id(&self) -> u64 {
        self.seal
    }

    /// What a client does before applying (§A.4 steps 3-5): the seal must match (AEAD), and the payload must carry exactly the
    /// groups the header names. Returns the record the envelope carries.
    pub fn open(&self) -> Result<Versioned, EnvelopeError> {
        #[cfg(feature = "real-envelope")]
        {
            crate::real::open(&self.wire, &self.header)
        }
        #[cfg(not(feature = "real-envelope"))]
        self.open_hash_sealed()
    }

    #[cfg(not(feature = "real-envelope"))]
    fn open_hash_sealed(&self) -> Result<Versioned, EnvelopeError> {
        if seal_of(&self.header, &self.payload) != self.seal {
            return Err(EnvelopeError::Tampered);
        }
        let payload_names: Vec<&String> = self.payload.keys().collect();
        let header_names: Vec<&String> = self.header.groups.keys().collect();
        if payload_names != header_names {
            return Err(EnvelopeError::Malformed);
        }
        let mut record = Versioned::new();
        for (name, (hlc, sv)) in &self.header.groups {
            let raw = self.payload.get(name).ok_or(EnvelopeError::Malformed)?;
            record.set(
                name,
                Group::new(hlc, *sv, raw).map_err(EnvelopeError::Group)?,
            );
        }
        Ok(record)
    }

    /// Test hook: an envelope whose header was changed after sealing, as a hostile server would.
    pub fn with_header_changed(&self, change: impl FnOnce(&mut Header)) -> Envelope {
        let mut copy = self.clone();
        change(&mut copy.header);
        #[cfg(feature = "real-envelope")]
        {
            copy.wire = crate::real::with_header(&self.wire, &copy.header);
        }
        copy
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvelopeError {
    /// AEAD failed: the page is not applied, the cursor stays (§5.4).
    Tampered,
    /// Authentic but built wrongly: `malformed` quarantine, the cursor moves (§5.4).
    Malformed,
    Group(SyncError),
}

#[cfg(not(feature = "real-envelope"))]
fn seal_of(header: &Header, payload: &BTreeMap<String, String>) -> u64 {
    let mut hasher = DefaultHasher::new();
    header.hash(&mut hasher);
    payload.hash(&mut hasher);
    hasher.finish()
}
