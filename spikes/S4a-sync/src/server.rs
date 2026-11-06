//! The in-memory server (ADR-0001 §6.1, §3.5, §A.3, §A.4). It reads only the open header of an envelope and never merges. `seq` is
//! issued in order under one lock (here: one `&mut self` call), idempotently by `op_id`, and the cursor is `(generation, seq)`.

use crate::envelope::{Entity, Envelope, Key, Kind, RETENTION_MS};
use crate::switches::Switches;
use kinige_domain::hlc::{self, SKEW_MAX_MS};
use kinige_domain::sync::{Snapshot, newer_than_snapshot};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

/// A stored envelope and when the server took it.
#[derive(Debug, Clone)]
pub struct Stored {
    pub seq: u64,
    pub at_ms: u64,
    pub env: Arc<Envelope>,
}

/// The answers to a push, per `op_id` (appendix §A.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushResponse {
    Ok(u64),
    Duplicate(u64),
    /// The same `op_id` with other bytes: a client error, sync stops.
    OpConflict,
    /// The entity is already purged.
    Purged,
    /// The server knows a group newer than the `purged_of` snapshot.
    StalePurge,
    ClockSkew,
    /// The pusher's cursor is behind a drop it missed (S4c, appendix §A.3): pull first. Only the real server answers it.
    Stale,
}

#[derive(Debug, Clone)]
pub enum PullResponse {
    Page {
        after: u64,
        items: Vec<(u64, Arc<Envelope>)>,
        /// Where the cursor goes after this page: the last `seq` of a truncated page, otherwise the head.
        next: u64,
        head: u64,
        /// The server's current drop epoch: the device sends it back with its next pull.
        epoch: u64,
    },
    /// Something that matters (a `purged`, or the `upsert`s of a purged entity) was dropped after the cursor (§6.3).
    Stale,
    /// The server was restored from a backup (§6.3). `stale`: the old cursor was also stale (both causes at once). `epoch`: the drop
    /// epoch of the new generation now; whatever dropped before it is covered by `stale`, a later drop is for the device to notice
    /// in the pages of its resync.
    GenerationChanged {
        generation: u64,
        stale: bool,
        epoch: u64,
    },
}

/// A compaction that dropped something a device cannot do without (a `purged` and the `upsert`s of the entity it purged).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Drop {
    /// The generation the drop was made in: its `seq` and `epoch` mean something only against a cursor of the same generation.
    generation: u64,
    epoch: u64,
    /// The highest `seq` dropped.
    seq: u64,
}

#[derive(Debug, Clone, Default)]
pub struct Server {
    /// The current generation: an opaque id, unique by chance (a random number made at the restore), ordered by `lineage`.
    pub generation: u64,
    /// The generations this state descends from, oldest first, the current one last. It is part of the dump: a restore appends one.
    lineage: Vec<u64>,
    next_seq: u64,
    log: BTreeMap<u64, Stored>,
    /// `op_id -> (seq, bytes id)`.
    by_op: HashMap<String, (u64, u64)>,
    /// The maximum HLC per group from the open headers of each entity: the index of `stale_purge` and of compaction.
    known: HashMap<Key, Snapshot>,
    /// Purged entities and the `seq` of the accepted `purged`.
    purged: HashMap<Key, u64>,
    /// The compactions that dropped something a device cannot do without. A cursor of the drop's generation below its `seq`, or any
    /// cursor of an older generation, has missed it, so it is stale (§6.3). Covered `upsert`s do not count: a later one says the same.
    drops: Vec<Drop>,
    /// A counter of such compactions; every page tells the device the current value. It is part of the dump and means something only
    /// within one generation (S4a review round 2): nothing has to live outside the dump.
    epoch: u64,
    /// With `no_seq_lock`: an envelope that has its `seq` but is not visible yet.
    in_flight: Option<Stored>,
    /// Every entity for which a `purged` was ever accepted (not reduced by compaction): for the property checks.
    pub purged_ever: Vec<Key>,
    pub switches: Switches,
}

impl Server {
    pub fn new(switches: Switches) -> Server {
        Server {
            generation: 1,
            lineage: vec![1],
            next_seq: 1,
            switches,
            ..Server::default()
        }
    }

    /// The highest `seq` pull can reach: the log's last, or the last one dropped if that is higher (a cursor must be able to get there).
    pub fn head(&self) -> u64 {
        let dropped = self.drops.iter().map(|d| d.seq).max().unwrap_or(0);
        self.log
            .keys()
            .next_back()
            .copied()
            .unwrap_or(0)
            .max(dropped)
    }

    pub fn log_len(&self) -> usize {
        self.log.len()
    }

    pub fn issued(&self) -> u64 {
        self.next_seq - 1
    }

    /// How many compactions have dropped a `purged` (and the entity's `upsert`s).
    pub fn drops_count(&self) -> usize {
        self.drops.len()
    }

    /// No envelope is waiting for its delayed commit (only with `no_seq_lock`).
    pub fn settled(&self) -> bool {
        self.in_flight.is_none()
    }

    /// The `seq` the server gave to an `op_id`, if it did.
    pub fn seq_of(&self, op_id: &str) -> Option<u64> {
        self.by_op.get(op_id).map(|(seq, _)| *seq)
    }

    pub fn push(&mut self, env: &Arc<Envelope>, now_ms: u64) -> PushResponse {
        self.push_inner(env, now_ms)
    }

    fn push_inner(&mut self, env: &Arc<Envelope>, now_ms: u64) -> PushResponse {
        let header = &env.header;
        // 3. op_id deduplication, before every purge rule, so that a repeat of an accepted envelope does not get `purged`.
        if !self.switches.no_dedup {
            if let Some(&(seq, bytes)) = self.by_op.get(&header.op_id) {
                return if bytes == env.bytes_id() {
                    PushResponse::Duplicate(seq)
                } else {
                    PushResponse::OpConflict
                };
            }
        }
        // 5. clock_skew: `wall_ms` in any HLC of the header beyond `server_now + SKEW_MAX`.
        let too_far = header
            .groups
            .values()
            .map(|(h, _)| h)
            .chain(header.purged_of.iter().flat_map(|s| s.values()))
            .any(|h| hlc::parse(h).is_ok_and(|(c, _)| c.l > now_ms.saturating_add(SKEW_MAX_MS)));
        if too_far {
            return PushResponse::ClockSkew;
        }
        let key = header.key();
        // 6. The entity is already purged: any kind except a repeated `purged`, which is a no-op ok.
        if let Some(&seq) = self.purged.get(&key) {
            return if header.kind == Kind::Purged {
                PushResponse::Ok(seq)
            } else {
                PushResponse::Purged
            };
        }
        // 7. stale_purge: each group of the snapshot against the maximum HLC of that group from the entity's headers.
        if header.kind == Kind::Purged {
            let snapshot = header.purged_of.clone().unwrap_or_default();
            if self
                .known
                .get(&key)
                .is_some_and(|known| newer_than_snapshot(known, &snapshot))
            {
                return PushResponse::StalePurge;
            }
        }
        // 8. seq under the account lock.
        let seq = self.next_seq;
        self.next_seq += 1;
        let stored = Stored {
            seq,
            at_ms: now_ms,
            env: Arc::clone(env),
        };
        self.by_op
            .insert(header.op_id.clone(), (seq, env.bytes_id()));
        match header.kind {
            Kind::Upsert => {
                let known = self.known.entry(key).or_default();
                for (group, (h, _)) in &header.groups {
                    let slot = known.entry(group.clone()).or_default();
                    if *h > *slot {
                        slot.clone_from(h);
                    }
                }
            }
            Kind::Purged => {
                self.purged.insert(key.clone(), seq);
                self.purged_ever.push(key);
            }
        }
        if self.switches.no_seq_lock && self.in_flight.is_none() {
            // Without a common lock the number is taken at the start and the commit comes later: this envelope is visible only
            // after the next pull, while a push that follows it is visible at once. A pull in between moves its cursor past this `seq`.
            self.in_flight = Some(stored);
        } else {
            self.log.insert(seq, stored);
        }
        PushResponse::Ok(seq)
    }

    fn commit_in_flight(&mut self) {
        if let Some(stored) = self.in_flight.take() {
            self.log.insert(stored.seq, stored);
        }
    }

    /// `known_epoch` is the `epoch` the device saw in its last page.
    pub fn pull(
        &mut self,
        generation: u64,
        after: u64,
        known_epoch: u64,
        limit: usize,
    ) -> PullResponse {
        let response = self.pull_inner(generation, after, known_epoch, limit);
        self.commit_in_flight();
        response
    }

    /// Whether the device with cursor `(generation, after)` and `known_epoch` has missed a drop (§6.3). A pull from 0 is a full resync
    /// itself: it reads what is left. Otherwise, by where the drop's generation stands against the cursor's in the lineage:
    /// - an older generation: the cursor was made after the drop, it cannot have missed it;
    /// - the same generation: it missed it if the drop is above the cursor and newer than the last page the device applied;
    /// - a newer generation: the cursor cannot have seen anything made in it, whatever the `seq`s say (a restored server issues `seq`
    ///   again from the backup's head).
    ///
    /// A generation that is not in the lineage was made after the backup the lineage ends in, so it stands between the backup's and the
    /// current one.
    fn missed(&self, generation: u64, after: u64, known_epoch: u64) -> bool {
        if after == 0 {
            return false;
        }
        if self.switches.drop_seq_across_generations {
            return self
                .drops
                .iter()
                .any(|d| d.epoch > known_epoch && d.seq > after);
        }
        let place = |g: u64| self.lineage.iter().position(|x| *x == g);
        // Positions are doubled and shifted by two so that an unknown generation can sit between two known ones, and below the only
        // one when the lineage has a single generation (then every drop of the current generation is missed).
        let unknown = if self.lineage.len() < 2 {
            1
        } else {
            2 * (self.lineage.len() - 2) + 3
        };
        let cursor = place(generation).map_or(unknown, |p| 2 * p + 2);
        self.drops.iter().any(|d| {
            let made = place(d.generation).map_or(0, |p| 2 * p + 2);
            made > cursor || (made == cursor && d.epoch > known_epoch && d.seq > after)
        })
    }

    fn pull_inner(
        &self,
        generation: u64,
        after: u64,
        known_epoch: u64,
        limit: usize,
    ) -> PullResponse {
        let missed = self.missed(generation, after, known_epoch);
        if generation != self.generation {
            // Both causes at once. The device does not guess from its clock; the drops are held against its cursor by generation.
            return PullResponse::GenerationChanged {
                generation: self.generation,
                stale: missed && !self.switches.no_stale_check,
                epoch: self.epoch,
            };
        }
        if missed && !self.switches.no_stale_check {
            return PullResponse::Stale;
        }
        let items: Vec<(u64, Arc<Envelope>)> = self
            .log
            .range(after + 1..)
            .take(limit)
            .map(|(seq, stored)| (*seq, Arc::clone(&stored.env)))
            .collect();
        let truncated = self.log.range(after + 1..).nth(limit).is_some();
        let next = match items.last() {
            Some((seq, _)) if truncated => *seq,
            _ => self.head().max(after),
        };
        PullResponse::Page {
            after,
            items,
            next,
            head: self.head(),
            epoch: self.epoch,
        }
    }

    /// Compaction (§6.1, §10.2): only envelopes older than the retention period. A `purged` goes after retention together with the
    /// entity's `upsert`s (otherwise a surviving `upsert` would resurrect it for a new device); an `upsert` goes when each of its
    /// groups is covered by a later, or greater, one.
    pub fn compact(&mut self, now_ms: u64) {
        let old = |s: &Stored| now_ms.saturating_sub(s.at_ms) > RETENTION_MS;
        let expired: Vec<Key> = self
            .log
            .values()
            .filter(|s| s.env.header.kind == Kind::Purged && old(s))
            .map(|s| s.env.header.key())
            .collect();
        let mut dropped_max = 0;
        for key in &expired {
            let highest = self
                .log
                .values()
                .filter(|s| s.env.header.key() == *key)
                .map(|s| s.seq)
                .max()
                .unwrap_or(0);
            dropped_max = dropped_max.max(highest);
            self.log.retain(|_, s| s.env.header.key() != *key);
            self.purged.remove(key);
            self.known.remove(key);
        }
        if dropped_max > 0 {
            self.epoch += 1;
            self.drops.push(Drop {
                generation: self.generation,
                epoch: self.epoch,
                seq: dropped_max,
            });
        }
        let mut best: HashMap<(Key, String), (String, u64)> = HashMap::new();
        for stored in self
            .log
            .values()
            .filter(|s| s.env.header.kind == Kind::Upsert)
        {
            for (group, (h, _)) in &stored.env.header.groups {
                let slot = best
                    .entry((stored.env.header.key(), group.clone()))
                    .or_insert((h.clone(), stored.seq));
                if (h.as_str(), stored.seq) > (slot.0.as_str(), slot.1) {
                    *slot = (h.clone(), stored.seq);
                }
            }
        }
        let covered = |s: &Stored| {
            s.env.header.groups.iter().all(|(group, (h, _))| {
                best.get(&(s.env.header.key(), group.clone()))
                    .is_some_and(|(bh, bseq)| (bh.as_str(), *bseq) > (h.as_str(), s.seq))
            })
        };
        let drop_seqs: Vec<u64> = self
            .log
            .values()
            .filter(|s| s.env.header.kind == Kind::Upsert && old(s) && covered(s))
            .map(|s| s.seq)
            .collect();
        for seq in drop_seqs {
            self.log.remove(&seq);
        }
    }

    /// A copy of everything, for a restore from a backup.
    pub fn backup(&self) -> Server {
        self.clone()
    }

    /// Restoring from a backup (§6.3), into a clean installation: the new state is the dump and a fresh `generation`, nothing else.
    /// Whatever was acked after the backup is gone. The generation id is `nonce` (a random number in a real server; the world supplies
    /// one that cannot repeat), and it is appended to the dump's lineage. The one thing kept from the old state is `purged_ever`, which
    /// is the simulation's own record for the property checks, not server state.
    pub fn restore(&mut self, backup: &Server, nonce: u64) {
        let purged_ever = std::mem::take(&mut self.purged_ever);
        *self = Server::from_dump(backup, nonce);
        self.purged_ever = purged_ever;
    }

    fn from_dump(dump: &Server, nonce: u64) -> Server {
        let mut state = dump.clone();
        let mut generation = nonce;
        while state.lineage.contains(&generation) {
            generation = generation.wrapping_add(1);
        }
        state.generation = generation;
        state.lineage.push(generation);
        state.in_flight = None;
        state
    }

    /// The generations of this state, oldest first (for the tests).
    pub fn lineage(&self) -> &[u64] {
        &self.lineage
    }

    /// The epoch counter (for the tests).
    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    /// Entities whose `purged` the server accepted and still remembers.
    pub fn is_purged(&self, key: &Key) -> bool {
        self.purged.contains_key(key)
    }

    /// Every envelope in the log, oldest first (for the idempotence replay).
    pub fn all_envelopes(&self) -> Vec<Arc<Envelope>> {
        self.log.values().map(|s| Arc::clone(&s.env)).collect()
    }

    pub fn entities(&self, entity: Entity) -> usize {
        self.known.keys().filter(|(e, _)| *e == entity).count()
    }
}
