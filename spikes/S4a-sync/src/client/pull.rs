//! Pull: applying a page and moving the cursor as one step, the order of appendix §A.4 (client, pull), conflict revisions, the first
//! binding and a full resync (ADR-0001 §6.2, §6.3).

use super::purge::message_id_of;
use super::{Client, Event, Local, Quarantined, Resync};
use crate::envelope::{Entity, Envelope, EnvelopeError, Key, Kind, RETENTION_MS};
use crate::server::PullResponse;
use crate::switches::Switches;
use kinige_domain::sync::conflict::uuid_from_bytes;
use kinige_domain::sync::{
    Facts, Merged, ResyncAction, ResyncCause, Snapshot, TombstoneVerdict, Versioned,
    conflict_revision, envelope_vs_tombstone, merge, needs_conflict_revision, newer_than_snapshot,
    resync_action,
};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

impl Client {
    /// Whether a pull makes sense: the device has an account, or is binding to one.
    pub fn can_pull(&self) -> bool {
        self.bound || self.resync.is_some()
    }

    /// The cursor `(generation, seq)` and the drop epoch the device last saw.
    pub fn pull_request(&self) -> (u64, u64, u64) {
        (self.cursor_generation, self.cursor_seq, self.known_epoch)
    }

    /// Starts the first binding (§6.2): a full pull from `seq = 0`, then envelopes for everything that never left the device.
    pub fn begin_binding(&mut self, server_generation: u64) {
        if self.bound || self.resync.is_some() {
            return;
        }
        self.cursor_generation = server_generation;
        self.cursor_seq = 0;
        self.reached_head = false;
        self.resync = Some(Resync {
            cause: ResyncCause::Binding,
            seen: Default::default(),
        });
        self.events.push(Event::Resync(ResyncCause::Binding));
    }

    /// A page and the cursor are applied together (§6.1). `order` lets the network deliver the page's envelopes in another order.
    pub fn on_pull_response(
        &mut self,
        response: PullResponse,
        now_ms: u64,
        order: impl FnOnce(&mut Vec<(u64, std::sync::Arc<Envelope>)>),
    ) {
        match response {
            PullResponse::GenerationChanged(generation) => {
                if self.switches.ignore_generation {
                    self.cursor_generation = generation;
                    return;
                }
                let stale = now_ms.saturating_sub(self.last_pull_ms) > RETENTION_MS;
                let cause = match self.resync.as_ref().map(|r| r.cause) {
                    Some(ResyncCause::Binding) => ResyncCause::Binding,
                    _ if stale => ResyncCause::StaleAndGeneration,
                    _ => ResyncCause::GenerationChanged,
                };
                self.cursor_generation = generation;
                self.start_resync(cause);
            }
            PullResponse::Stale => {
                let cause = match self.resync.as_ref().map(|r| r.cause) {
                    Some(ResyncCause::Binding) => ResyncCause::Binding,
                    _ => ResyncCause::StaleCursor,
                };
                self.start_resync(cause);
            }
            PullResponse::Page {
                after,
                mut items,
                next,
                head,
                epoch,
            } => {
                if after != self.cursor_seq {
                    // A delayed answer to an earlier request: the envelopes are not lost, the cursor is not moved by it.
                    return;
                }
                order(&mut items);
                for (seq, env) in &items {
                    self.apply_envelope(*seq, env, now_ms);
                }
                self.cursor_seq = next.max(self.cursor_seq);
                self.known_epoch = self.known_epoch.max(epoch);
                self.last_pull_ms = now_ms;
                let cursor = self.cursor_seq;
                self.outbox
                    .retain(|row| row.acked_seq.is_none_or(|seq| seq > cursor));
                if cursor >= head {
                    self.reached_head = true;
                    if self.resync.is_some() {
                        self.finish_resync();
                    }
                    // The cycle is always "pull first, then push" (§3.5): a push is allowed only right after a pull reached the head.
                    self.cycle_ready = true;
                }
            }
        }
    }

    /// A full pull from `seq = 0` (§6.3). A change of `generation` also forgets the acks: everything unconfirmed is sent again.
    fn start_resync(&mut self, cause: ResyncCause) {
        if matches!(
            cause,
            ResyncCause::GenerationChanged | ResyncCause::StaleAndGeneration
        ) {
            for row in &mut self.outbox {
                row.acked_seq = None;
                row.sent = false;
            }
            for tombstone in self.tombstones.values_mut() {
                tombstone.confirmed = false;
            }
        }
        self.cursor_seq = 0;
        self.reached_head = false;
        self.cycle_ready = false;
        self.resync = Some(Resync {
            cause,
            seen: Default::default(),
        });
        self.events.push(Event::Resync(cause));
    }

    fn apply_envelope(&mut self, seq: u64, env: &Envelope, now_ms: u64) {
        let header = &env.header;
        let incoming = match env.open() {
            Ok(record) => record,
            Err(EnvelopeError::Tampered) => {
                self.events.push(Event::BadEnvelope);
                return;
            }
            Err(_) => {
                // Authentic but malformed: quarantine, the cursor moves (§5.4).
                self.events.push(Event::BadEnvelope);
                return;
            }
        };
        // The clock receives every HLC of the header, applied or not (§2).
        let stamps: Vec<String> = header
            .groups
            .values()
            .map(|(h, _)| h.clone())
            .chain(header.purged_of.iter().flat_map(|s| s.values().cloned()))
            .collect();
        for stamp in &stamps {
            self.receive_hlc(stamp, now_ms);
        }
        let key = header.key();
        if let Some(resync) = &mut self.resync {
            if header.kind == Kind::Upsert {
                let seen = resync.seen.entry(key.clone()).or_default();
                for (group, (h, _)) in &header.groups {
                    let slot = seen.entry(group.clone()).or_default();
                    if *h > *slot {
                        slot.clone_from(h);
                    }
                }
            }
        }
        // The device's own operation: recognized by `op_id`, not merged again. A lost ack is recovered here.
        if let Some(row) = self
            .outbox
            .iter()
            .find(|row| row.env.header.op_id == header.op_id)
        {
            if row.acked_seq.is_none() {
                let snapshot = header.purged_of.clone().unwrap_or_default();
                self.acked(&header.op_id, &key, header.kind, &snapshot, seq);
            }
            return;
        }
        match header.kind {
            Kind::Purged => {
                let snapshot = header.purged_of.clone().unwrap_or_default();
                self.pulled_purged(&key, &snapshot);
            }
            Kind::Upsert => self.apply_upsert(&key, incoming, &header.group_hlcs()),
        }
    }

    fn apply_upsert(&mut self, key: &Key, incoming: Versioned, group_hlcs: &Snapshot) {
        if let Some(tombstone) = self.tombstones.get(key) {
            match envelope_vs_tombstone(group_hlcs, &tombstone.purged_of) {
                TombstoneVerdict::Lift => self.lift_tombstone(key),
                TombstoneVerdict::Drop => {
                    if tombstone.cascade_of.is_some() {
                        // A cascade child: its groups are held so that it can come back if the parent's tombstone is lifted.
                        let held = match tombstone.held.clone() {
                            Some(held) => merge(&held, &incoming).record,
                            None => incoming,
                        };
                        if let Some(tombstone) = self.tombstones.get_mut(key) {
                            tombstone.held = Some(held);
                        }
                    }
                    return;
                }
            }
        }
        if key.0 == Entity::Revision {
            // A revision that arrives after the `purged` of its message is deleted locally and purged (§3.5).
            if let Some(message) = message_id_of(&incoming) {
                if self
                    .tombstones
                    .contains_key(&(Entity::Message, message.clone()))
                {
                    self.tombstones.insert(
                        key.clone(),
                        super::Tombstone {
                            purged_of: group_hlcs.clone(),
                            cascade_of: Some(message),
                            held: None,
                            confirmed: false,
                            purged_at: self.now_ms,
                        },
                    );
                    if self.bound {
                        self.enqueue_purged(key, group_hlcs.clone(), None);
                    }
                    return;
                }
            }
        }
        let Some(local) = self.records.get(key).map(|l| l.v.clone()) else {
            self.records.insert(
                key.clone(),
                Local {
                    v: incoming,
                    server_known: true,
                },
            );
            return;
        };
        let merged = merge_with_switches(&local, &incoming, &self.switches);
        for group in &merged.equal_hlc_different_bytes {
            self.events
                .push(Event::EqualHlcDifferentBytes(group.clone()));
        }
        if key.0 == Entity::Message && !self.switches.no_conflict_revision {
            for contest in &merged.contests {
                let own = contest.loser_is_local
                    && self.is_own_unconfirmed(key, &contest.group, &contest.loser.hlc);
                if needs_conflict_revision(&contest.group, &contest.winner, &contest.loser, own) {
                    self.make_revision(key, &contest.group, &contest.loser);
                }
            }
        }
        if let Some(local) = self.records.get_mut(key) {
            local.v = merged.record;
            local.server_known = true;
        }
    }

    /// The `conflict` revision of a losing version: the same id and bytes on every device (§3.4).
    fn make_revision(&mut self, message: &Key, group: &str, loser: &kinige_domain::sync::Group) {
        let Ok(revision) = conflict_revision(&message.1, group, loser) else {
            return;
        };
        let key = (
            Entity::Revision,
            uuid_from_bytes(toy_hash(&revision.id_input)),
        );
        if self.records.contains_key(&key)
            || self.tombstones.contains_key(&key)
            || self.tombstones.contains_key(message)
        {
            return;
        }
        let mut v = Versioned::new();
        v.set("core", revision.core);
        self.records.insert(
            key.clone(),
            Local {
                v,
                server_known: false,
            },
        );
        self.enqueue_upsert(&key);
    }

    /// The end of a full pull: compare every local record with what the server has (the table of §6.3), send the tombstones, and for a
    /// first binding set the flag.
    fn finish_resync(&mut self) {
        let Some(resync) = self.resync.take() else {
            return;
        };
        let cause = resync.cause;
        if cause == ResyncCause::Binding {
            self.bound = true;
        }
        let keys: Vec<Key> = self.records.keys().cloned().collect();
        // Records that already have an unacked row: only they need the precise "is it covered" check (a binding of 100 000 records
        // would otherwise scan the outbox once per record).
        let with_rows: std::collections::HashSet<Key> = self
            .outbox
            .iter()
            .filter(|row| row.acked_seq.is_none() && row.env.header.kind == Kind::Upsert)
            .map(|row| row.env.header.key())
            .collect();
        for key in keys {
            let Some(local) = self.records.get(&key) else {
                continue;
            };
            let mine = local.v.snapshot();
            let server = resync.seen.get(&key);
            let facts = Facts {
                server_known: local.server_known,
                on_server: server.is_some(),
                newer_than_server: server.is_none_or(|server| newer_than_snapshot(&mine, server)),
            };
            match resync_action(cause, facts) {
                ResyncAction::Nothing => {}
                ResyncAction::Push => {
                    if !with_rows.contains(&key) || !self.covered_by_outbox(&key) {
                        self.enqueue_upsert(&key);
                    }
                }
                ResyncAction::Quarantine => {
                    if let Some(local) = self.records.remove(&key) {
                        self.drop_unacked_upserts(std::slice::from_ref(&key));
                        self.quarantine.push(Quarantined {
                            key,
                            v: local.v,
                            reason: "found old entry",
                        });
                    }
                }
            }
        }
        if matches!(
            cause,
            ResyncCause::Binding | ResyncCause::GenerationChanged | ResyncCause::StaleAndGeneration
        ) {
            self.resend_tombstones();
        }
    }

    /// Local tombstones go out as `purged` with their snapshot, parent first (§6.2 step 3): a cascade child waits for its parent's ack.
    fn resend_tombstones(&mut self) {
        let todo: Vec<(Key, Snapshot, Option<String>)> = self
            .tombstones
            .iter()
            .filter(|(key, t)| {
                !t.confirmed
                    && !self.outbox.iter().any(|row| {
                        row.env.header.kind == Kind::Purged
                            && row.env.header.key() == **key
                            && row.acked_seq.is_none()
                    })
            })
            .map(|(key, t)| (key.clone(), t.purged_of.clone(), t.cascade_of.clone()))
            .collect();
        let mut parent_ops: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        for (key, snapshot, cascade) in &todo {
            if cascade.is_none() {
                let op = self.enqueue_purged(key, snapshot.clone(), None);
                parent_ops.insert(key.1.clone(), op);
            }
        }
        for (key, snapshot, cascade) in todo {
            if let Some(parent) = cascade {
                let blocked_by = parent_ops.get(&parent).cloned();
                let op = self.enqueue_purged(&key, snapshot, blocked_by);
                parent_ops.insert(key.1.clone(), op);
            }
        }
    }

    /// An unacked row already carries every group that is newer than the server's: no second envelope is needed.
    fn covered_by_outbox(&self, key: &Key) -> bool {
        let Some(local) = self.records.get(key) else {
            return true;
        };
        self.outbox.iter().any(|row| {
            row.acked_seq.is_none()
                && row.env.header.kind == Kind::Upsert
                && row.env.header.key() == *key
                && local.v.groups().all(|(g, group)| {
                    row.env
                        .header
                        .groups
                        .get(g)
                        .is_some_and(|(h, _)| *h >= group.hlc)
                })
        })
    }
}

/// The merge a client runs, with the test-only breakages of `Switches` applied to the spike's copy of the rule.
fn merge_with_switches(local: &Versioned, incoming: &Versioned, switches: &Switches) -> Merged {
    let mut incoming = incoming.clone();
    if switches.no_device_tiebreak {
        // A tie of `wall_ms` and counter between devices is resolved by arrival order (the local copy stays).
        let mut tied = Vec::new();
        for (name, group) in incoming.groups() {
            if let Some(mine) = local.get(name) {
                if mine.hlc != group.hlc && mine.hlc.get(..19) == group.hlc.get(..19) {
                    tied.push((name.to_string(), mine.clone()));
                }
            }
        }
        for (name, mine) in tied {
            incoming.set(&name, mine);
        }
    }
    let mut merged = merge(local, &incoming);
    if switches.whole_record_lww {
        let newest = |v: &Versioned| v.groups().map(|(_, g)| g.hlc.clone()).max();
        let take_incoming = newest(&incoming) > newest(local);
        let (winner, other) = if take_incoming {
            (&incoming, local)
        } else {
            (local, &incoming)
        };
        let mut record = winner.clone();
        for (name, group) in other.groups() {
            if record.get(name).is_none() {
                record.set(name, group.clone());
            }
        }
        merged.record = record;
        merged.changed = merged.record != *local;
    }
    if switches.delete_always_wins {
        let deleted = |v: &Versioned| {
            v.get("deleted")
                .filter(|g| !g.raw.contains("null"))
                .cloned()
        };
        if let Some(group) = deleted(local).or_else(|| deleted(&incoming)) {
            merged.record.set("deleted", group);
        }
    }
    merged
}

/// A stand-in for `crypto_generichash(K_rev, ...)`: deterministic, 16 bytes. The real keyed hash is S4b's.
fn toy_hash(input: &[u8]) -> [u8; 16] {
    let mut out = [0u8; 16];
    for (i, chunk) in out.chunks_mut(8).enumerate() {
        let mut hasher = DefaultHasher::new();
        i.hash(&mut hasher);
        input.hash(&mut hasher);
        chunk.copy_from_slice(&hasher.finish().to_be_bytes());
    }
    out
}
