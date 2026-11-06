//! What a pull (and the cursor of a push) is told about its cursor `(generation, seq)` and the drop `epoch` it last saw: the stale
//! rule with drops stored with their generation, and the placement of a generation that is not in the lineage (ADR-0001 §6.3,
//! ADR-0017). Ported from `spikes/S4a-sync/src/server.rs::missed`, which the S4a properties and negative controls pin.

/// A compaction that dropped something a device cannot do without: a `purged` and the `upsert`s of the entity it purged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Drop {
    /// The generation it was made in: its `seq` and `epoch` mean something only against a cursor of the same generation.
    pub generation: u64,
    pub epoch: u64,
    /// The highest `seq` dropped.
    pub seq: u64,
    /// When the compaction ran; S4c-2's branch-cut rule reads it (ADR-0017).
    pub compacted_ms: u64,
}

/// What the device sent: where it is, and the epoch of the last page it applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    pub generation: u64,
    pub after: u64,
    pub known_epoch: u64,
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
pub fn plan(lineage: &[u64], drops: &[Drop], cursor: &Cursor) -> Plan {
    let missed = missed(lineage, drops, cursor);
    if lineage.last() != Some(&cursor.generation) {
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
/// A generation that is not in the lineage was made after the backup the lineage ends in, so it stands between the backup's and the
/// current one: it misses the drops of the current generation only.
pub fn missed(lineage: &[u64], drops: &[Drop], cursor: &Cursor) -> bool {
    if cursor.after == 0 {
        return false;
    }
    let place = |generation: u64| lineage.iter().position(|g| *g == generation);
    // Positions are doubled and shifted by two so that an unknown generation can sit between two known ones, and below the only one
    // when the lineage has a single generation (then every drop of the current generation is missed).
    let unknown = if lineage.len() < 2 {
        1
    } else {
        2 * lineage.len().saturating_sub(2) + 3
    };
    let at = place(cursor.generation).map_or(unknown, |p| 2 * p + 2);
    drops.iter().any(|drop| {
        let made = place(drop.generation).map_or(0, |p| 2 * p + 2);
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
    fn a_generation_outside_the_lineage_misses_only_the_current_generations_drops() {
        let lineage = [1, 3];
        let abandoned = 2;
        // Made in the backup's generation, before the branch was cut: held by a device that pulled from 0 on the branch.
        assert_eq!(
            plan(&lineage, &[drop_at(1, 1, 40)], &cursor(abandoned, 10, 0)),
            Plan::GenerationChanged { stale: false }
        );
        assert_eq!(
            plan(&lineage, &[drop_at(3, 1, 4)], &cursor(abandoned, 10, 0)),
            Plan::GenerationChanged { stale: true }
        );
        // With a single generation an unknown one sits below it.
        assert_eq!(
            plan(&[7], &[drop_at(7, 1, 4)], &cursor(abandoned, 10, 0)),
            Plan::GenerationChanged { stale: true }
        );
    }
}
