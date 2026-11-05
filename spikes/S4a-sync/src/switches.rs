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
    /// Server: a drop is held against a cursor by `seq` and epoch alone, whatever generation it was made in (revision 8 as first
    /// written). A restored server issues `seq` again from the backup's head, so a drop of the new generation can sit below an old
    /// cursor that had gone further: the guardian's second blocker (round 2).
    pub drop_seq_across_generations: bool,
    /// Client: the drop epoch of the old generation is kept after a generation change, as ADR-0001 revision 8 said. A restored server
    /// counts epochs from its backup, so a later drop can carry a number the device already knows and is not seen as a drop (the
    /// guardian's first blocker, round 1).
    pub epoch_survives_generation: bool,
    /// Client: a conflict is made when the winner was made from the device's own unconfirmed version (its `base`), as before
    /// ADR-0001 revision 8 (S4a, finding 5): a false conflict that property 9 catches.
    pub own_base_is_concurrent: bool,
    /// Client: a resync that began because the `generation` changed does not notice a compaction that drops a `purged` while it is
    /// still pulling from 0 (a pull from 0 is never stale), so what the server dropped looks like what the restore lost and is pushed
    /// back (found by the random restore schedules in round 2).
    pub no_epoch_check_in_resync: bool,
    /// Client: an envelope of the device's own, met again in a pull, is only an ack and never merged, even when a purge and a lift
    /// have left the local record without the device's own writes (found by the random restore schedules in round 2).
    pub own_envelope_not_merged: bool,
    /// Client: an envelope for a record that is in quarantine is applied live (nothing in ADR-0001 revision 8 said otherwise), so the
    /// record is in the feed and in quarantine at once.
    pub quarantine_applies_live: bool,
    /// Client: on the ack of its own `purged(chat)` the initiator deletes the chat's messages and their revisions at once, as ADR-0001
    /// revision 7 says; a stale `purged` for a message that was moved elsewhere then loses what only this device had (S4a, finding 3).
    pub cascade_at_parent_ack: bool,
    /// Client feed: the children of a purged chat stay hidden for as long as the tombstone lives, as ADR-0001 revision 7 says; the
    /// ones whose own `purged` never comes are then invisible for months (S4a, finding 4).
    pub no_child_grace: bool,
    /// Client feed: the initiator of a chat purge shows the chat's messages in Inbox after the grace although their own `purged` still
    /// sit in its outbox (ADR-0001 revision 8 said nothing about it; S4a review).
    pub show_children_with_purge_pending: bool,
    /// Client feed: a message whose chat is unknown is shown in that chat at once instead of waiting for the parent (§3.6).
    pub show_orphans: bool,
    /// Client: a changed `generation` is ignored, the old cursor and `server_known` flags are kept (§6.3).
    pub ignore_generation: bool,
}
