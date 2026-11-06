//! The server's side of a push, in the order of ADR-0001 appendix §A.4 (server): 3 `op_id` deduplication, 4 `too_old`, 4a `stale`
//! (S4c, 2026-11-06), 5 `clock_skew`, 6 purged entity, 7 `stale_purge`; step 8, assigning `seq`, is the store's, under the lock. Steps 1
//! and 2 (lengths and session) are done before this function is reached.

use kinige_crypto::header::{Header, Kind};
use kinige_domain::hlc::{self, SKEW_MAX_MS};
use kinige_domain::sync::{Snapshot, newer_than_snapshot};
use libsodium_rs::utils;

/// What the server already has for this `op_id`: its `seq` and the hash of the bytes it took.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpRecord {
    pub seq: u64,
    pub bytes_hash: [u8; 32],
}

/// An entity whose `purged` the server accepted: where, and the snapshot it carried.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PurgedRecord {
    pub seq: u64,
    pub purged_of: Snapshot,
}

/// Everything the decision needs, read under the account lock.
#[derive(Debug)]
pub struct Facts<'a> {
    pub header: &'a Header,
    pub bytes_hash: [u8; 32],
    pub op: Option<&'a OpRecord>,
    /// The request's protocol version is at least the server's minimum.
    pub protocol_ok: bool,
    /// The pusher's cursor is behind a drop it missed, or in another generation (appendix §A.3 `stale`).
    pub cursor_stale: bool,
    pub now_ms: u64,
    pub purged: Option<&'a PurgedRecord>,
    /// The maximum HLC per group among the entity's accepted headers.
    pub known: &'a Snapshot,
}

/// The answer for one `op_id` (appendix §A.3, with `Stale` added by S4c).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// Accepted, or a repeated `purged`: then `purged_of` is the snapshot of the `purged` the server accepted (ADR-0017).
    Ok {
        seq: u64,
        purged_of: Option<Snapshot>,
    },
    Duplicate {
        seq: u64,
    },
    OpConflict,
    Purged,
    StalePurge,
    ClockSkew,
    TooOld,
    Stale,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Steps 3-7 passed: the store assigns `seq` and writes.
    Accept,
    Answer(Answer),
}

pub fn decide(facts: &Facts<'_>) -> Verdict {
    let header = facts.header;
    // 3. Before every purge rule, so that a repeat of an accepted envelope does not get `purged`.
    if let Some(op) = facts.op {
        let same = utils::memcmp(&op.bytes_hash, &facts.bytes_hash);
        return Verdict::Answer(if same {
            Answer::Duplicate { seq: op.seq }
        } else {
            Answer::OpConflict
        });
    }
    // 4. Protocol version below the minimum.
    if !facts.protocol_ok {
        return Verdict::Answer(Answer::TooOld);
    }
    // 4a. The cursor is behind a drop it missed: pull first, so a dropped `purged` cannot be pushed back over (ADR-0017 finding 7).
    if facts.cursor_stale {
        return Verdict::Answer(Answer::Stale);
    }
    // 5. `wall_ms` of any HLC of the header beyond `server_now + SKEW_MAX`.
    if beyond_skew(header, facts.now_ms) {
        return Verdict::Answer(Answer::ClockSkew);
    }
    // 6. The entity is already purged: anything but a repeated `purged` gets `purged`; a repeat is a no-op `ok` that carries the
    // snapshot the server accepted.
    if let Some(purged) = facts.purged {
        return Verdict::Answer(if header.kind == Kind::Purged {
            Answer::Ok {
                seq: purged.seq,
                purged_of: Some(purged.purged_of.clone()),
            }
        } else {
            Answer::Purged
        });
    }
    // 7. `stale_purge`: a group of the entity known with an HLC above the snapshot's, or missing from it.
    if header.kind == Kind::Purged {
        let empty = Snapshot::new();
        let snapshot = header.purged_of.as_ref().unwrap_or(&empty);
        if newer_than_snapshot(facts.known, snapshot) {
            return Verdict::Answer(Answer::StalePurge);
        }
    }
    Verdict::Accept
}

/// Whether any HLC in the header (a group's, or a `purged_of` entry) has `wall_ms` beyond `now + SKEW_MAX`. An HLC that does not parse
/// cannot be here (`Header::parse` refused it); it counts as beyond so that a bug fails closed.
pub fn beyond_skew(header: &Header, now_ms: u64) -> bool {
    let limit = now_ms.saturating_add(SKEW_MAX_MS);
    let stamps = header.groups.values().map(|group| &group.h).chain(
        header
            .purged_of
            .iter()
            .flat_map(|snapshot| snapshot.values()),
    );
    stamps
        .into_iter()
        .any(|stamp| hlc::parse(stamp).map_or(true, |(clock, _)| clock.l > limit))
}

#[cfg(test)]
mod tests {
    use super::*;
    use kinige_crypto::header::GroupStamp;
    use std::collections::BTreeMap;

    const DEVICE: &str = "66666666-6666-4666-8666-666666666666";
    const ID: &str = "33333333-3333-4333-8333-333333333333";
    const OP: &str = "55555555-5555-4555-8555-555555555555";
    const NOW: u64 = 1_762_300_000_000;

    fn hlc_at(wall: u64) -> String {
        format!("{wall:013}-00000-{DEVICE}")
    }

    fn upsert(wall: u64) -> Header {
        Header {
            entity: "message".to_string(),
            id: ID.to_string(),
            op: OP.to_string(),
            kind: Kind::Upsert,
            groups: BTreeMap::from([(
                "body".to_string(),
                GroupStamp {
                    h: hlc_at(wall),
                    sv: 1,
                },
            )]),
            purged_of: None,
            blob: None,
        }
    }

    fn purged(snapshot: &[(&str, u64)]) -> Header {
        Header {
            kind: Kind::Purged,
            groups: BTreeMap::new(),
            purged_of: Some(
                snapshot
                    .iter()
                    .map(|(group, wall)| (group.to_string(), hlc_at(*wall)))
                    .collect(),
            ),
            ..upsert(NOW)
        }
    }

    fn facts<'a>(header: &'a Header, known: &'a Snapshot) -> Facts<'a> {
        Facts {
            header,
            bytes_hash: [1; 32],
            op: None,
            protocol_ok: true,
            cursor_stale: false,
            now_ms: NOW,
            purged: None,
            known,
        }
    }

    #[test]
    fn a_clean_push_is_accepted() {
        let header = upsert(NOW);
        let known = Snapshot::new();
        assert_eq!(decide(&facts(&header, &known)), Verdict::Accept);
    }

    #[test]
    fn a_repeat_is_a_duplicate_or_an_op_conflict_before_every_other_rule() {
        let header = upsert(NOW + 2 * SKEW_MAX_MS);
        let known = Snapshot::new();
        let record = OpRecord {
            seq: 7,
            bytes_hash: [1; 32],
        };
        let mut f = facts(&header, &known);
        f.op = Some(&record);
        f.protocol_ok = false;
        f.cursor_stale = true;
        assert_eq!(
            decide(&f),
            Verdict::Answer(Answer::Duplicate { seq: 7 }),
            "same bytes: a duplicate even from an old client, a stale cursor and a skewed clock"
        );
        f.bytes_hash = [2; 32];
        assert_eq!(decide(&f), Verdict::Answer(Answer::OpConflict));
    }

    #[test]
    fn the_checks_come_in_the_order_of_the_appendix() {
        let header = upsert(NOW + SKEW_MAX_MS + 1);
        let known = Snapshot::new();
        let accepted = PurgedRecord {
            seq: 3,
            purged_of: Snapshot::new(),
        };
        let mut f = facts(&header, &known);
        f.purged = Some(&accepted);
        f.protocol_ok = false;
        f.cursor_stale = true;
        assert_eq!(decide(&f), Verdict::Answer(Answer::TooOld));
        f.protocol_ok = true;
        assert_eq!(decide(&f), Verdict::Answer(Answer::Stale));
        f.cursor_stale = false;
        assert_eq!(decide(&f), Verdict::Answer(Answer::ClockSkew));
        let fine = upsert(NOW);
        f.header = &fine;
        assert_eq!(decide(&f), Verdict::Answer(Answer::Purged));
    }

    #[test]
    fn the_skew_limit_is_exclusive_and_covers_a_purged_snapshot() {
        let known = Snapshot::new();
        let at_limit = upsert(NOW + SKEW_MAX_MS);
        assert_eq!(decide(&facts(&at_limit, &known)), Verdict::Accept);
        let over = upsert(NOW + SKEW_MAX_MS + 1);
        assert_eq!(
            decide(&facts(&over, &known)),
            Verdict::Answer(Answer::ClockSkew)
        );
        let snapshot_over = purged(&[("body", NOW + SKEW_MAX_MS + 1)]);
        assert_eq!(
            decide(&facts(&snapshot_over, &known)),
            Verdict::Answer(Answer::ClockSkew)
        );
    }

    #[test]
    fn a_repeated_purged_is_ok_with_the_accepted_snapshot_and_other_kinds_are_purged() {
        let known = Snapshot::new();
        let accepted = PurgedRecord {
            seq: 9,
            purged_of: Snapshot::from([("body".to_string(), hlc_at(NOW - 5))]),
        };
        let again = purged(&[("body", NOW - 1)]);
        let mut f = facts(&again, &known);
        f.purged = Some(&accepted);
        assert_eq!(
            decide(&f),
            Verdict::Answer(Answer::Ok {
                seq: 9,
                purged_of: Some(accepted.purged_of.clone())
            }),
            "the answer carries the accepted snapshot, not the repeat's own"
        );
        let edit = upsert(NOW);
        f.header = &edit;
        assert_eq!(decide(&f), Verdict::Answer(Answer::Purged));
    }

    #[test]
    fn stale_purge_when_a_known_group_is_newer_or_missing_from_the_snapshot() {
        let header = purged(&[("body", NOW - 10)]);
        let newer = Snapshot::from([("body".to_string(), hlc_at(NOW - 9))]);
        assert_eq!(
            decide(&facts(&header, &newer)),
            Verdict::Answer(Answer::StalePurge)
        );
        let missing = Snapshot::from([("flags".to_string(), hlc_at(NOW - 20))]);
        assert_eq!(
            decide(&facts(&header, &missing)),
            Verdict::Answer(Answer::StalePurge),
            "a group missing from the snapshot counts as newer"
        );
        let equal = Snapshot::from([("body".to_string(), hlc_at(NOW - 10))]);
        assert_eq!(decide(&facts(&header, &equal)), Verdict::Accept);
    }
}
