//! The decision table of the first binding and of a full resync (ADR-0001 §6.2, §6.3, decision 10): for a local record, after the
//! full pull from `seq = 0` has been merged, push it, quarantine it, or leave it. Quarantine is decided by `server_known` and the
//! cause, not by the age of the HLC.

/// Why everything is pulled from `seq = 0` again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResyncCause {
    /// The first binding of a database to an account (§6.2). Nothing was ever sent: `server_known = 0` for every record.
    Binding,
    /// The server answered "cursor is stale": since the device last heard from it, a compaction dropped a `purged` (or the
    /// envelopes of the entity it purged) above the cursor (§6.3, S4a), so a purge the device never saw may be gone.
    StaleCursor,
    /// The `generation` changed: the server was restored from a backup and lost whatever was acked after it.
    GenerationChanged,
    /// Both of the last two at once. The server cannot say whether the old cursor was stale (its drops are those of the backup, in
    /// another numbering), so the device decides by its own clock: the last page it applied is older than the retention period
    /// (§6.3). The "stale" column applies: quarantine is safer.
    StaleAndGeneration,
}

/// What a device knows about one local record after the full pull.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Facts {
    /// The server knew this record before (an ack or a pull), as opposed to a record that never left the device.
    pub server_known: bool,
    /// The record is in the pulled log. A record whose envelopes arrived in an unknown `format_version` (`pending_unknown`)
    /// counts as on the server: it must not go to quarantine.
    pub on_server: bool,
    /// At least one group is newer than the server's version of it.
    pub newer_than_server: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResyncAction {
    /// The server has it and is not behind.
    Nothing,
    /// An envelope is created, regardless of the age of the HLC.
    Push,
    /// "Found old entries": the server knew it and does not have it, so it was deleted and the tombstone has expired. Pushing it
    /// back would resurrect a deleted record.
    Quarantine,
}

pub fn resync_action(cause: ResyncCause, facts: Facts) -> ResyncAction {
    if facts.on_server {
        return if facts.newer_than_server {
            ResyncAction::Push
        } else {
            ResyncAction::Nothing
        };
    }
    if !facts.server_known {
        return ResyncAction::Push;
    }
    match cause {
        // The server lost it in the restore.
        ResyncCause::GenerationChanged => ResyncAction::Push,
        // A `Binding` record with `server_known = 1` cannot exist; the safe side is the same as for a stale cursor.
        ResyncCause::StaleCursor | ResyncCause::StaleAndGeneration | ResyncCause::Binding => {
            ResyncAction::Quarantine
        }
    }
}
