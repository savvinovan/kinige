//! Test-only switches that break one rule on purpose (spec: "a negative control for each property"). A property that passes on
//! broken code proves nothing, so each property has a test that turns one of these on and expects the property to fail.
//! All are off in every real run. A switch changes the spike's own copy of a rule, never `kinige-domain`.

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Switches {
    /// Client merge: an HLC tie between devices is broken by arrival order instead of by `device_id` (§3.1, §2).
    pub no_device_tiebreak: bool,
    /// Client writer: `body` / `placement` carry no `vv`, which is the rule as ADR-0001 revision 7 states it (`base` and `supersedes`
    /// only). The counterexample of S4a, finding 2, needs this on.
    pub no_vv: bool,
    /// Client: a lost concurrent `body` / `placement` version is dropped instead of kept as a `conflict` revision (§3.4).
    pub no_conflict_revision: bool,
    /// Client merge: a `deleted_at` on either side wins, however old; a restore never wins (§3.5).
    pub delete_always_wins: bool,
    /// Client clock: a received HLC does not move the local clock, so a restore made after seeing a delete can get a smaller stamp.
    pub no_clock_receive: bool,
    /// Server: staleness of a cursor is not checked, so a device back after 200 days is never told to resync (§6.3).
    pub no_stale_check: bool,
    /// Server: `seq` is issued, but the envelope becomes visible one call later, after the next one (no common lock, §6.1).
    pub no_seq_lock: bool,
    /// Server: a repeated `op_id` gets a new `seq` instead of `duplicate` (§A.4 step 3).
    pub no_dedup: bool,
    /// Client merge: the record with the newest group wins as a whole, other groups are lost (the "LWW of a whole record" the ADR
    /// refuses, decision 12).
    pub whole_record_lww: bool,
    /// Client: a pulled `purged(X)` leaves an existing tombstone's snapshot as it was (ADR-0001 revision 7 refines it only after a
    /// `purged` response to a push). S4a, finding 6.
    pub keep_own_snapshot: bool,
    /// Client: on the ack of its own `purged(chat)` the initiator deletes the chat's messages and their revisions at once, as ADR-0001
    /// revision 7 says; a stale `purged` for a message that was moved elsewhere then loses what only this device had (S4a, finding 3).
    pub cascade_at_parent_ack: bool,
    /// Client feed: the children of a purged chat stay hidden for as long as the tombstone lives, as ADR-0001 revision 7 says; the
    /// ones whose own `purged` never comes are then invisible for months (S4a, finding 4).
    pub no_child_grace: bool,
    /// Client feed: a message whose chat is unknown is shown in that chat at once instead of waiting for the parent (§3.6).
    pub show_orphans: bool,
    /// Client: a changed `generation` is ignored, the old cursor and `server_known` flags are kept (§6.3).
    pub ignore_generation: bool,
}
