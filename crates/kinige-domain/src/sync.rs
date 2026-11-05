//! The sync rules of ADR-0001 that need no IO (spike S4a, KINIG-94): merging a record, which losing version becomes a `conflict`
//! revision, the purge and tombstone verdicts, the binding and resync decision table, and what the feed shows while a parent is
//! missing. Slice 11 calls this code; the spike `spikes/S4a-sync` runs it under hostile delivery.
//!
//! Everything works on the formats the app already writes: a record is `groups_raw` + `hlc_json` ([`Versioned::from_columns`]),
//! an HLC is the strict 56-character stamp of [`crate::hlc`], group bytes are canonical JSON ([`crate::jcs`]) and are carried
//! verbatim. A group this client does not know, or one with an `sv` from the future, is just another group (ADR-0001 §4).

pub mod conflict;
pub mod lineage;
pub mod merge;
pub mod purge;
pub mod record;
pub mod resync;
pub mod visibility;

use crate::hlc::HlcError;
use std::fmt;

pub use conflict::{ConflictRevision, Lineage, conflict_revision, needs_conflict_revision};
pub use lineage::{MAX_VV, next_vv, with_vv};
pub use merge::{Contest, Merged, merge};
pub use purge::{
    PurgeVerdict, TombstoneVerdict, envelope_vs_tombstone, is_chat_child, newer_than_snapshot,
    on_purge,
};
pub use record::{Group, Snapshot, Versioned};
pub use resync::{Facts, ResyncAction, ResyncCause, resync_action};
pub use visibility::{ChatState, InboxMark, Slot, message_slot};

/// A record or envelope part that cannot be used. Foreign data never panics: it is an error the caller turns into `malformed`
/// quarantine (ADR-0001 §5.4) or into a skipped record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncError {
    /// `groups_raw` / `hlc_json` / a group is not what the format says.
    Corrupt(String),
    /// A group's HLC is not a strict stamp (width, digits, device id).
    Hlc(HlcError),
    /// A group has an `sv` above the one this client supports: it cannot be written into a revision (ADR-0001 §4 item 4).
    UnsupportedVersion,
}

impl fmt::Display for SyncError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SyncError::UnsupportedVersion => f.write_str("group version is unsupported"),
            SyncError::Corrupt(why) => write!(f, "record is corrupt: {why}"),
            SyncError::Hlc(error) => write!(f, "bad HLC: {error}"),
        }
    }
}

impl std::error::Error for SyncError {}

impl From<HlcError> for SyncError {
    fn from(error: HlcError) -> Self {
        SyncError::Hlc(error)
    }
}

pub(crate) fn corrupt(why: impl ToString) -> SyncError {
    SyncError::Corrupt(why.to_string())
}

#[cfg(test)]
mod lineage_tests;
#[cfg(test)]
mod rules_tests;
#[cfg(test)]
mod tests;
