//! Purge and tombstone verdicts (ADR-0001 §3.5, §3.6). `purged_of` is a snapshot of the HLCs of **all** groups of the record as
//! the purging device saw them; a group missing from the snapshot is newer than it. One predicate, [`newer_than_snapshot`], serves the
//! server's `stale_purge` check, the device's own ack, a pulled `purged`, a `purged` response and an envelope against a tombstone.

use super::record::Snapshot;

/// Whether any group in `known` has an HLC greater than the one in `snapshot`, or is missing from it.
///
/// `known` is whatever the asking party knows: the maximum HLC per group from the entity's open headers (the server), the groups of
/// the local record (a device), or the groups of an incoming envelope (the tombstone check). A group only in `snapshot` is not newer.
pub fn newer_than_snapshot(known: &Snapshot, snapshot: &Snapshot) -> bool {
    known
        .iter()
        .any(|(group, hlc)| snapshot.get(group).is_none_or(|purged| hlc > purged))
}

/// What a device does with a record when `purged(X)` is confirmed or arrives (§3.5): the ack of its own purge, or a pull.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PurgeVerdict {
    /// No group is newer than the snapshot: the record and its children are deleted and tombstones are written.
    DeleteSilently,
    /// The user managed to restore or edit it: the record together with its children goes to quarantine (the server already
    /// deleted it) and a tombstone is written. A copy without edits loses nothing; one with edits must not vanish silently.
    Quarantine,
}

pub fn on_purge(record: &Snapshot, purged_of: &Snapshot) -> PurgeVerdict {
    if newer_than_snapshot(record, purged_of) {
        PurgeVerdict::Quarantine
    } else {
        PurgeVerdict::DeleteSilently
    }
}

/// What a device does with an envelope for an id that has a tombstone (§3.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TombstoneVerdict {
    /// An HLC of any group is greater than the snapshot's: the purge was stale. The envelope is applied and the tombstone is lifted
    /// together with those of the cascade's children. This is the client's own protection against a server that reordered its log.
    Lift,
    /// The envelope is dropped (a cascade child's groups are merged into the held snapshot instead).
    Drop,
}

pub fn envelope_vs_tombstone(envelope: &Snapshot, purged_of: &Snapshot) -> TombstoneVerdict {
    if newer_than_snapshot(envelope, purged_of) {
        TombstoneVerdict::Lift
    } else {
        TombstoneVerdict::Drop
    }
}

/// Whether a message is a child of the chat being purged (§3.6): its `placement` HLC is **less** than the HLC of the chat's deletion
/// (`deleted` in the chat's `purged_of`). A message with a greater `placement` was moved into the chat after the deletion; it is live
/// and neither goes to quarantine nor is deleted on the ack. A chat snapshot without `deleted` has no children.
pub fn is_chat_child(placement_hlc: &str, chat_purged_of: &Snapshot) -> bool {
    chat_purged_of
        .get("deleted")
        .is_some_and(|deleted| placement_hlc < deleted.as_str())
}
