//! What a pull (and the cursor of a push) is told about its cursor `(generation, seq)` and the drop `epoch` it last saw: the stale
//! rule with drops stored with their generation, and the placement of a generation that is not in the lineage (ADR-0001 §6.3,
//! ADR-0017). Ported from `spikes/S4a-sync/src/server.rs::missed`, which the S4a properties and negative controls pin. S4c-2 adds the
//! branch-cut rule of ADR-0017: a generation outside the lineage is placed by the cut time it carries, not by a guess.

/// A compaction that dropped something a device cannot do without: a `purged` and the `upsert`s of the entity it purged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Drop {
    /// The generation it was made in: its `seq` and `epoch` mean something only against a cursor of the same generation.
    pub generation: u64,
    pub epoch: u64,
    /// The highest `seq` dropped.
    pub seq: u64,
    /// When the compaction ran, on the server's clock, read after the backup lock was taken (`store::lock`): a backup whose cut time is
    /// above it contains the drop (the branch-cut rule below).
    pub compacted_ms: u64,
}

/// One generation of the lineage: its opaque id and the cut time of the restore that made it (0 for the first generation, which no
/// restore made). The cut time is the time of the backup it was cut from (ADR-0017).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Generation {
    pub id: u64,
    pub cut_ms: u64,
}

/// The longest lineage the server keeps and a device may send (S4c-2, round 1 of the review). A restore appends a generation, so the
/// lineage grows with the number of restores; past this many the oldest are dropped. A device whose lineage no longer shares a
/// generation with the server's is treated as having missed everything (stale): never wrong, at worst a needless trip to quarantine.
pub const MAX_LINEAGE: usize = 64;

/// What the device sent: where it is, the epoch of the last page it applied, and its **lineage**: the lineage of the
/// `GenerationChanged` answer that put it into its generation, oldest first, ending with that generation (empty if it never had one).
/// A push carries no lineage: a pusher whose generation is not the current one is stale whatever the lineage says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cursor {
    pub generation: u64,
    pub after: u64,
    pub known_epoch: u64,
    pub lineage: Vec<Generation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plan {
    /// Serve the next page.
    Page,
    /// Something was dropped above the cursor since the device last heard (§6.3).
    Stale,
    /// The server was restored: the cursor's generation is not the current one. `stale` says whether the old cursor also missed a
    /// drop ("both causes at once", answered by the server).
    GenerationChanged { stale: bool },
}

/// `lineage` is the server's generations, oldest first, ending with the current one.
pub fn plan(lineage: &[Generation], drops: &[Drop], cursor: &Cursor) -> Plan {
    let missed = missed(lineage, drops, cursor);
    if lineage.last().map(|g| g.id) != Some(cursor.generation) {
        return Plan::GenerationChanged { stale: missed };
    }
    if missed { Plan::Stale } else { Plan::Page }
}

/// Whether the device with `cursor` has missed a drop. A pull from 0 is a full resync itself and reads what is left. Otherwise, by
/// where the drop's generation stands against the cursor's in the lineage:
/// - an older generation: the cursor was made after the drop, it cannot have missed it;
/// - the same generation: it missed it if the drop is above the cursor and newer than the last page the device applied;
/// - a newer generation: the cursor cannot have seen anything made in it, whatever the `seq`s say (a restored server issues `seq`
///   again from its backup's head).
///
/// A generation that is not in the lineage is a branch that a restore abandoned (S4c-2, the branch-cut rule of ADR-0017). A device on
/// it pulled from 0 when it entered the branch, so it holds what the backup the branch was cut from held, and nothing that was
/// compacted after that backup. **Which backup is the point:** the branch may itself have been restored from a backup of a branch
/// (backup, restore, backup of the branch, restore that, and so on), so the device's own generation's cut time is a time on another
/// branch's clock of events. What the device holds descends from the history it shares with this server up to the **fork**: the first
/// generation of the device's lineage, after the first one this server has, that this server's lineage does not have, and the cut
/// time of that generation is the time of the backup of the shared history it was restored from (the *fork cut*). Every drop of this
/// server compacted at or after the fork cut is one the device has not seen, whichever generation it was made in. The rule is
/// **conservative, not exact**: it never says "not missed" for a drop the device has missed, and it may say "missed" for one it has
/// not (a tie at the fork cut, a device that cannot be placed). A device that sends no lineage, or one that shares no generation with
/// this server's (it may have been trimmed away: the true fork is earlier than anything the device's lineage names), has missed every
/// drop ("stale", cut 0). The API refuses a lineage that does not end with the cursor's generation (`400 lineage`).
pub fn missed(lineage: &[Generation], drops: &[Drop], cursor: &Cursor) -> bool {
    if cursor.after == 0 {
        return false;
    }
    let place = |generation: u64| lineage.iter().position(|g| g.id == generation);
    let Some(at) = place(cursor.generation) else {
        // `>=`: a drop stamped at the fork cut itself may or may not be in the backup, so it counts as missed.
        let fork_cut = fork_cut(lineage, &cursor.lineage);
        return drops.iter().any(|drop| drop.compacted_ms >= fork_cut);
    };
    drops.iter().any(|drop| {
        let made = place(drop.generation).unwrap_or(0);
        made > at || (made == at && drop.epoch > cursor.known_epoch && drop.seq > cursor.after)
    })
}

/// The cut time of the fork: the first generation of the device's lineage **after the first one the server's lineage has** that the
/// server's lineage does not have; its cut time is the time, on the history both share, of the backup the device's branch was cut
/// from. The device's lineage may start before the server's does (the server keeps the newest `MAX_LINEAGE` generations), so the
/// entries before the first shared one are not a fork, they are history the server has trimmed. 0, which makes every drop missed,
/// when no such generation can be named: the device's lineage is empty, **shares no generation with the server's** (its first entry's
/// cut time is then a time on a history this server no longer has, and the true fork is somewhere before it), or has nothing foreign
/// after the shared part (nonsense for a cursor whose generation is foreign). A device that cannot be placed is never placed late.
fn fork_cut(server: &[Generation], device: &[Generation]) -> u64 {
    let shared = |g: &Generation| server.iter().any(|s| s.id == g.id);
    let Some(first_shared) = device.iter().position(shared) else {
        return 0;
    };
    device
        .iter()
        .skip(first_shared)
        .find(|g| !shared(g))
        .map_or(0, |g| g.cut_ms)
}

/// The size of one page: at least one envelope and at least one byte of budget. A budget of 0 would select no row, and `next` would then
/// be the head: the cursor would skip the log. The first envelope of a page always goes, whatever its size.
pub fn page_budget(limit: usize, bytes: usize) -> (usize, usize) {
    (limit.max(1), bytes.max(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_is_never_empty_by_configuration() {
        assert_eq!(page_budget(0, 0), (1, 1));
        assert_eq!(page_budget(200, 8 << 20), (200, 8 << 20));
        assert_eq!(page_budget(5, 0), (5, 1));
    }

    fn drop_at(generation: u64, epoch: u64, seq: u64) -> Drop {
        Drop {
            generation,
            epoch,
            seq,
            compacted_ms: 0,
        }
    }

    fn cursor(generation: u64, after: u64, known_epoch: u64) -> Cursor {
        Cursor {
            generation,
            after,
            known_epoch,
            lineage: Vec::new(),
        }
    }

    #[test]
    fn a_pull_from_zero_is_never_stale_and_no_drops_means_a_page() {
        let drops = [drop_at(1, 1, 50)];
        assert_eq!(plan(&[1], &drops, &cursor(1, 0, 0)), Plan::Page);
        assert_eq!(plan(&[1], &[], &cursor(1, 99, 0)), Plan::Page);
    }

    #[test]
    fn a_drop_above_the_cursor_and_newer_than_the_known_epoch_is_missed() {
        let drops = [drop_at(1, 1, 50)];
        assert_eq!(plan(&[1], &drops, &cursor(1, 10, 0)), Plan::Stale);
        assert_eq!(
            plan(&[1], &drops, &cursor(1, 60, 0)),
            Plan::Page,
            "the cursor is past the dropped seq"
        );
        assert_eq!(
            plan(&[1], &drops, &cursor(1, 10, 1)),
            Plan::Page,
            "the device already saw epoch 1"
        );
    }

    #[test]
    fn a_drop_of_a_later_generation_is_missed_whatever_the_seqs_say() {
        // The round 2 blocker of the S4a review: the restored server issues seq again from its backup's head, so the new generation's
        // drop sits below a cursor that had gone further.
        let lineage = [1, 2];
        let drops = [drop_at(2, 1, 5)];
        assert_eq!(
            plan(&lineage, &drops, &cursor(1, 90, 0)),
            Plan::GenerationChanged { stale: true }
        );
        assert_eq!(
            plan(&lineage, &drops, &cursor(2, 90, 0)),
            Plan::Page,
            "a cursor of the generation itself is compared by seq and epoch"
        );
    }

    #[test]
    fn a_drop_of_an_earlier_generation_is_never_missed() {
        let lineage = [1, 2];
        let drops = [drop_at(1, 4, 500)];
        assert_eq!(
            plan(&lineage, &drops, &cursor(2, 10, 0)),
            Plan::Page,
            "the cursor was made after the drop"
        );
        assert_eq!(
            plan(&lineage, &drops, &cursor(1, 10, 0)),
            Plan::GenerationChanged { stale: true },
            "in its own generation the drop is above the cursor"
        );
    }

    #[test]
    fn a_generation_changed_answer_says_whether_the_old_cursor_was_also_stale() {
        let lineage = [1, 2];
        assert_eq!(
            plan(&lineage, &[], &cursor(1, 10, 0)),
            Plan::GenerationChanged { stale: false }
        );
    }

    #[test]
    fn a_generation_outside_the_lineage_is_placed_by_the_cut_time_of_its_fork() {
        let server = [1, 3];
        // The device's lineage: the shared generation 1 and one branch, 2, restored from a backup cut at `cut_ms`.
        let branch = |cut_ms: u64| on_a_branch(&[(1, 0), (2, cut_ms)], 10);
        // Compacted before the backup the branch was cut from: the branch holds the result.
        assert_eq!(
            plan(&server, &[drop_at_ms(1, 1, 40, 100)], &branch(200)),
            Plan::GenerationChanged { stale: false }
        );
        // Compacted after it: missed, in whatever generation it was made.
        for generation in [1, 3] {
            assert_eq!(
                plan(&server, &[drop_at_ms(generation, 1, 4, 300)], &branch(200)),
                Plan::GenerationChanged { stale: true },
                "generation {generation}"
            );
        }
        // At the cut time itself it may or may not be in the backup: the safe side.
        assert_eq!(
            plan(&server, &[drop_at_ms(1, 1, 40, 200)], &branch(200)),
            Plan::GenerationChanged { stale: true }
        );
        // A server with no drops has nothing to miss.
        assert_eq!(
            plan(&server, &[], &branch(0)),
            Plan::GenerationChanged { stale: false }
        );
    }

    #[test]
    fn a_device_that_sends_no_lineage_or_one_the_server_shares_nothing_with_has_missed_every_drop()
    {
        let drops = [drop_at_ms(1, 1, 40, 5)];
        let nothing = Cursor {
            lineage: Vec::new(),
            ..cursor(2, 10, 0)
        };
        assert_eq!(
            plan(&[1, 3], &drops, &nothing),
            Plan::GenerationChanged { stale: true }
        );
        // The server trimmed its lineage (MAX_LINEAGE) past everything the device knows: no shared generation, the fork is the
        // device's first generation, cut 0.
        let strangers = on_a_branch(&[(7, 0), (8, 900)], 10);
        assert_eq!(
            plan(&[1, 3], &drops, &strangers),
            Plan::GenerationChanged { stale: true }
        );
        // Round 2 of the review: the device's own lineage was trimmed too, so its first entry has a non-zero cut time. Taken as the
        // fork it would say "compacted at 5, before 500: not missed", and the true fork is somewhere before it.
        let trimmed = on_a_branch(&[(5, 500), (6, 600)], 10);
        assert_eq!(
            plan(&[1, 3], &drops, &trimmed),
            Plan::GenerationChanged { stale: true }
        );
        // A lineage with nothing foreign in it names no fork: nonsense, answered with "stale".
        let nonsense = on_a_branch(&[(1, 0), (3, 50)], 10);
        assert_eq!(
            plan(
                &[1, 3],
                &drops,
                &Cursor {
                    generation: 2,
                    ..nonsense
                }
            ),
            Plan::GenerationChanged { stale: true }
        );
    }

    #[test]
    fn what_the_server_trimmed_from_the_front_of_the_device_lineage_is_history_and_not_a_fork() {
        // The server kept [3, 4] of a longer lineage; the device holds [1 (cut 10), 3 (cut 30), 9 (cut 90)]. Generation 1 is older than
        // the shared 3 and the server has trimmed it: the fork is 9, cut 90, not 1 (cut 10).
        let device = on_a_branch(&[(1, 10), (3, 30), (9, 90)], 10);
        let server = [3, 4];
        assert_eq!(
            plan(&server, &[drop_at_ms(4, 1, 40, 50)], &device),
            Plan::GenerationChanged { stale: false },
            "compacted at 50, before the fork cut 90: the device holds it"
        );
        assert_eq!(
            plan(&server, &[drop_at_ms(4, 1, 40, 95)], &device),
            Plan::GenerationChanged { stale: true }
        );
    }

    #[test]
    fn the_branch_cut_from_an_older_backup_than_the_one_restored_now_is_stale() {
        // ADR-0017, round 3: the first restore used the older backup (cut at 100), the second a newer one that already holds a drop
        // compacted at 150. The device lives on the abandoned first branch; it must be told it missed the drop.
        let device = on_a_branch(&[(1, 0), (2, 100)], 10);
        assert_eq!(
            plan(&[1, 3], &[drop_at_ms(1, 1, 40, 150)], &device),
            Plan::GenerationChanged { stale: true }
        );
    }

    #[test]
    fn a_branch_that_was_itself_restored_is_placed_by_where_it_forked_and_not_by_its_own_cut() {
        // The round 1 critical of the guardian. Backup B1 is cut at 100; a drop is compacted at 150; backup B2 is cut at 200 and holds
        // it. B1 is restored: generation 2, cut 100. A backup B3 of that branch is cut at 300 and restored: generation 4, cut 300. The
        // device enters generation 4 from 0 (so it has everything B1 had, alive), and then B2 is restored: the server's lineage is
        // [1, 3]. The device's own cut is 300, after the drop at 150, but what it holds comes from B1: its fork from the server's
        // history is generation 2, cut 100, and the drop at 150 is one it has not seen.
        let device = on_a_branch(&[(1, 0), (2, 100), (4, 300)], 10);
        assert_eq!(
            plan(&[1, 3], &[drop_at_ms(1, 1, 40, 150)], &device),
            Plan::GenerationChanged { stale: true }
        );
        // The old rule, the device's own cut, would have said no: that is the hole.
        let own_cut = device.lineage.last().map_or(0, |g| g.cut_ms);
        assert!(150 < own_cut);
        // A drop compacted before the fork is in what the device holds.
        assert_eq!(
            plan(&[1, 3], &[drop_at_ms(1, 1, 40, 90)], &device),
            Plan::GenerationChanged { stale: false }
        );
    }

    #[test]
    fn a_pull_from_zero_on_an_unknown_generation_is_never_stale() {
        let device = on_a_branch(&[(1, 0), (2, 0)], 0);
        assert_eq!(
            plan(&[1, 3], &[drop_at_ms(1, 1, 40, 150)], &device),
            Plan::GenerationChanged { stale: false }
        );
    }

    /// A cursor on the last generation of `lineage` (`(id, cut_ms)`, oldest first), `after` into it.
    fn on_a_branch(lineage: &[(u64, u64)], after: u64) -> Cursor {
        let lineage: Vec<Generation> = lineage
            .iter()
            .map(|(id, cut_ms)| Generation {
                id: *id,
                cut_ms: *cut_ms,
            })
            .collect();
        Cursor {
            generation: lineage.last().map_or(0, |g| g.id),
            after,
            known_epoch: 0,
            lineage,
        }
    }

    fn drop_at_ms(generation: u64, epoch: u64, seq: u64, compacted_ms: u64) -> Drop {
        Drop {
            compacted_ms,
            ..drop_at(generation, epoch, seq)
        }
    }

    /// A lineage of the given ids, none of them cut by a restore.
    fn lin(ids: &[u64]) -> Vec<Generation> {
        ids.iter()
            .map(|id| Generation { id: *id, cut_ms: 0 })
            .collect()
    }

    fn plan(ids: &[u64], drops: &[Drop], cursor: &Cursor) -> Plan {
        super::plan(&lin(ids), drops, cursor)
    }
}
