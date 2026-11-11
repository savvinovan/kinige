//! The oracle: a record of every write a user action made, kept outside the clients, and the checks that compare the converged
//! state with it. The checks never use the rules under test (`merge`, `needs_conflict_revision`): they ask only what a person
//! would, "is the newest write there", "is every version still somewhere".

use crate::backend::Cursor;
use crate::client::{
    Client, FeedSlot, Write,
    purge::{chat_id_of, deleted_at},
};
use crate::envelope::{Entity, Key};
use crate::server::PushResponse;
use crate::switches::Switches;
use crate::world::World;
use kinige_domain::sync::Versioned;
use kinige_domain::sync::conflict::content_differs;
use std::collections::{BTreeMap, BTreeSet, HashMap};

#[derive(Default)]
pub struct Oracle {
    pub writes: Vec<Write>,
    /// `(record, group, hlc) -> the HLC of the same group on the writing device just before`: the causal parent.
    parents: HashMap<(Key, String, String), Option<String>>,
    /// `(record, group, hlc) -> the group's bytes`.
    raws: HashMap<(Key, String, String), String>,
    last: BTreeMap<usize, (String, Key)>,
}

impl Oracle {
    /// Records a write and checks the clock: the stamp is above its causal parent (even when the device clock is behind, §2), and a
    /// device never repeats or reverses a stamp (also across restarts).
    pub fn record(&mut self, write: Write) -> Option<String> {
        let mut problem = None;
        if let Some(parent) = &write.parent
            && write.hlc <= *parent
        {
            problem = Some(format!(
                "write {} of {:?}.{} is not above its causal parent {parent}",
                write.hlc, write.key, write.group
            ));
        }
        if let Some((last_hlc, last_key)) = self.last.get(&write.device) {
            let fine = write.hlc > *last_hlc || (write.hlc == *last_hlc && write.key == *last_key);
            if !fine {
                problem = Some(format!(
                    "device {} wrote {} after {last_hlc}: a stamp repeated or went back",
                    write.device, write.hlc
                ));
            }
        }
        self.last
            .insert(write.device, (write.hlc.clone(), write.key.clone()));
        self.parents.insert(
            (write.key.clone(), write.group.clone(), write.hlc.clone()),
            write.parent.clone(),
        );
        self.raws.insert(
            (write.key.clone(), write.group.clone(), write.hlc.clone()),
            write.raw.clone(),
        );
        self.writes.push(write);
        problem
    }

    /// The `deleted` group at `hlc` was written by a person restoring the record from the trash (not by its creation).
    pub fn is_restore(&self, key: &Key, hlc: &str) -> bool {
        self.writes.iter().any(|w| {
            w.key == *key
                && w.group == "deleted"
                && w.hlc == hlc
                && w.parent.is_some()
                && w.raw.contains("null")
        })
    }

    /// Some write was made from this version (it is somebody's causal parent).
    fn has_successor(&self, key: &Key, group: &str, hlc: &str) -> bool {
        self.parents
            .iter()
            .any(|((k, g, _), parent)| k == key && g == group && parent.as_deref() == Some(hlc))
    }

    /// The HLC itself and every causal ancestor of it, by following the writing devices' own `parent` links.
    fn ancestors(&self, key: &Key, group: &str, hlc: &str) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        let mut at = Some(hlc.to_string());
        while let Some(h) = at {
            if !out.insert(h.clone()) {
                break;
            }
            at = self
                .parents
                .get(&(key.clone(), group.to_string(), h))
                .cloned()
                .flatten();
        }
        out
    }
}

/// Records the server purged, and records a device holds in quarantine: those leave the "newest write is there" check.
fn set_aside(world: &World) -> BTreeSet<Key> {
    let mut out: BTreeSet<Key> = world.server.purged_ever().into_iter().collect();
    for client in &world.clients {
        out.extend(client.quarantine.iter().map(|q| q.key.clone()));
        out.extend(client.tombstones.keys().cloned());
    }
    out
}

/// The live records of a device that count for comparison. Quarantine is a device's own decision (ADR-0001 §3.5, §6.3): a message or
/// revision that one device holds in quarantine may be live on another, because the purger of its chat had not seen it and so never
/// purged it (S4a, finding 4). Nothing else may differ.
fn comparable(world: &World, client: &Client) -> BTreeMap<Key, Versioned> {
    let quarantined: BTreeSet<&Key> = world
        .clients
        .iter()
        .flat_map(|c| c.quarantine.iter().map(|q| &q.key))
        .collect();
    client
        .state()
        .into_iter()
        .filter(|(k, _)| {
            !(matches!(k.0, Entity::Message | Entity::Revision) && quarantined.contains(k))
        })
        .collect()
}

/// Property 1: every device has the same live records, with the same groups, HLCs and bytes.
pub fn converged(world: &World) -> Result<(), String> {
    let Some(first) = world.clients.first() else {
        return Ok(());
    };
    let reference = comparable(world, first);
    for client in &world.clients {
        let state = comparable(world, client);
        if state != reference {
            let only_a: Vec<_> = reference
                .keys()
                .filter(|k| !state.contains_key(*k))
                .collect();
            let only_b: Vec<_> = state
                .keys()
                .filter(|k| !reference.contains_key(*k))
                .collect();
            let differing: Vec<_> = reference
                .iter()
                .filter(|(k, v)| state.get(*k).is_some_and(|o| o != *v))
                .map(|(k, _)| k)
                .collect();
            return Err(format!(
                "device 0 and device {} differ: only on 0 {only_a:?}, only on {} {only_b:?}, different {differing:?}",
                client.idx, client.idx
            ));
        }
    }
    Ok(())
}

/// Properties 3 and 6: for every group of every record, the converged value is the write with the highest HLC, whatever the
/// order of delivery (a restore with a newer HLC beats the older delete; edits of different groups are both kept).
pub fn newest_write_wins(world: &World) -> Result<(), String> {
    let Some(first) = world.clients.first() else {
        return Ok(());
    };
    let state = first.state();
    let aside = set_aside(world);
    let mut newest: BTreeMap<(Key, String), &Write> = BTreeMap::new();
    for write in &world.oracle.writes {
        let slot = newest
            .entry((write.key.clone(), write.group.clone()))
            .or_insert(write);
        if write.hlc > slot.hlc {
            *slot = write;
        }
    }
    for ((key, group), write) in newest {
        if aside.contains(&key) {
            continue;
        }
        match state.get(&key).and_then(|v| v.get(&group)) {
            None => {
                return Err(format!(
                    "{key:?} (group {group}) is not on the devices; the newest write was {}",
                    write.hlc
                ));
            }
            Some(have) if have.hlc != write.hlc => {
                return Err(format!(
                    "{key:?}.{group}: the devices hold {}, the newest write is {}",
                    have.hlc, write.hlc
                ));
            }
            Some(_) => {}
        }
    }
    Ok(())
}

/// Property 2: no version of a message's `body` or `placement` is lost. Every version ever written is the winner, a causal ancestor
/// of the winner (it was edited into it), says the same as the winner, or is kept as a `conflict` revision (or in quarantine).
pub fn no_version_lost(world: &World) -> Result<(), String> {
    let Some(first) = world.clients.first() else {
        return Ok(());
    };
    let state = first.state();
    let aside = set_aside(world);
    let mut kept: BTreeSet<(String, String, String)> = BTreeSet::new();
    let mut note_revision = |v: &Versioned| {
        let Some(core) = v
            .get("core")
            .and_then(|g| serde_json::from_str::<serde_json::Value>(&g.raw).ok())
        else {
            return;
        };
        let Some(snapshot) = core
            .get("snapshot_json")
            .and_then(|s| s.as_str())
            .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
        else {
            return;
        };
        let text = |value: &serde_json::Value, field: &str| {
            value
                .get(field)
                .and_then(|x| x.as_str())
                .unwrap_or_default()
                .to_string()
        };
        kept.insert((
            text(&core, "message_id"),
            text(&snapshot, "g"),
            text(&snapshot, "h"),
        ));
    };
    for ((entity, _), v) in &state {
        if *entity == Entity::Revision {
            note_revision(v);
        }
    }
    for client in &world.clients {
        for q in &client.quarantine {
            if q.key.0 == Entity::Revision {
                note_revision(&q.v);
            }
        }
    }
    for write in &world.oracle.writes {
        if write.key.0 != Entity::Message
            || !matches!(write.group.as_str(), "body" | "placement")
            || aside.contains(&write.key)
        {
            continue;
        }
        // Only the end of a branch can be a lost concurrent version. A version its own author edited again was replaced, not lost.
        if world
            .oracle
            .has_successor(&write.key, &write.group, &write.hlc)
        {
            continue;
        }
        let Some(winner) = state.get(&write.key).and_then(|v| v.get(&write.group)) else {
            continue;
        };
        // The tips that are kept: the winner and every version kept as a revision. A version that was edited into a kept tip is
        // history of that branch (the user's own `edit` revisions keep it, slice 3), not a lost concurrent version.
        let mut tips = vec![winner.hlc.clone()];
        tips.extend(
            kept.iter()
                .filter(|(m, g, _)| *m == write.key.1 && *g == write.group)
                .map(|(_, _, h)| h.clone()),
        );
        // "Kept" is about what the version said: it survives if it is on the history of a kept tip, or if some version on that history
        // says the same (a move to the chat the message was already in loses nothing).
        let survives = tips.iter().any(|tip| {
            world
                .oracle
                .ancestors(&write.key, &write.group, tip)
                .iter()
                .any(|h| {
                    *h == write.hlc
                        || world
                            .oracle
                            .raws
                            .get(&(write.key.clone(), write.group.clone(), h.clone()))
                            .is_some_and(|raw| !content_differs(&write.group, &write.raw, raw))
                })
        });
        if !survives {
            return Err(format!(
                "a version of {:?}.{} written at {} is gone: not the winner ({}), not on the history of a kept version, no conflict revision",
                write.key, write.group, write.hlc, winner.hlc
            ));
        }
    }
    Ok(())
}

/// A record is in quarantine or in the live tables of a device, not both (S4a review: an envelope for a quarantined record stays in
/// quarantine). A revision that was made before its message went to quarantine stays live: it is not shown without its message.
pub fn quarantine_is_exclusive(world: &World) -> Result<(), String> {
    for client in &world.clients {
        for held in &client.quarantine {
            if client.records.contains_key(&held.key) {
                return Err(format!(
                    "device {}: {:?} is live and in quarantine at once",
                    client.idx, held.key
                ));
            }
        }
    }
    Ok(())
}

/// Property 4: nothing the server purged is back in a journal. It may sit in quarantine, never live.
pub fn purged_stays_gone(world: &World) -> Result<(), String> {
    for key in &world.server.purged_ever() {
        for client in &world.clients {
            if client.records.contains_key(key) {
                return Err(format!(
                    "{key:?} was purged, and is live again on device {}",
                    client.idx
                ));
            }
        }
    }
    Ok(())
}

/// Property 7: once pull reached the head, every live message is somewhere a person can see it, and the only ones that are not
/// shown are those in a chat that is in the trash. Before the head the feed check in `World::pull` ran at every step.
pub fn no_orphan_hidden(world: &World) -> Result<(), String> {
    for client in &world.clients {
        let shown: BTreeSet<String> = client.feed().into_iter().map(|item| item.message).collect();
        for item in client.feed() {
            if let FeedSlot::Chat(chat) = &item.slot
                && !client.records.contains_key(&(Entity::Chat, chat.clone()))
            {
                return Err(format!(
                    "device {}: message {} is shown in a chat the device does not have",
                    client.idx, item.message
                ));
            }
        }
        for ((entity, id), local) in &client.records {
            if *entity != Entity::Message || deleted_at(&local.v).is_some() || shown.contains(id) {
                continue;
            }
            let in_trash = chat_id_of(&local.v)
                .and_then(|chat| client.records.get(&(Entity::Chat, chat)))
                .is_some_and(|chat| deleted_at(&chat.v).is_some());
            if !in_trash {
                return Err(format!(
                    "device {}: message {id} is hidden after a complete pull and its chat is not in the trash",
                    client.idx
                ));
            }
        }
    }
    Ok(())
}

/// Property 5, the replay: every envelope in the server log is pushed again, in another order, twice. Every answer must be
/// `duplicate` with the same `seq`, and neither the log nor the numbering may change.
pub fn push_is_idempotent(world: &mut World) -> Result<(), String> {
    let issued = world.server.issued();
    let len = world.server.log_len();
    let generation = world.server.generation();
    let mut envelopes = world.server.all_envelopes();
    envelopes.reverse();
    let again = envelopes.clone();
    for env in envelopes.iter().chain(again.iter()) {
        let seq_before = world.server.seq_of(&env.header.op_id);
        // A cursor at 0 is never stale, so the replay is judged on `op_id` alone.
        let cursor = Cursor {
            generation,
            seq: 0,
            known_epoch: 0,
            lineage: Vec::new(),
        };
        let response = world.server.push(env, world.now, &cursor);
        match (response, seq_before) {
            (PushResponse::Duplicate(seq), Some(before)) if seq == before => {}
            (other, _) => {
                return Err(format!(
                    "a repeated push of {} got {other:?}, not a duplicate",
                    env.header.op_id
                ));
            }
        }
    }
    if world.server.issued() != issued || world.server.log_len() != len {
        return Err(format!(
            "repeated pushes changed the server: issued {issued} -> {}, log {len} -> {}",
            world.server.issued(),
            world.server.log_len()
        ));
    }
    Ok(())
}

/// A device that joins now and pulls the server's log from 0 ends with the converged state: the server holds everything, nothing
/// that was acked is only in the devices' heads (compaction, a restore from a backup and a lost push all leave it intact).
pub fn server_log_rebuilds_state(world: &mut World) -> Result<(), String> {
    let Some(reference) = world.clients.first().map(|c| comparable(world, c)) else {
        return Ok(());
    };
    let mut fresh = Client::new(
        world.clients.len(),
        0,
        false,
        false,
        Switches::default(),
        world.now,
    );
    fresh.records.clear();
    fresh.begin_binding(world.server.generation());
    for _ in 0..500 {
        let response = world.server.pull(fresh.pull_request(), 3);
        fresh.on_pull_response(response, world.now, |_| {});
        if fresh.resync.is_none() && fresh.cursor_seq >= world.server.head() {
            break;
        }
    }
    // Conflict revisions may depend on the order a device saw the versions in (ADR-0001 §3.1): the fresh device may make extra ones.
    // Every other record must be the same, and every revision the devices have must be on the server too.
    let rebuilt = comparable(world, &fresh);
    let relevant = |state: &BTreeMap<Key, Versioned>| -> BTreeMap<Key, Versioned> {
        state
            .iter()
            .filter(|(k, _)| k.0 != Entity::Revision)
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    };
    let lost_revisions: Vec<_> = reference
        .keys()
        .filter(|k| k.0 == Entity::Revision && rebuilt.get(*k) != reference.get(*k))
        .collect();
    if relevant(&rebuilt) != relevant(&reference) || !lost_revisions.is_empty() {
        let missing: Vec<_> = reference
            .keys()
            .filter(|k| !rebuilt.contains_key(*k))
            .collect();
        let extra: Vec<_> = rebuilt
            .keys()
            .filter(|k| !reference.contains_key(*k) && k.0 != Entity::Revision)
            .collect();
        let different: Vec<_> = reference
            .iter()
            .filter(|(k, v)| rebuilt.get(*k).is_some_and(|o| o != *v))
            .map(|(k, _)| k)
            .collect();
        return Err(format!(
            "a fresh device pulling the server log differs from the devices: not on the server {missing:?}, only on the server {extra:?}, different {different:?}"
        ));
    }
    Ok(())
}
