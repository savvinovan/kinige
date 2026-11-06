//! The client model: records, the outbox, the cursor, `server_known`, tombstones, quarantine (ADR-0001 §6.1, appendix §C). The rules
//! it applies are `kinige_domain::sync`; this file is the state and the plumbing around them.
//!
//! - `ops`: what the user does (create, edit, move, delete, restore, purge after 30 days);
//! - `push`: building a batch and handling the responses of appendix §A.3;
//! - `pull`: applying a page, tombstones, purge verdicts, conflict revisions, resync and binding;
//! - `view`: what the feed shows (§3.6) and the state compared between clients.

mod ops;
mod pull;
pub mod purge;
mod push;
mod view;

use crate::envelope::{Entity, Envelope, Key, Kind};
use crate::switches::Switches;
use kinige_domain::hlc::{self, Clock, Skew};
pub use kinige_domain::sync::ResyncCause;
use kinige_domain::sync::conflict::uuid_from_bytes;
use kinige_domain::sync::{Snapshot, Versioned};
use std::collections::BTreeMap;
use std::sync::Arc;

pub use ops::{OpError, inbox_key};
pub use view::{FeedItem, FeedSlot};

/// A record in a live table: its groups and whether the server has seen it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Local {
    pub v: Versioned,
    pub server_known: bool,
}

/// A row of `change_outbox`. The row stays until the cursor passes its `seq`, so a pull recognizes the device's own operations.
#[derive(Debug, Clone)]
pub struct OutRow {
    pub env: Arc<Envelope>,
    pub acked_seq: Option<u64>,
    /// Handed to the network at least once (a lost response leaves it unacked and sent).
    pub sent: bool,
    /// A cascade child's `purged` waits for the parent's ack (§6.2 step 3): the `op_id` of the parent's row.
    pub blocked_by: Option<String>,
}

/// A local memory of a purged record (§3.5): the snapshot, the cascade it belongs to, and what arrived for it since.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tombstone {
    pub purged_of: Snapshot,
    /// The id of the cascade's parent; cleared when the server confirms the child's own purge.
    pub cascade_of: Option<String>,
    /// The merged groups of a child that arrived while the tombstone is alive, to recreate it if the parent's tombstone is lifted.
    pub held: Option<Versioned>,
    /// The server has this purge (an ack or a pull). A purge made without an account is not confirmed until binding.
    pub confirmed: bool,
    /// When this device wrote the tombstone, by the shared virtual time (`tombstone.purged_at`, appendix §C.2).
    pub purged_at: u64,
}

/// A record the user must decide about ("Found old entries" and the like); never shown in the feed, never pushed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Quarantined {
    pub key: Key,
    pub v: Versioned,
    pub reason: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// A received HLC more than ten minutes ahead (§2): a warning, never a block.
    ClockWarning,
    /// A received HLC beyond `SKEW_MAX`: applied to the data, not to the clock.
    ClockTooFar,
    /// Equal HLC, different bytes in a group (§3.3).
    EqualHlcDifferentBytes(String),
    /// The server answered `op_conflict`: a client error that must never happen here.
    OpConflict,
    /// An envelope that failed AEAD or parsing.
    BadEnvelope,
    /// An operation could not be made because the new HLC was not above the old one (the clock had not seen it).
    HlcNotAbove(String),
    Resync(ResyncCause),
    /// A tombstone was lifted because an envelope newer than its snapshot arrived (§3.5). The record comes back with the groups of
    /// that envelope; a local `deleted` write that the purge destroyed does not.
    Lifted(Key),
}

/// One write of a field group made by a user action, reported to the oracle (the simulation's record of what really happened).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Write {
    pub device: usize,
    pub key: Key,
    pub group: String,
    pub hlc: String,
    /// The HLC of the group on this device just before the write: the causal parent.
    pub parent: Option<String>,
    pub raw: String,
}

/// A full pull in progress: after the head, local records are compared with what was seen (§6.2, §6.3).
#[derive(Debug, Clone)]
pub struct Resync {
    pub cause: ResyncCause,
    pub seen: BTreeMap<Key, Snapshot>,
}

#[derive(Debug, Clone)]
pub struct Client {
    pub idx: usize,
    pub device: String,
    pub skew_ms: i64,
    pub clock: Clock,
    pub bound: bool,
    pub records: BTreeMap<Key, Local>,
    pub outbox: Vec<OutRow>,
    pub tombstones: BTreeMap<Key, Tombstone>,
    pub quarantine: Vec<Quarantined>,
    pub cursor_generation: u64,
    pub cursor_seq: u64,
    /// The server's drop epoch in the last page this device applied (sent back with the next pull, see `Server::pull`).
    pub known_epoch: u64,
    /// The lineage that came with the `GenerationChanged` answer which put the device into its generation, `(generation, cut_ms)` oldest
    /// first (empty if none did). Sent back with every pull (S4c-2, ADR-0017): the server finds where the device's branch forked.
    pub lineage: Vec<(u64, u64)>,
    /// Pull has reached the head of the server log at least once since the last full resync started (§3.6).
    pub reached_head: bool,
    /// A pull has just reached the head, so a push may follow (ADR-0001 §3.5: "pull first, then push"). One batch uses it up.
    pub cycle_ready: bool,
    pub resync: Option<Resync>,
    pub compact_outbox: bool,
    pub switches: Switches,
    /// The shared virtual time, set by the world before every step (the handlers of responses do not take it as an argument).
    pub now_ms: u64,
    pub events: Vec<Event>,
    /// Writes since the world last drained them.
    pub writes: Vec<Write>,
    counter: u64,
}

impl Client {
    pub fn new(
        idx: usize,
        skew_ms: i64,
        bound: bool,
        compact_outbox: bool,
        switches: Switches,
        now_ms: u64,
    ) -> Client {
        let mut client = Client {
            idx,
            device: device_id(idx),
            skew_ms,
            clock: Clock::default(),
            bound,
            records: BTreeMap::new(),
            outbox: Vec::new(),
            tombstones: BTreeMap::new(),
            quarantine: Vec::new(),
            cursor_generation: 1,
            cursor_seq: 0,
            known_epoch: 0,
            lineage: Vec::new(),
            reached_head: false,
            cycle_ready: false,
            resync: None,
            compact_outbox,
            switches,
            now_ms,
            events: Vec::new(),
            writes: Vec::new(),
            counter: 0,
        };
        client.create_inbox(now_ms);
        client
    }

    /// The device's physical clock: the shared virtual time plus this device's skew.
    pub fn pt(&self, now_ms: u64) -> u64 {
        u64::try_from(i128::from(now_ms) + i128::from(self.skew_ms)).unwrap_or(0)
    }

    /// A restart: the clock is resumed from the stored pair (§2, "Storage and restart").
    pub fn restart(&mut self, now_ms: u64) {
        self.clock = Clock::resume(self.clock.l, self.clock.c, self.pt(now_ms));
        self.cycle_ready = false;
        for row in &mut self.outbox {
            row.sent = row.acked_seq.is_some();
        }
    }

    /// Local event: a new stamp for a group the device writes.
    pub(crate) fn stamp(&mut self, now_ms: u64) -> Result<String, hlc::HlcError> {
        self.clock = self.clock.tick(self.pt(now_ms))?;
        Ok(self.clock.format(&self.device))
    }

    /// Every HLC in the header of an envelope that passed AEAD enters the clock, applied or not (§2).
    pub(crate) fn receive_hlc(&mut self, stamp: &str, now_ms: u64) {
        if self.switches.no_clock_receive {
            return;
        }
        let Ok((remote, _)) = hlc::parse(stamp) else {
            return;
        };
        if let Ok((clock, skew)) = self.clock.receive_checked(remote, self.pt(now_ms)) {
            self.clock = clock;
            match skew {
                Skew::Warning => self.events.push(Event::ClockWarning),
                Skew::TooFar => self.events.push(Event::ClockTooFar),
                Skew::Fine => {}
            }
        }
    }

    pub(crate) fn next_id(&mut self) -> String {
        self.counter += 1;
        // The device in the first two bytes, the counter in the last eight: unique across the simulation, valid as a v4 UUID.
        let mut bytes = [0u8; 16];
        bytes[0] = 0xaa;
        bytes[1] = u8::try_from(self.idx).unwrap_or(0xff);
        bytes[8..].copy_from_slice(&self.counter.to_be_bytes());
        uuid_from_bytes(bytes)
    }

    pub(crate) fn next_op_id(&mut self) -> String {
        self.next_id()
    }

    /// Whether the version of `group` the record has now is confirmed: `server_known` and no unacked outbox row of this very
    /// version (§3.4: "an outbox row without `acked_seq` or `server_known = 0`" is unconfirmed).
    pub(crate) fn confirmed(&self, key: &Key, group: &str) -> bool {
        let Some(local) = self.records.get(key) else {
            return false;
        };
        let Some(current) = local.v.get(group) else {
            return false;
        };
        // A version that came from elsewhere is confirmed by definition (the server had it). An upsert is a snapshot of the whole
        // record, so an unacked row that merely carries someone else's group is not "an unconfirmed version of this device".
        !self.is_own_unconfirmed(key, group, &current.hlc)
    }

    /// Whether `hlc` of `group` is a version this device wrote itself and the server has not acked: its own HLC (the device id is in
    /// the stamp) and either the record never left the device or an unacked row carries it. A version this device wrote and the
    /// server acked earlier, carried again by a later unacked snapshot row, counts as unconfirmed too; that is harmless (at worst
    /// one extra revision) and not worth a per-group ack.
    pub(crate) fn is_own_unconfirmed(&self, key: &Key, group: &str, hlc: &str) -> bool {
        let written_here = hlc::parse(hlc).is_ok_and(|(_, device)| device == self.device);
        if !written_here {
            return false;
        }
        let never_sent = self.records.get(key).is_some_and(|l| !l.server_known);
        never_sent
            || self.outbox.iter().any(|row| {
                row.acked_seq.is_none()
                    && row.env.header.key() == *key
                    && row
                        .env
                        .header
                        .groups
                        .get(group)
                        .is_some_and(|(h, _)| h == hlc)
            })
    }

    /// Writes an `upsert` of the record into the outbox. Before binding nothing is written (§6.2). An unsent older row of the same
    /// record whose groups are all covered is dropped (outbox compaction, §6.1).
    pub(crate) fn enqueue_upsert(&mut self, key: &Key) {
        if !self.bound {
            return;
        }
        let Some(local) = self.records.get(key).map(|l| l.v.clone()) else {
            return;
        };
        if local.is_read_only() {
            return;
        }
        let op_id = self.next_op_id();
        let env = Arc::new(Envelope::upsert(key, op_id, &local));
        if self.compact_outbox {
            self.outbox.retain(|row| {
                let old = &row.env.header;
                let covered = old.kind == Kind::Upsert
                    && old.key() == *key
                    && old
                        .groups
                        .iter()
                        .all(|(g, (h, _))| env.header.groups.get(g).is_some_and(|(nh, _)| nh >= h));
                !(covered && !row.sent && row.acked_seq.is_none())
            });
        }
        self.outbox.push(OutRow {
            env,
            acked_seq: None,
            sent: false,
            blocked_by: None,
        });
    }

    pub(crate) fn enqueue_purged(
        &mut self,
        key: &Key,
        snapshot: Snapshot,
        blocked_by: Option<String>,
    ) -> String {
        let op_id = self.next_op_id();
        let env = Arc::new(Envelope::purged(key, op_id.clone(), snapshot));
        self.outbox.push(OutRow {
            env,
            acked_seq: None,
            sent: false,
            blocked_by,
        });
        op_id
    }

    /// Drops the not-yet-acked rows for records that left the live tables: they would only get a `purged` response.
    pub(crate) fn drop_unacked_upserts(&mut self, keys: &[Key]) {
        self.outbox.retain(|row| {
            !(row.acked_seq.is_none()
                && row.env.header.kind == Kind::Upsert
                && keys.contains(&row.env.header.key()))
        });
    }

    pub fn has_pending_work(&self) -> bool {
        self.outbox.iter().any(|row| row.acked_seq.is_none())
    }

    pub fn live_keys(&self, entity: Entity) -> Vec<Key> {
        self.records
            .keys()
            .filter(|(e, _)| *e == entity)
            .cloned()
            .collect()
    }
}

pub fn device_id(idx: usize) -> String {
    let mut bytes = [0u8; 16];
    bytes[0] = 0xd0;
    bytes[1] = u8::try_from(idx).unwrap_or(0xff);
    uuid_from_bytes(bytes)
}
