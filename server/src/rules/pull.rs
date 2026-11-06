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

/// What the device sent: where it is, and the epoch of the last page it applied. `cut_ms` is the cut time that came with the
/// `GenerationChanged` answer which put the device into its generation (0 if it never had one).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    pub generation: u64,
    pub after: u64,
    pub known_epoch: u64,
    pub cut_ms: u64,
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
/// compacted after that backup. The device sends the cut time of its branch (the one `GenerationChanged` gave it); every drop of this
/// server compacted at or after that time is one it has not seen, whichever generation it was made in. A device that sends no cut
/// time (0) has missed every drop: the answer is "stale", the safe side.
pub fn missed(lineage: &[Generation], drops: &[Drop], cursor: &Cursor) -> bool {
    if cursor.after == 0 {
        return false;
    }
    let place = |generation: u64| lineage.iter().position(|g| g.id == generation);
    let Some(at) = place(cursor.generation) else {
        // `>=`: a drop stamped at the cut time itself may or may not be in the backup, so it counts as missed.
        return drops.iter().any(|drop| drop.compacted_ms >= cursor.cut_ms);
    };
    drops.iter().any(|drop| {
        let made = place(drop.generation).unwrap_or(0);
        made > at || (made == at && drop.epoch > cursor.known_epoch && drop.seq > cursor.after)
    })
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
            cut_ms: 0,
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
    fn a_generation_outside_the_lineage_is_placed_by_its_cut_time() {
        let lineage = [1, 3];
        let abandoned = 2;
        let branch = |cut_ms: u64| Cursor {
            cut_ms,
            ..cursor(abandoned, 10, 0)
        };
        // Compacted before the backup the branch was cut from: the branch holds the result.
        assert_eq!(
            plan(&lineage, &[drop_at_ms(1, 1, 40, 100)], &branch(200)),
            Plan::GenerationChanged { stale: false }
        );
        // Compacted after it: missed, in whatever generation it was made.
        for generation in [1, 3] {
            assert_eq!(
                plan(&lineage, &[drop_at_ms(generation, 1, 4, 300)], &branch(200)),
                Plan::GenerationChanged { stale: true },
                "generation {generation}"
            );
        }
        // At the cut time itself it may or may not be in the backup: the safe side.
        assert_eq!(
            plan(&lineage, &[drop_at_ms(1, 1, 40, 200)], &branch(200)),
            Plan::GenerationChanged { stale: true }
        );
        // A device that sends no cut time has missed every drop, and a server with no drops has nothing to miss.
        assert_eq!(
            plan(&lineage, &[drop_at_ms(1, 1, 40, 5)], &branch(0)),
            Plan::GenerationChanged { stale: true }
        );
        assert_eq!(
            plan(&lineage, &[], &branch(0)),
            Plan::GenerationChanged { stale: false }
        );
    }

    #[test]
    fn the_branch_cut_from_an_older_backup_than_the_one_restored_now_is_stale() {
        // ADR-0017, round 3: the first restore used the older backup (cut at 100), the second a newer one that already holds a drop
        // compacted at 150. The device lives on the abandoned first branch; it must be told it missed the drop.
        let lineage = [1, 3];
        let on_the_old_branch = Cursor {
            cut_ms: 100,
            ..cursor(2, 10, 0)
        };
        assert_eq!(
            plan(&lineage, &[drop_at_ms(1, 1, 40, 150)], &on_the_old_branch),
            Plan::GenerationChanged { stale: true }
        );
    }

    #[test]
    fn a_pull_from_zero_on_an_unknown_generation_is_never_stale() {
        let on_a_branch = Cursor {
            cut_ms: 0,
            ..cursor(2, 0, 0)
        };
        assert_eq!(
            plan(&[1, 3], &[drop_at_ms(1, 1, 40, 150)], &on_a_branch),
            Plan::GenerationChanged { stale: false }
        );
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
