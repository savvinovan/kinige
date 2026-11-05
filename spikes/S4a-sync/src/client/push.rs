//! Push: the next batch of the outbox and the device's reaction to each answer (appendix §A.3).

use super::Client;
use super::Event;
use crate::envelope::{Envelope, Key, Kind};
use crate::server::PushResponse;
use kinige_domain::sync::Snapshot;
use std::sync::Arc;

impl Client {
    /// The next unacked rows, in outbox order, marked as handed to the network. A cascade child's `purged` waits for its parent's
    /// ack (§6.2 step 3).
    pub fn next_batch(&mut self, max: usize) -> Vec<Arc<Envelope>> {
        if !self.bound || !self.cycle_ready {
            return Vec::new();
        }
        self.cycle_ready = false;
        let waiting: Vec<String> = self
            .outbox
            .iter()
            .filter(|row| row.acked_seq.is_none())
            .map(|row| row.env.header.op_id.clone())
            .collect();
        let mut batch = Vec::new();
        for row in &mut self.outbox {
            if batch.len() >= max {
                break;
            }
            let blocked = row
                .blocked_by
                .as_ref()
                .is_some_and(|parent| waiting.contains(parent));
            if row.acked_seq.is_none() && !blocked {
                row.sent = true;
                batch.push(Arc::clone(&row.env));
            }
        }
        batch
    }

    /// The device's reaction to the server's answer for one `op_id`.
    pub fn on_push_response(&mut self, op_id: &str, response: PushResponse) {
        let Some(row) = self.outbox.iter().find(|row| row.env.header.op_id == op_id) else {
            return;
        };
        let key = row.env.header.key();
        let kind = row.env.header.kind;
        let snapshot = row.env.header.purged_of.clone().unwrap_or_default();
        match response {
            PushResponse::Ok(seq) | PushResponse::Duplicate(seq) => {
                self.acked(op_id, &key, kind, &snapshot, seq)
            }
            PushResponse::OpConflict => self.events.push(Event::OpConflict),
            PushResponse::Purged => {
                self.outbox.retain(|row| row.env.header.op_id != op_id);
                self.purged_response(&key);
            }
            PushResponse::StalePurge => {
                // Pull and re-evaluate (the next purge pass makes a fresh snapshot). The tombstones of this cascade's children are not
                // sent: their rows are dropped (§3.5, §A.3).
                let parent = key.1.clone();
                let children: Vec<Key> = self
                    .tombstones
                    .iter()
                    .filter(|(_, t)| t.cascade_of.as_deref() == Some(parent.as_str()))
                    .map(|(k, _)| k.clone())
                    .collect();
                self.outbox.retain(|row| {
                    let child = row.env.header.kind == Kind::Purged
                        && row.acked_seq.is_none()
                        && children.contains(&row.env.header.key());
                    row.env.header.op_id != op_id
                        && row.blocked_by.as_deref() != Some(op_id)
                        && !child
                });
            }
            PushResponse::ClockSkew => {}
        }
    }

    /// An ack, from the push response or recovered from the device's own `op_id` in pull.
    pub(super) fn acked(
        &mut self,
        op_id: &str,
        key: &Key,
        kind: Kind,
        snapshot: &Snapshot,
        seq: u64,
    ) {
        if let Some(row) = self
            .outbox
            .iter_mut()
            .find(|row| row.env.header.op_id == op_id)
        {
            row.acked_seq = Some(seq);
        }
        match kind {
            Kind::Upsert => {
                if let Some(local) = self.records.get_mut(key) {
                    local.server_known = true;
                }
            }
            Kind::Purged => self.purge_acked(key, snapshot),
        }
    }
}
