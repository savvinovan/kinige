//! Merging a record (ADR-0001 §3.1, §3.3): for each group in the union of groups the larger HLC wins; a group missing on one side
//! is taken from the other. The result is an element-wise maximum, so it is commutative, associative and idempotent: the state of
//! records converges under any order, duplicates and retries.

use super::record::{Group, Versioned};
use std::cmp::Ordering;

/// Two different versions of one group met: which one stayed and which one lost. The caller decides what the loser becomes
/// ([`super::conflict`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contest {
    pub group: String,
    pub winner: Group,
    pub loser: Group,
    /// The loser is the version this device already had (the incoming one won).
    pub loser_is_local: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Merged {
    pub record: Versioned,
    /// One entry per group where the two sides differed.
    pub contests: Vec<Contest>,
    /// Groups where the HLC was equal and the bytes were not: an integrity error, only possible with two live copies of one
    /// `device_id` (§3.3). The larger bytes won; the caller writes the event to the app log without content.
    pub equal_hlc_different_bytes: Vec<String>,
    /// The merge changed the local record.
    pub changed: bool,
}

/// The result of merging `incoming` into `local`.
pub fn merge(local: &Versioned, incoming: &Versioned) -> Merged {
    let mut record = Versioned::new();
    let mut contests = Vec::new();
    let mut equal_hlc_different_bytes = Vec::new();

    for (name, mine) in local.groups() {
        let Some(theirs) = incoming.get(name) else {
            record.set(name, mine.clone());
            continue;
        };
        let (winner, loser, loser_is_local) = match mine.hlc.cmp(&theirs.hlc) {
            Ordering::Greater => (mine, theirs, false),
            Ordering::Less => (theirs, mine, true),
            Ordering::Equal if mine == theirs => {
                record.set(name, mine.clone());
                continue;
            }
            Ordering::Equal => {
                // §3.3: the lexicographically larger bytes win (then `sv`, so that the choice never depends on the side).
                equal_hlc_different_bytes.push(name.to_string());
                if (&mine.raw, mine.sv) >= (&theirs.raw, theirs.sv) {
                    (mine, theirs, false)
                } else {
                    (theirs, mine, true)
                }
            }
        };
        record.set(name, winner.clone());
        contests.push(Contest {
            group: name.to_string(),
            winner: winner.clone(),
            loser: loser.clone(),
            loser_is_local,
        });
    }
    for (name, theirs) in incoming.groups() {
        if local.get(name).is_none() {
            record.set(name, theirs.clone());
        }
    }

    let changed = record != *local;
    Merged {
        record,
        contests,
        equal_hlc_different_bytes,
        changed,
    }
}
