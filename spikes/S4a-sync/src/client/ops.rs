//! What the user does on one device. Every change is a rewrite of one field group with a new HLC through the real
//! `kinige-domain` functions (slices 1-3), then an `upsert` into the outbox (once the device is bound).

use super::{Client, Event, Local, Write};
use crate::envelope::{Entity, Key};
use kinige_domain::append::{CurrentBody, rewrite_body};
use kinige_domain::chat::{NewChat, new_chat, set_meta};
use kinige_domain::groups::{GroupError, INBOX_ID, NewMessage, inbox_chat, new_message};
use kinige_domain::message_ops::{FlagChange, change_flag, move_to_chat, set_deleted};
use kinige_domain::rewrite::{Groups, Rewritten};
use kinige_domain::sync::{Group, Versioned, next_vv, with_vv};
use serde_json::Value;

pub const JOURNAL_DATE: &str = "2026-03-14";

pub fn inbox_key() -> Key {
    (Entity::Chat, INBOX_ID.to_string())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpError {
    NotFound,
    /// The record has a group from the future; this client may not write it (§4 item 4).
    ReadOnly,
    /// The new HLC was not above the old one: the clock had not seen the group it is rewriting.
    HlcNotAbove,
    Other(String),
}

impl Client {
    pub(crate) fn create_inbox(&mut self, now_ms: u64) {
        let Ok(hlc) = self.stamp(now_ms) else { return };
        let Ok(row) = inbox_chat(&hlc, i64::try_from(now_ms).unwrap_or(0)) else {
            return;
        };
        let Ok(v) = Versioned::from_columns(&row.groups_raw, &row.hlc_json) else {
            return;
        };
        let key = inbox_key();
        self.records.insert(
            key.clone(),
            Local {
                v,
                server_known: false,
            },
        );
        self.note_created(&key);
        self.enqueue_upsert(&key);
    }

    pub fn create_chat(&mut self, now_ms: u64) -> Option<Key> {
        let id = self.next_id();
        let hlc = self.stamp(now_ms).ok()?;
        let title = format!("chat {id}");
        let row = new_chat(
            &NewChat {
                id: &id,
                title: &title,
                icon: "x",
                sort_key: "a1",
                created_at: i64::try_from(now_ms).unwrap_or(0),
            },
            &hlc,
        )
        .ok()?;
        let v = Versioned::from_columns(&row.groups_raw, &row.hlc_json).ok()?;
        let key = (Entity::Chat, id);
        self.records.insert(
            key.clone(),
            Local {
                v,
                server_known: false,
            },
        );
        self.note_created(&key);
        self.enqueue_upsert(&key);
        Some(key)
    }

    pub fn create_message(&mut self, chat: &Key, now_ms: u64) -> Option<Key> {
        let body = self.next_text();
        self.create_message_with(chat, &body, JOURNAL_DATE, now_ms)
    }

    /// A new message with the given text and day: what an import does (each record its own HLC, slice 4).
    pub fn create_message_with(
        &mut self,
        chat: &Key,
        body: &str,
        journal_date: &str,
        now_ms: u64,
    ) -> Option<Key> {
        let id = self.next_id();
        let hlc = self.stamp(now_ms).ok()?;
        let row = new_message(
            &NewMessage {
                id: &id,
                chat_id: &chat.1,
                body_md: body,
                created_at: i64::try_from(now_ms).unwrap_or(0),
                created_tz: "UTC",
                journal_date,
            },
            &hlc,
        )
        .ok()?;
        let mut v = Versioned::from_columns(&row.groups_raw, &row.hlc_json).ok()?;
        for group in ["body", "placement"] {
            self.add_vv(&mut v, group, None);
        }
        let key = (Entity::Message, id);
        self.records.insert(
            key.clone(),
            Local {
                v,
                server_known: false,
            },
        );
        self.note_created(&key);
        self.enqueue_upsert(&key);
        Some(key)
    }

    pub fn rename_chat(&mut self, key: &Key, now_ms: u64) -> Result<(), OpError> {
        let title = self.next_text();
        self.rewrite(key, "meta", now_ms, |groups, hlc, _| {
            set_meta(groups, Some(&title), None, hlc)
        })
    }

    pub fn edit_body(&mut self, key: &Key, now_ms: u64) -> Result<(), OpError> {
        let text = self.next_text();
        let edited_at = i64::try_from(now_ms).unwrap_or(0);
        self.rewrite(key, "body", now_ms, |groups, hlc, confirmed| {
            let rewritten = rewrite_body(
                &CurrentBody {
                    groups_raw: groups.groups_raw,
                    hlc_json: groups.hlc_json,
                    body_confirmed: confirmed,
                },
                &text,
                edited_at,
                hlc,
            )?;
            Ok(Rewritten {
                groups_raw: rewritten.groups_raw,
                hlc_json: rewritten.hlc_json,
                schema_version: rewritten.schema_version,
            })
        })
    }

    pub fn toggle_favorite(&mut self, key: &Key, now_ms: u64) -> Result<(), OpError> {
        let on = self
            .records
            .get(key)
            .and_then(|l| l.v.get("flags"))
            .and_then(|g| serde_json::from_str::<Value>(&g.raw).ok())
            .is_some_and(|v| v.get("favorite_at").is_none_or(Value::is_null));
        let at = i64::try_from(now_ms).unwrap_or(0);
        self.rewrite(key, "flags", now_ms, |groups, hlc, _| {
            change_flag(groups, FlagChange::Favorite(on), at, hlc)
        })
    }

    pub fn move_message(&mut self, key: &Key, chat: &Key, now_ms: u64) -> Result<(), OpError> {
        self.rewrite(key, "placement", now_ms, |groups, hlc, confirmed| {
            move_to_chat(groups, &chat.1, confirmed, hlc)
        })
    }

    /// Delete (`true`) or restore (`false`): the `deleted` group with a new HLC (§3.5). Inbox is never deleted.
    pub fn set_deleted(&mut self, key: &Key, deleted: bool, now_ms: u64) -> Result<(), OpError> {
        if *key == inbox_key() {
            return Err(OpError::Other("Inbox".into()));
        }
        // The interface cannot delete what is in the trash or restore what is not: no write, no new HLC.
        let in_trash = self
            .records
            .get(key)
            .is_some_and(|l| super::purge::deleted_at(&l.v).is_some());
        if self.records.contains_key(key) && in_trash == deleted {
            return Ok(());
        }
        let at = deleted.then(|| i64::try_from(now_ms).unwrap_or(0));
        self.rewrite(key, "deleted", now_ms, |groups, hlc, _| {
            set_deleted(groups, at, hlc)
        })
    }

    /// Rewrites one group of a record with a new HLC and queues the record.
    fn rewrite(
        &mut self,
        key: &Key,
        group: &str,
        now_ms: u64,
        edit: impl FnOnce(&Groups<'_>, &str, bool) -> Result<Rewritten, GroupError>,
    ) -> Result<(), OpError> {
        let confirmed = self.confirmed(key, group);
        let local = self.records.get(key).ok_or(OpError::NotFound)?;
        if local.v.is_read_only() {
            return Err(OpError::ReadOnly);
        }
        let parent = local.v.get(group).map(|g| g.hlc.clone());
        let (groups_raw, hlc_json) = local
            .v
            .to_columns()
            .map_err(|e| OpError::Other(e.to_string()))?;
        let hlc = self
            .stamp(now_ms)
            .map_err(|e| OpError::Other(e.to_string()))?;
        let rewritten = match edit(
            &Groups {
                groups_raw: &groups_raw,
                hlc_json: &hlc_json,
            },
            &hlc,
            confirmed,
        ) {
            Ok(rewritten) => rewritten,
            Err(GroupError::HlcNotNewer) => {
                self.events.push(Event::HlcNotAbove(group.to_string()));
                return Err(OpError::HlcNotAbove);
            }
            Err(other) => return Err(OpError::Other(other.to_string())),
        };
        let mut v = Versioned::from_columns(&rewritten.groups_raw, &rewritten.hlc_json)
            .map_err(|e| OpError::Other(e.to_string()))?;
        let parent_group = self.records.get(key).and_then(|l| l.v.get(group)).cloned();
        self.add_vv(&mut v, group, parent_group.as_ref());
        if let Some(local) = self.records.get_mut(key) {
            local.v = v;
        }
        self.note_write(key, group, parent);
        self.enqueue_upsert(key);
        Ok(())
    }

    /// Adds `vv` (ADR-0001 §3.4, S4a) to a freshly written `body` / `placement` group, computed from the version it was made from.
    /// The app's own rewrite functions do not write it yet (slice 11 will); the simulation adds it right after them.
    fn add_vv(&self, v: &mut Versioned, group: &str, parent: Option<&Group>) {
        if self.switches.no_vv || !matches!(group, "body" | "placement") {
            return;
        }
        let Some(new) = v.get(group).cloned() else {
            return;
        };
        let Ok(raw) = with_vv(&new.raw, &next_vv(parent, &new.hlc)) else {
            return;
        };
        if let Ok(group_value) = Group::new(&new.hlc, new.sv, &raw) {
            v.set(group, group_value);
        }
    }

    fn note_created(&mut self, key: &Key) {
        let names: Vec<String> = self
            .records
            .get(key)
            .map(|l| l.v.groups().map(|(n, _)| n.to_string()).collect())
            .unwrap_or_default();
        for name in names {
            self.note_write(key, &name, None);
        }
    }

    fn note_write(&mut self, key: &Key, group: &str, parent: Option<String>) {
        let Some(group_value) = self.records.get(key).and_then(|l| l.v.get(group)) else {
            return;
        };
        self.writes.push(Write {
            device: self.idx,
            key: key.clone(),
            group: group.to_string(),
            hlc: group_value.hlc.clone(),
            parent,
            raw: group_value.raw.clone(),
        });
    }

    /// Unique content for every write, so two versions never look equal by accident.
    fn next_text(&mut self) -> String {
        self.counter += 1;
        format!("t{}-{}", self.idx, self.counter)
    }
}
