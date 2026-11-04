//! Chats (slice 3): creating, naming, emoji, pinning and archiving (the colour is in `chat_color.rs`). Each action is
//! a rewrite of one chat group (`meta` or `state`) or a creation with all six groups under one HLC.
//! There is no chat deletion (PRD §2.1: archive). Inbox cannot be renamed, given another emoji or archived.

use crate::error::{read_err, write_err};
use crate::feed::chat_list;
use crate::hlc_state::next_hlc;
use crate::send::insert_chat;
use crate::{Now, Store, StoreError};
use kinige_domain::ChatSummary;
use kinige_domain::chat::{
    ChatColumns, NewChat, StateChange, chat_columns, is_inbox, key_after, new_chat, normalize_icon,
    normalize_title, set_meta, set_state,
};
use kinige_domain::folders::{FolderChat, chat_folders};
use kinige_domain::groups::INBOX_ID;
use kinige_domain::rewrite::{Groups, Rewritten, is_read_only};
use rusqlite::{Connection, Transaction, TransactionBehavior};
use std::collections::BTreeMap;
use uuid::Uuid;

pub(crate) struct LoadedChat {
    hlc_json: String,
    groups_raw: String,
    pub(crate) columns: ChatColumns,
}

pub(crate) fn load_chat(conn: &Connection, id: &str) -> Result<LoadedChat, StoreError> {
    let (hlc_json, groups_raw): (String, String) = match conn.query_row(
        "SELECT hlc_json, groups_raw FROM chat WHERE id = ?1 AND deleted_at IS NULL",
        [id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ) {
        Ok(found) => found,
        Err(rusqlite::Error::QueryReturnedNoRows) => return Err(StoreError::ChatNotFound),
        Err(error) => return Err(read_err(error)),
    };
    let columns = chat_columns(&groups_raw)?;
    Ok(LoadedChat {
        hlc_json,
        groups_raw,
        columns,
    })
}

impl LoadedChat {
    pub(crate) fn groups(&self) -> Groups<'_> {
        Groups {
            groups_raw: &self.groups_raw,
            hlc_json: &self.hlc_json,
        }
    }

    pub(crate) fn ensure_writable(&self) -> Result<(), StoreError> {
        if is_read_only(&self.hlc_json)? {
            return Err(StoreError::ReadOnly);
        }
        Ok(())
    }
}

pub(crate) fn apply_chat(
    tx: &Transaction<'_>,
    id: &str,
    rewritten: &Rewritten,
    now_ms: i64,
) -> Result<(), StoreError> {
    let columns = chat_columns(&rewritten.groups_raw)?;
    let changed = tx
        .execute(
            "UPDATE chat SET title = ?1, icon = ?2, color = ?3, sort_key = ?4, pinned_at = ?5, archived_at = ?6, \
             ai_excluded = ?7, deleted_at = ?8, updated_at = ?9, hlc_json = ?10, schema_version = ?11, \
             groups_raw = ?12 WHERE id = ?13",
            rusqlite::params![
                columns.title,
                columns.icon,
                columns.color,
                columns.sort_key,
                columns.pinned_at,
                columns.archived_at,
                columns.ai_excluded,
                columns.deleted_at,
                now_ms,
                rewritten.hlc_json,
                rewritten.schema_version,
                rewritten.groups_raw,
                id
            ],
        )
        .map_err(write_err)?;
    if changed != 1 {
        return Err(StoreError::ChatNotFound);
    }
    Ok(())
}

/// The folder of each chat in the Markdown copy by the current titles (chats without the deleted ones).
pub(crate) fn folder_map(conn: &Connection) -> Result<BTreeMap<String, String>, StoreError> {
    let mut statement = conn
        .prepare("SELECT id, title, created_at FROM chat WHERE deleted_at IS NULL")
        .map_err(read_err)?;
    let chats: Vec<(String, String, i64)> = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .map_err(read_err)?
        .collect::<Result<_, _>>()
        .map_err(read_err)?;
    let input: Vec<FolderChat<'_>> = chats
        .iter()
        .map(|(id, title, created_at)| FolderChat {
            id,
            title,
            created_at: *created_at,
        })
        .collect();
    Ok(chat_folders(&input))
}

/// All days of the chat with entries (except the trash) are marked for rewriting: the chat folder changed.
fn mark_chat_dirty(tx: &Transaction<'_>, chat_id: &str) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT INTO mirror_dirty (chat_id, journal_date, seq) \
         SELECT DISTINCT chat_id, journal_date, (SELECT COALESCE(MAX(seq), 0) + 1 FROM mirror_dirty) \
         FROM message WHERE chat_id = ?1 AND deleted_at IS NULL \
         ON CONFLICT(chat_id, journal_date) DO UPDATE SET seq = excluded.seq",
        [chat_id],
    )?;
    Ok(())
}

/// Chats whose copy folder changed after the edit (the chat itself and those that shared its name) are marked for rewriting.
fn mark_folder_changes(
    tx: &Transaction<'_>,
    before: &BTreeMap<String, String>,
    after: &BTreeMap<String, String>,
) -> Result<(), StoreError> {
    for (chat_id, folder) in after {
        if before.get(chat_id) != Some(folder) {
            mark_chat_dirty(tx, chat_id).map_err(write_err)?;
        }
    }
    Ok(())
}

pub(crate) fn begin(conn: &mut Connection) -> Result<Transaction<'_>, StoreError> {
    conn.transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(write_err)
}

pub(crate) fn summary(conn: &Connection, id: &str) -> Result<ChatSummary, StoreError> {
    chat_list(conn)?
        .chats
        .into_iter()
        .find(|chat| chat.id == id)
        .ok_or(StoreError::ChatNotFound)
}

/// A new chat at the end of the section, with all six groups under one HLC, in the caller's open transaction. `title` and `icon`
/// are already normalised (`normalize_title`, `normalize_icon`). The "+" button and the import of a personal chat share this path.
pub(crate) fn insert_new_chat(
    tx: &Transaction<'_>,
    title: &str,
    icon: &str,
    now_ms: i64,
) -> Result<String, StoreError> {
    let before = folder_map(tx)?;
    let max_key: Option<String> = tx
        .query_row("SELECT MAX(sort_key) FROM chat", [], |row| row.get(0))
        .map_err(read_err)?;
    let id = Uuid::new_v4().to_string();
    let sort_key = key_after(max_key.as_deref());
    let hlc = next_hlc(tx, now_ms)?;
    let chat = new_chat(
        &NewChat {
            id: &id,
            title,
            icon,
            sort_key: &sort_key,
            created_at: now_ms,
        },
        &hlc,
    )?;
    insert_chat(tx, &chat).map_err(write_err)?;
    // A new chat with the name of an existing one takes the suffix, and other folders do not change; but in case
    // the owner of the plain name changes, we compare anyway.
    mark_folder_changes(tx, &before, &folder_map(tx)?)?;
    Ok(id)
}

impl Store {
    /// The "+" of the Chats section: a new chat at the end of the section. The name is 1 to 128 characters, the emoji comes from the picker, without an emoji it is 💬.
    pub fn create_chat(
        &self,
        title: &str,
        icon: Option<&str>,
        now: &Now,
    ) -> Result<ChatSummary, StoreError> {
        let title = normalize_title(title)?;
        let icon = normalize_icon(icon)?;
        let now = now.clone();
        let summary = self.write(move |conn| {
            let tx = begin(conn)?;
            let id = insert_new_chat(&tx, &title, &icon, now.ms)?;
            let summary = summary(&tx, &id)?;
            tx.commit().map_err(write_err)?;
            Ok(summary)
        })?;
        self.wake_mirror();
        Ok(summary)
    }

    /// "Rename". Inbox is not renamed. The chat folder in the Markdown copy moves, all its days are rewritten.
    pub fn rename_chat(&self, id: &str, title: &str, now: &Now) -> Result<ChatSummary, StoreError> {
        let title = normalize_title(title)?;
        let result = self.change_meta(id, now, Some(title), None);
        self.wake_mirror();
        result
    }

    /// "Change emoji". Inbox does not change its emoji.
    pub fn set_chat_icon(
        &self,
        id: &str,
        icon: &str,
        now: &Now,
    ) -> Result<ChatSummary, StoreError> {
        let icon = normalize_icon(Some(icon))?;
        self.change_meta(id, now, None, Some(icon))
    }

    fn change_meta(
        &self,
        id: &str,
        now: &Now,
        title: Option<String>,
        icon: Option<String>,
    ) -> Result<ChatSummary, StoreError> {
        let (id, now) = (id.to_string(), now.clone());
        self.write(move |conn| {
            if is_inbox(&id) {
                return Err(StoreError::InboxProtected);
            }
            let tx = begin(conn)?;
            let loaded = load_chat(&tx, &id)?;
            loaded.ensure_writable()?;
            let changed = title.as_deref().is_some_and(|t| t != loaded.columns.title)
                || icon
                    .as_deref()
                    .is_some_and(|i| Some(i) != loaded.columns.icon.as_deref());
            if changed {
                let before = folder_map(&tx)?;
                let hlc = next_hlc(&tx, now.ms)?;
                let rewritten =
                    set_meta(&loaded.groups(), title.as_deref(), icon.as_deref(), &hlc)?;
                apply_chat(&tx, &id, &rewritten, now.ms)?;
                mark_folder_changes(&tx, &before, &folder_map(&tx)?)?;
            }
            let summary = summary(&tx, &id)?;
            tx.commit().map_err(write_err)?;
            Ok(summary)
        })
    }

    /// "Pin" / "Unpin". Inbox can be pinned too.
    pub fn set_chat_pinned(
        &self,
        id: &str,
        pinned: bool,
        now: &Now,
    ) -> Result<ChatSummary, StoreError> {
        self.change_state(id, now, StateKind::Pinned, pinned)
    }

    /// "Archive" / "Unarchive". Inbox cannot be archived.
    pub fn set_chat_archived(
        &self,
        id: &str,
        archived: bool,
        now: &Now,
    ) -> Result<ChatSummary, StoreError> {
        if archived && id == INBOX_ID {
            return Err(StoreError::InboxProtected);
        }
        self.change_state(id, now, StateKind::Archived, archived)
    }

    fn change_state(
        &self,
        id: &str,
        now: &Now,
        kind: StateKind,
        on: bool,
    ) -> Result<ChatSummary, StoreError> {
        let (id, now) = (id.to_string(), now.clone());
        self.write(move |conn| {
            let tx = begin(conn)?;
            let loaded = load_chat(&tx, &id)?;
            loaded.ensure_writable()?;
            let current = match kind {
                StateKind::Pinned => loaded.columns.pinned_at,
                StateKind::Archived => loaded.columns.archived_at,
            };
            if current.is_some() != on {
                let value = on.then_some(now.ms);
                let change = match kind {
                    StateKind::Pinned => StateChange::Pinned(value),
                    StateKind::Archived => StateChange::Archived(value),
                };
                let hlc = next_hlc(&tx, now.ms)?;
                let rewritten = set_state(&loaded.groups(), change, &hlc)?;
                apply_chat(&tx, &id, &rewritten, now.ms)?;
            }
            let summary = summary(&tx, &id)?;
            tx.commit().map_err(write_err)?;
            Ok(summary)
        })
    }
}

#[derive(Clone, Copy)]
enum StateKind {
    Pinned,
    Archived,
}
