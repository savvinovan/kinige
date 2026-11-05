//! Purge on a device (ADR-0001 §3.5, §3.6): enqueueing `purged` after 30 days, the ack, a pulled `purged`, a `purged` response, the
//! cascade to children and lifting a tombstone. The verdicts are `kinige_domain::sync::purge`.

use super::{Client, Quarantined, Tombstone};
use crate::client::ops::inbox_key;
use crate::envelope::{Entity, Key, Kind, PURGE_AFTER_MS};
use kinige_domain::sync::{PurgeVerdict, Snapshot, Versioned, is_chat_child, on_purge};
use serde_json::Value;

/// Where a purge comes from; the rules differ in who deletes the children and whether a tombstone is confirmed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ctx {
    /// The ack of this device's own `purged` (§3.5): the record and its children are deleted, the children's `purged` are queued.
    OwnAck,
    /// A purge without an account: local only, the tombstone waits for binding (§3.5).
    OwnLocal,
    /// Another device's `purged(X)` from pull (§3.5): children of a message are deleted too, children of a chat are not.
    Pulled,
    /// A `purged` response to a push: the response carries no snapshot, so a live record goes to quarantine (§3.5).
    PushResponse,
}

impl Client {
    /// The purge pass: everything in the trash for 30 days by this device's clock (§3.5). The simulation runs it at the end of a sync
    /// cycle, on a complete view: not in the middle of a full pull, and for a device with an account right after a pull that reached the head.
    pub fn purge_due(&mut self, now_ms: u64) {
        if self.resync.is_some() || (self.bound && !self.cycle_ready) {
            return;
        }
        let pt = self.pt(now_ms);
        let due: Vec<(Key, Snapshot)> = self
            .records
            .iter()
            .filter(|((entity, _), _)| *entity != Entity::Revision)
            .filter(|(key, local)| **key != inbox_key() && !local.v.is_read_only())
            .filter(|(_, local)| {
                deleted_at(&local.v).is_some_and(|at| pt >= at.saturating_add(PURGE_AFTER_MS))
            })
            .map(|(key, local)| (key.clone(), local.v.snapshot()))
            .collect();
        for (key, snapshot) in due {
            if self.bound {
                let pending = self.outbox.iter().any(|row| {
                    row.env.header.kind == Kind::Purged
                        && row.env.header.key() == key
                        && row.acked_seq.is_none()
                });
                if !pending {
                    self.enqueue_purged(&key, snapshot, None);
                }
            } else {
                self.apply_purge(&key, &snapshot, Ctx::OwnLocal);
            }
        }
    }

    /// The server accepted this device's `purged` (or it came back in pull): handle it in one step (§3.5).
    pub(super) fn purge_acked(&mut self, key: &Key, snapshot: &Snapshot) {
        if let Some(tombstone) = self.tombstones.get_mut(key) {
            // A re-sent tombstone (binding, new generation) or one already applied: the server has it now.
            tombstone.confirmed = true;
            tombstone.cascade_of = None;
            tombstone.held = None;
            return;
        }
        self.apply_purge(key, snapshot, Ctx::OwnAck);
    }

    /// `purged(X)` from another device in pull.
    pub(super) fn pulled_purged(&mut self, key: &Key, snapshot: &Snapshot) {
        if let Some(tombstone) = self.tombstones.get_mut(key) {
            // The child's own purge is confirmed by the server: lifting the parent no longer resurrects it.
            tombstone.confirmed = true;
            tombstone.cascade_of = None;
            tombstone.held = None;
            // S4a (finding 6): the `purged` in the log is the one the server accepted, so its snapshot is the one a later envelope is
            // judged by. The device's own tombstone may carry an older snapshot (its own `purged` was a no-op repeat), and an envelope
            // that is newer than that but not newer than the accepted one would lift a purge the server already made.
            if !self.switches.keep_own_snapshot {
                for (group, hlc) in snapshot {
                    let slot = tombstone.purged_of.entry(group.clone()).or_default();
                    if *hlc > *slot {
                        slot.clone_from(hlc);
                    }
                }
            }
            return;
        }
        self.apply_purge(key, snapshot, Ctx::Pulled);
    }

    /// The `purged` answer to a push of `key` (§3.5, §A.3).
    pub(super) fn purged_response(&mut self, key: &Key) {
        self.drop_unacked_upserts(std::slice::from_ref(key));
        if self.tombstones.contains_key(key) {
            return;
        }
        if let Some(snapshot) = self.records.get(key).map(|local| local.v.snapshot()) {
            self.apply_purge(key, &snapshot, Ctx::PushResponse);
        }
    }

    /// An envelope newer than the tombstone arrived: the purge was stale. The tombstone is lifted together with those of the cascade's
    /// children, which are recreated from what was held for them (§3.5). A lifted tombstone takes its unsent `purged` with it.
    pub(super) fn lift_tombstone(&mut self, key: &Key) {
        self.events.push(super::Event::Lifted(key.clone()));
        self.tombstones.remove(key);
        self.outbox.retain(|row| {
            !(row.env.header.kind == Kind::Purged
                && row.acked_seq.is_none()
                && row.env.header.key() == *key)
        });
        let children: Vec<Key> = self
            .tombstones
            .iter()
            .filter(|(_, t)| t.cascade_of.as_deref() == Some(key.1.as_str()))
            .map(|(k, _)| k.clone())
            .collect();
        for child in children {
            let held = self.tombstones.get(&child).and_then(|t| t.held.clone());
            self.lift_tombstone(&child);
            if let Some(v) = held {
                self.records.insert(
                    child,
                    super::Local {
                        v,
                        server_known: true,
                    },
                );
            }
        }
    }

    fn apply_purge(&mut self, key: &Key, snapshot: &Snapshot, ctx: Ctx) {
        let present = self.records.get(key).map(|local| local.v.clone());
        let (verdict, snapshot) = match (&present, ctx) {
            (Some(v), Ctx::PushResponse) => (PurgeVerdict::Quarantine, v.snapshot()),
            (Some(v), _) => (on_purge(&v.snapshot(), snapshot), snapshot.clone()),
            (None, _) => (PurgeVerdict::DeleteSilently, snapshot.clone()),
        };
        let kids = self.descendants(key, &snapshot, ctx, verdict);
        // The initiator's chat messages stay where they are; their own `purged` go out now, with a snapshot of what the device has.
        let messages_to_purge: Vec<(Key, Snapshot)> =
            if key.0 == Entity::Chat && ctx == Ctx::OwnAck && kids.is_empty() {
                self.chat_messages(&key.1, &snapshot)
            } else {
                Vec::new()
            };

        let mut gone = vec![key.clone()];
        let mut kid_snapshots = Vec::new();
        if let Some(local) = self.records.remove(key) {
            self.leave(key.clone(), local.v, verdict);
        }
        for kid in kids {
            if let Some(local) = self.records.remove(&kid) {
                // `cascade_of` is the message for a revision and the purged parent (the chat) for a message.
                let parent = match kid.0 {
                    Entity::Revision => message_id_of(&local.v).unwrap_or_else(|| key.1.clone()),
                    _ => key.1.clone(),
                };
                kid_snapshots.push((kid.clone(), local.v.snapshot(), parent));
                self.leave(kid.clone(), local.v, verdict);
                gone.push(kid);
            }
        }
        self.tombstones.insert(
            key.clone(),
            Tombstone {
                purged_of: snapshot,
                cascade_of: None,
                held: None,
                confirmed: ctx != Ctx::OwnLocal,
                purged_at: self.now_ms,
            },
        );
        for (kid, kid_snapshot, parent) in &kid_snapshots {
            self.tombstones.insert(
                kid.clone(),
                Tombstone {
                    purged_of: kid_snapshot.clone(),
                    cascade_of: Some(parent.clone()),
                    held: None,
                    confirmed: false,
                    purged_at: self.now_ms,
                },
            );
        }
        if ctx == Ctx::OwnAck {
            for (kid, kid_snapshot, _) in kid_snapshots {
                self.enqueue_purged(&kid, kid_snapshot, None);
            }
            for (message, message_snapshot) in messages_to_purge {
                let pending = self.outbox.iter().any(|row| {
                    row.env.header.kind == Kind::Purged
                        && row.env.header.key() == message
                        && row.acked_seq.is_none()
                });
                if !pending {
                    self.enqueue_purged(&message, message_snapshot, None);
                }
            }
        }
        self.drop_unacked_upserts(&gone);
    }

    fn leave(&mut self, key: Key, v: Versioned, verdict: PurgeVerdict) {
        if verdict == PurgeVerdict::Quarantine {
            self.quarantine.push(Quarantined {
                key,
                v,
                reason: "purged while edited",
            });
        }
    }

    /// The children a purge of `key` takes with it (§3.5, §3.6): a message's revisions always; a chat's messages (and theirs) when this
    /// device deletes them on its own ack or purges locally, or when the chat goes to quarantine; a recipient does not cascade a chat.
    fn descendants(
        &self,
        key: &Key,
        snapshot: &Snapshot,
        ctx: Ctx,
        verdict: PurgeVerdict,
    ) -> Vec<Key> {
        match key.0 {
            Entity::Revision => Vec::new(),
            Entity::Message => self.revisions_of(&key.1),
            Entity::Chat => {
                // S4a (finding 3): on its own ack the initiator does NOT delete the chat's messages; each is deleted when its own
                // `purged` is acked. The literal ADR rule (`cascade_at_parent_ack`) deletes them at once and loses local data.
                let cascade = matches!(ctx, Ctx::OwnLocal | Ctx::PushResponse)
                    || (ctx == Ctx::OwnAck && self.switches.cascade_at_parent_ack)
                    || verdict == PurgeVerdict::Quarantine;
                if !cascade {
                    return Vec::new();
                }
                let mut out = Vec::new();
                for (k, local) in &self.records {
                    if k.0 != Entity::Message
                        || chat_id_of(&local.v).as_deref() != Some(key.1.as_str())
                    {
                        continue;
                    }
                    let placed = local
                        .v
                        .get("placement")
                        .map(|g| g.hlc.as_str())
                        .unwrap_or_default();
                    if is_chat_child(placed, snapshot) {
                        out.push(k.clone());
                        out.extend(self.revisions_of(&k.1));
                    }
                }
                out
            }
        }
    }

    /// The messages that are children of the chat being purged (§3.6) with a snapshot of each one's groups as the device has them.
    fn chat_messages(&self, chat_id: &str, chat_purged_of: &Snapshot) -> Vec<(Key, Snapshot)> {
        self.records
            .iter()
            .filter(|(k, local)| {
                k.0 == Entity::Message
                    && chat_id_of(&local.v).as_deref() == Some(chat_id)
                    && local
                        .v
                        .get("placement")
                        .is_some_and(|g| is_chat_child(&g.hlc, chat_purged_of))
            })
            .map(|(k, local)| (k.clone(), local.v.snapshot()))
            .collect()
    }

    pub(super) fn revisions_of(&self, message_id: &str) -> Vec<Key> {
        self.records
            .iter()
            .filter(|((entity, _), local)| {
                *entity == Entity::Revision
                    && message_id_of(&local.v).as_deref() == Some(message_id)
            })
            .map(|(key, _)| key.clone())
            .collect()
    }
}

/// `deleted_at` of the `deleted` group, in ms.
pub fn deleted_at(v: &Versioned) -> Option<u64> {
    let group = v.get("deleted")?;
    serde_json::from_str::<Value>(&group.raw)
        .ok()?
        .get("deleted_at")?
        .as_u64()
}

/// The chat a message's `placement` names.
pub fn chat_id_of(v: &Versioned) -> Option<String> {
    let group = v.get("placement")?;
    Some(
        serde_json::from_str::<Value>(&group.raw)
            .ok()?
            .get("chat_id")?
            .as_str()?
            .to_string(),
    )
}

/// The message a revision's `core` names.
pub fn message_id_of(v: &Versioned) -> Option<String> {
    let group = v.get("core")?;
    Some(
        serde_json::from_str::<Value>(&group.raw)
            .ok()?
            .get("message_id")?
            .as_str()?
            .to_string(),
    )
}
