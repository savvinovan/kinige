//! Compaction (ADR-0001 §6.1, §10.2), the rule of `spikes/S4a-sync/src/server.rs::compact`: only envelopes older than the retention
//! period go. A `purged` goes after retention together with all its entity's envelopes (otherwise a surviving `upsert` would
//! resurrect the record for a new device), and that is a **drop**. An `upsert` goes when every one of its groups is covered by a later,
//! or greater, one. The plan is computed from the open metadata of the log; the store applies it under the account lock.

use kinige_crypto::header::Kind;
use std::collections::{BTreeMap, HashMap, HashSet};

pub const DAY_MS: u64 = 24 * 60 * 60 * 1000;
/// The tombstone retention of §3.5 and §6.3: the minimum age before a compaction may drop a `purged`.
pub const RETENTION_MS: u64 = 180 * DAY_MS;

/// What the server knows of one stored envelope without reading it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Meta {
    pub seq: u64,
    pub at_ms: u64,
    pub entity: String,
    pub id: String,
    pub kind: Kind,
    /// `group -> HLC`; empty for a `purged`.
    pub groups: BTreeMap<String, String>,
}

impl Meta {
    fn key(&self) -> (&str, &str) {
        (&self.entity, &self.id)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Plan {
    /// Entities whose `purged` expired: every envelope of theirs goes, and so does the server's memory of the purge.
    pub forgotten: Vec<(String, String)>,
    /// The highest `seq` removed together with a `purged`; 0 when no `purged` expired. A non-zero value is a drop (new epoch).
    pub dropped_max: u64,
    /// Every `seq` to delete from the log, those of `forgotten` entities included, ascending.
    pub remove: Vec<u64>,
}

/// `entries` in any order; `now_ms` is the server's clock.
pub fn plan(entries: &[Meta], now_ms: u64) -> Plan {
    let old = |meta: &Meta| now_ms.saturating_sub(meta.at_ms) > RETENTION_MS;
    let expired: HashSet<(&str, &str)> = entries
        .iter()
        .filter(|meta| meta.kind == Kind::Purged && old(meta))
        .map(Meta::key)
        .collect();
    let mut remove: Vec<u64> = Vec::new();
    let mut dropped_max = 0;
    for meta in entries.iter().filter(|meta| expired.contains(&meta.key())) {
        remove.push(meta.seq);
        dropped_max = dropped_max.max(meta.seq);
    }
    let alive = || {
        entries
            .iter()
            .filter(|meta| meta.kind == Kind::Upsert && !expired.contains(&meta.key()))
    };
    // The newest version of each group of each entity: by HLC, ties by `seq`.
    let mut best: HashMap<(&str, &str, &str), (&str, u64)> = HashMap::new();
    for meta in alive() {
        for (group, hlc) in &meta.groups {
            let slot = best
                .entry((&meta.entity, &meta.id, group))
                .or_insert((hlc, meta.seq));
            if (hlc.as_str(), meta.seq) > (slot.0, slot.1) {
                *slot = (hlc, meta.seq);
            }
        }
    }
    for meta in alive().filter(|meta| old(meta)) {
        let covered = meta.groups.iter().all(|(group, hlc)| {
            best.get(&(meta.entity.as_str(), meta.id.as_str(), group.as_str()))
                .is_some_and(|(best_hlc, best_seq)| {
                    (*best_hlc, *best_seq) > (hlc.as_str(), meta.seq)
                })
        });
        if covered {
            remove.push(meta.seq);
        }
    }
    remove.sort_unstable();
    let mut forgotten: Vec<(String, String)> = expired
        .into_iter()
        .map(|(entity, id)| (entity.to_string(), id.to_string()))
        .collect();
    forgotten.sort();
    Plan {
        forgotten,
        dropped_max,
        remove,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_800_000_000_000;
    const OLD: u64 = NOW - RETENTION_MS - 1;

    fn hlc(wall: u64) -> String {
        format!("{wall:013}-00000-66666666-6666-4666-8666-666666666666")
    }

    fn upsert(seq: u64, at_ms: u64, id: &str, groups: &[(&str, u64)]) -> Meta {
        Meta {
            seq,
            at_ms,
            entity: "message".to_string(),
            id: id.to_string(),
            kind: Kind::Upsert,
            groups: groups
                .iter()
                .map(|(group, wall)| (group.to_string(), hlc(*wall)))
                .collect(),
        }
    }

    fn purged(seq: u64, at_ms: u64, id: &str) -> Meta {
        Meta {
            kind: Kind::Purged,
            groups: BTreeMap::new(),
            ..upsert(seq, at_ms, id, &[])
        }
    }

    #[test]
    fn nothing_young_is_touched() {
        let entries = [
            upsert(1, NOW - 1000, "a", &[("body", 1)]),
            upsert(2, NOW - 500, "a", &[("body", 2)]),
            purged(3, NOW - 100, "b"),
        ];
        assert_eq!(plan(&entries, NOW), Plan::default());
    }

    #[test]
    fn an_old_purged_goes_with_every_envelope_of_its_entity_and_is_a_drop() {
        let entries = [
            upsert(1, OLD, "a", &[("body", 1)]),
            upsert(2, NOW - 10, "a", &[("flags", 5)]),
            purged(3, OLD, "a"),
            upsert(4, OLD, "other", &[("body", 1)]),
        ];
        let plan = plan(&entries, NOW);
        assert_eq!(
            plan.forgotten,
            vec![("message".to_string(), "a".to_string())]
        );
        assert_eq!(plan.dropped_max, 3);
        assert_eq!(
            plan.remove,
            vec![1, 2, 3],
            "the young upsert of a purged entity goes too"
        );
    }

    #[test]
    fn an_old_upsert_goes_only_when_every_group_is_covered_by_a_later_one() {
        let entries = [
            upsert(1, OLD, "a", &[("body", 1), ("flags", 1)]),
            upsert(2, OLD, "a", &[("body", 2)]),
            upsert(3, OLD, "a", &[("flags", 3), ("body", 2)]),
        ];
        let plan = plan(&entries, NOW);
        // 1 is covered by 3 (flags) and by 2 and 3 (body); 2 is covered in body by 3 (the same HLC, a later seq); 3 is the newest.
        assert_eq!(plan.remove, vec![1, 2]);
        assert_eq!(plan.dropped_max, 0, "covered upserts are not a drop");
        assert!(plan.forgotten.is_empty());
    }

    #[test]
    fn an_upsert_with_a_group_nobody_else_has_stays() {
        let entries = [
            upsert(1, OLD, "a", &[("body", 1), ("extra", 1)]),
            upsert(2, OLD, "a", &[("body", 2)]),
        ];
        assert_eq!(plan(&entries, NOW).remove, Vec::<u64>::new());
    }

    #[test]
    fn a_young_covering_envelope_covers_an_old_one() {
        let entries = [
            upsert(1, OLD, "a", &[("body", 1)]),
            upsert(2, NOW, "a", &[("body", 2)]),
        ];
        assert_eq!(plan(&entries, NOW).remove, vec![1]);
    }

    #[test]
    fn age_is_strictly_beyond_the_retention_period() {
        let at_limit = [purged(1, NOW - RETENTION_MS, "a")];
        assert_eq!(plan(&at_limit, NOW), Plan::default());
        let beyond = [purged(1, NOW - RETENTION_MS - 1, "a")];
        assert_eq!(plan(&beyond, NOW).dropped_max, 1);
    }
}
