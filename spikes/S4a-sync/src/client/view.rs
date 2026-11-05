//! What the device shows (ADR-0001 §3.6) and the state two devices are compared by.

use super::Client;
use super::purge::{chat_id_of, deleted_at};
use crate::envelope::{Entity, Key};
use kinige_domain::sync::visibility::PURGED_CHILD_GRACE_MS;
use kinige_domain::sync::{ChatState, InboxMark, Slot, Versioned, is_chat_child, message_slot};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeedSlot {
    /// In this chat, which exists and is alive.
    Chat(String),
    Inbox(InboxMark),
    /// In the chat the message names, although the device has no such chat: an orphan (only with the `show_orphans` breakage).
    Orphan(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedItem {
    pub message: String,
    pub slot: FeedSlot,
}

impl Client {
    /// The live (not in the trash) messages that are shown, and where. Hidden ones are absent.
    pub fn feed(&self) -> Vec<FeedItem> {
        let mut items = Vec::new();
        for ((entity, id), local) in &self.records {
            if *entity != Entity::Message || deleted_at(&local.v).is_some() {
                continue;
            }
            let Some(chat) = chat_id_of(&local.v) else {
                continue;
            };
            let state = self.chat_state(&chat, &local.v);
            let slot = match message_slot(state, self.reached_head) {
                Slot::InChat => FeedSlot::Chat(chat),
                Slot::Inbox(mark) => FeedSlot::Inbox(mark),
                Slot::Hidden if self.switches.show_orphans && state == ChatState::Unknown => {
                    FeedSlot::Orphan(chat)
                }
                Slot::Hidden => continue,
            };
            items.push(FeedItem {
                message: id.clone(),
                slot,
            });
        }
        items
    }

    fn chat_state(&self, chat: &str, message: &Versioned) -> ChatState {
        let key = (Entity::Chat, chat.to_string());
        if let Some(local) = self.records.get(&key) {
            return if deleted_at(&local.v).is_some() {
                ChatState::Trashed
            } else {
                ChatState::Live
            };
        }
        if let Some(tombstone) = self.tombstones.get(&key) {
            let placed = message
                .get("placement")
                .map(|g| g.hlc.as_str())
                .unwrap_or_default();
            return ChatState::Purged {
                message_is_child: is_chat_child(placed, &tombstone.purged_of),
                recent: self.switches.no_child_grace
                    || self.now_ms.saturating_sub(tombstone.purged_at) < PURGED_CHILD_GRACE_MS,
            };
        }
        ChatState::Unknown
    }

    /// The records of the live tables with their groups: what must be equal on every device after quiescence. Tombstones and
    /// quarantine are per-device decisions and are not part of it.
    pub fn state(&self) -> BTreeMap<Key, Versioned> {
        self.records
            .iter()
            .map(|(key, local)| (key.clone(), local.v.clone()))
            .collect()
    }
}
