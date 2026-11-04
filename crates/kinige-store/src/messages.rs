//! Entry actions (slice 3): delete and restore, text edit (with an `edit` revision), flags, the entry day, a task
//! item. Each is one transaction: a rewrite of one group (`kinige_domain::rewrite`), HLC, columns from the new group bytes,
//! tags, the revision and the Markdown copy mark. No optimistic display: on error nothing is written.

use crate::attachment::live_count;
use crate::error::{read_err, write_err};
use crate::hlc_state::next_hlc;
use crate::message_body::edit_body;
use crate::send::mark_dirty;
use crate::view::load_view;
use crate::{Now, Store, StoreError};
use kinige_domain::groups::normalize_body_or_empty;
use kinige_domain::journal::is_valid_journal_date;
use kinige_domain::message_ops::{self, FlagChange, MessageColumns, message_columns};
use kinige_domain::rewrite::{Groups, Rewritten, is_read_only};
use kinige_domain::tasks::{TaskItemError, set_task_item};
use kinige_domain::{MessageFlag, MessageView};
use rusqlite::{Connection, Transaction, TransactionBehavior};

/// An entry as the rewrite sees it: the group bytes and the "confirmed by the server" flag.
pub(crate) struct Loaded {
    pub(crate) id: String,
    pub(crate) hlc_json: String,
    pub(crate) groups_raw: String,
    pub(crate) server_known: bool,
    pub(crate) columns: MessageColumns,
}

/// An entry by id; no entry -- `MessageNotFound`, damaged groups -- `Internal`.
pub(crate) fn load(conn: &Connection, id: &str) -> Result<Loaded, StoreError> {
    let (hlc_json, groups_raw, server_known): (String, String, i64) = match conn.query_row(
        "SELECT hlc_json, groups_raw, server_known FROM message WHERE id = ?1",
        [id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ) {
        Ok(found) => found,
        Err(rusqlite::Error::QueryReturnedNoRows) => return Err(StoreError::MessageNotFound),
        Err(error) => return Err(read_err(error)),
    };
    let columns = message_columns(&groups_raw)?;
    Ok(Loaded {
        id: id.to_string(),
        hlc_json,
        groups_raw,
        server_known: server_known != 0,
        columns,
    })
}

impl Loaded {
    pub(crate) fn groups(&self) -> Groups<'_> {
        Groups {
            groups_raw: &self.groups_raw,
            hlc_json: &self.hlc_json,
        }
    }

    /// An entry with a group in an unsupported `sv` is read-only (ADR-0001 §4 item 4).
    pub(crate) fn ensure_writable(&self) -> Result<(), StoreError> {
        if is_read_only(&self.hlc_json)? {
            return Err(StoreError::ReadOnly);
        }
        Ok(())
    }
}

/// Writes the new group bytes and all the columns built from them. Immutable columns (`created_*`, `origin`,
/// `ext_json`, `reply_to_id`) are not touched: their `core` group is not rewritten.
pub(crate) fn apply(
    tx: &Transaction<'_>,
    id: &str,
    rewritten: &Rewritten,
) -> Result<MessageColumns, StoreError> {
    let columns = message_columns(&rewritten.groups_raw)?;
    let changed = tx
        .execute(
            "UPDATE message SET chat_id = ?1, body_md = ?2, journal_date = ?3, edited_at = ?4, is_task = ?5, \
             task_done_at = ?6, favorite_at = ?7, pinned_at = ?8, deleted_at = ?9, body_base = ?10, \
             body_supersedes = ?11, placement_base = ?12, placement_supersedes = ?13, hlc_json = ?14, \
             schema_version = ?15, groups_raw = ?16 WHERE id = ?17",
            rusqlite::params![
                columns.chat_id,
                columns.body_md,
                columns.journal_date,
                columns.edited_at,
                columns.is_task,
                columns.task_done_at,
                columns.favorite_at,
                columns.pinned_at,
                columns.deleted_at,
                columns.body_base,
                columns.body_supersedes,
                columns.placement_base,
                columns.placement_supersedes,
                rewritten.hlc_json,
                rewritten.schema_version,
                rewritten.groups_raw,
                id
            ],
        )
        .map_err(write_err)?;
    if changed != 1 {
        return Err(StoreError::Internal(
            "the entry to change disappeared".into(),
        ));
    }
    Ok(columns)
}

fn begin(conn: &mut Connection) -> Result<Transaction<'_>, StoreError> {
    conn.transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(write_err)
}

impl Store {
    /// The chat of a live entry (not in the trash) -- to open it in the feed from a window that has no feed itself. If there is no entry, it
    /// was deleted for good or is in the trash -- `MessageNotFound`. The entry text is not read.
    pub fn live_message_chat(&self, id: &str) -> Result<String, StoreError> {
        self.read(|conn| {
            match conn.query_row(
                "SELECT chat_id FROM message WHERE id = ?1 AND deleted_at IS NULL",
                [id],
                |row| row.get(0),
            ) {
                Ok(chat_id) => Ok(chat_id),
                Err(rusqlite::Error::QueryReturnedNoRows) => Err(StoreError::MessageNotFound),
                Err(error) => Err(read_err(error)),
            }
        })
    }

    /// "Delete": the entry goes to the trash (the `deleted` group), the chat counter decreases, it disappears from the Markdown copy.
    /// Deleting again changes nothing.
    pub fn delete_message(&self, id: &str, now: &Now) -> Result<(), StoreError> {
        self.set_deleted(id, now, true)
    }

    /// "Restore": the entry returns to its place (the chat and the day did not change).
    pub fn restore_message(&self, id: &str, now: &Now) -> Result<(), StoreError> {
        self.set_deleted(id, now, false)
    }

    fn set_deleted(&self, id: &str, now: &Now, deleted: bool) -> Result<(), StoreError> {
        let (id, now) = (id.to_string(), now.clone());
        self.write(move |conn| {
            let tx = begin(conn)?;
            let loaded = load(&tx, &id)?;
            if loaded.columns.deleted_at.is_some() == deleted {
                return Ok(());
            }
            loaded.ensure_writable()?;
            let hlc = next_hlc(&tx, now.ms)?;
            let rewritten =
                message_ops::set_deleted(&loaded.groups(), deleted.then_some(now.ms), &hlc)?;
            apply(&tx, &id, &rewritten)?;
            mark_dirty(&tx, &loaded.columns.chat_id, &loaded.columns.journal_date)
                .map_err(write_err)?;
            tx.commit().map_err(write_err)
        })?;
        self.wake_mirror();
        Ok(())
    }

    /// Text edit: the old version is kept as an `edit` revision in the same transaction; the entry stays in place, with the same
    /// time and day. Empty text -- `Empty`, nothing changes. Text equal to the previous one writes nothing.
    pub fn edit_message(&self, id: &str, body: &str, now: &Now) -> Result<MessageView, StoreError> {
        // Empty text is valid exactly when the entry has a live attachment (appendix §C.6); checked below, in the transaction.
        let body = normalize_body_or_empty(body)?;
        let (id, now) = (id.to_string(), now.clone());
        let view = self.write(move |conn| {
            let tx = begin(conn)?;
            let loaded = load(&tx, &id)?;
            loaded.ensure_writable()?;
            if body.is_empty() && live_count(&tx, &id)? == 0 {
                return Err(StoreError::Empty);
            }
            edit_body(&tx, &loaded, &body, &now)?;
            let view = load_view(&tx, &id)?;
            tx.commit().map_err(write_err)?;
            Ok(view)
        })?;
        self.wake_mirror();
        Ok(view)
    }

    /// A click on `- [ ]` in the feed: item `index` (zero-based) is set to the `checked` state. This is a text edit -- the `edit`
    /// revision is written the same way. No such item or it is already in the needed state -- `TaskItemStale`: the window showed stale text.
    pub fn toggle_task_item(
        &self,
        id: &str,
        index: usize,
        checked: bool,
        now: &Now,
    ) -> Result<MessageView, StoreError> {
        let (id, now) = (id.to_string(), now.clone());
        let view = self.write(move |conn| {
            let tx = begin(conn)?;
            let loaded = load(&tx, &id)?;
            loaded.ensure_writable()?;
            let body =
                set_task_item(&loaded.columns.body_md, index, checked).map_err(
                    |error| match error {
                        TaskItemError::NotFound | TaskItemError::AlreadyThere => {
                            StoreError::TaskItemStale
                        }
                    },
                )?;
            edit_body(&tx, &loaded, &body, &now)?;
            let view = load_view(&tx, &id)?;
            tx.commit().map_err(write_err)?;
            Ok(view)
        })?;
        self.wake_mirror();
        Ok(view)
    }

    /// Entry flags: task, done, favorite. If the entry is already in the needed state, nothing is written.
    /// "Done" on an entry that is not a task is an internal error (the interface has no such button).
    pub fn set_message_flag(
        &self,
        id: &str,
        flag: MessageFlag,
        on: bool,
        now: &Now,
    ) -> Result<MessageView, StoreError> {
        let (id, now) = (id.to_string(), now.clone());
        self.write(move |conn| {
            let tx = begin(conn)?;
            let loaded = load(&tx, &id)?;
            loaded.ensure_writable()?;
            let columns = &loaded.columns;
            let already = match flag {
                MessageFlag::Task => columns.is_task == on,
                // There is nothing to clear "done" on a non-task; setting it is an error (`change_flag`).
                MessageFlag::TaskDone if columns.is_task => columns.task_done_at.is_some() == on,
                MessageFlag::TaskDone => !on,
                MessageFlag::Favorite => columns.favorite_at.is_some() == on,
            };
            if !already {
                let change = match flag {
                    MessageFlag::Task => FlagChange::Task(on),
                    MessageFlag::TaskDone => FlagChange::TaskDone(on),
                    MessageFlag::Favorite => FlagChange::Favorite(on),
                };
                let hlc = next_hlc(&tx, now.ms)?;
                let rewritten = message_ops::change_flag(&loaded.groups(), change, now.ms, &hlc)?;
                apply(&tx, &id, &rewritten)?;
            }
            let view = load_view(&tx, &id)?;
            tx.commit().map_err(write_err)?;
            Ok(view)
        })
    }

    /// "Change date": the entry moves to another journal day (the `placement` group; the first rewrite writes
    /// `base`/`supersedes` per ADR-0001 §3.4). Future dates are allowed. The files of the old and the new day in the
    /// Markdown copy are rewritten.
    pub fn set_message_date(
        &self,
        id: &str,
        journal_date: &str,
        now: &Now,
    ) -> Result<MessageView, StoreError> {
        if !is_valid_journal_date(journal_date) {
            return Err(StoreError::InvalidDate);
        }
        let (id, journal_date, now) = (id.to_string(), journal_date.to_string(), now.clone());
        let view = self.write(move |conn| {
            let tx = begin(conn)?;
            let loaded = load(&tx, &id)?;
            loaded.ensure_writable()?;
            if loaded.columns.journal_date != journal_date {
                let hlc = next_hlc(&tx, now.ms)?;
                let rewritten = message_ops::set_journal_date(
                    &loaded.groups(),
                    &journal_date,
                    // The outbox appears in slice 11: then "confirmed" is both a column and an outbox row (ADR-0001 §3.4).
                    loaded.server_known,
                    &hlc,
                )?;
                apply(&tx, &id, &rewritten)?;
                for day in [&loaded.columns.journal_date, &journal_date] {
                    mark_dirty(&tx, &loaded.columns.chat_id, day).map_err(write_err)?;
                }
            }
            let view = load_view(&tx, &id)?;
            tx.commit().map_err(write_err)?;
            Ok(view)
        })?;
        self.wake_mirror();
        Ok(view)
    }

    /// "Move to chat": the entry moves to another chat (the `placement` group, `chat_id`; `journal_date` does not change;
    /// the first rewrite writes `base`/`supersedes` per ADR-0001 §3.4). No `edit` revision is written: the text did not change. The day
    /// of the entry in the old and in the new chat is marked for rewriting in the Markdown copy. The target chat may be archived;
    /// no such chat -- `ChatNotFound`; an entry in the trash -- `MessageInTrash` (moving is only for live entries); an entry with a
    /// group in an unsupported `sv` -- `ReadOnly`. The same chat -- nothing is written.
    pub fn move_message(
        &self,
        id: &str,
        chat_id: &str,
        now: &Now,
    ) -> Result<MessageView, StoreError> {
        let (id, chat_id, now) = (id.to_string(), chat_id.to_string(), now.clone());
        let view = self.write(move |conn| {
            let tx = begin(conn)?;
            let loaded = load(&tx, &id)?;
            loaded.ensure_writable()?;
            if loaded.columns.deleted_at.is_some() {
                return Err(StoreError::MessageInTrash);
            }
            let chat_exists: bool = tx
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM chat WHERE id = ?1 AND deleted_at IS NULL)",
                    [&chat_id],
                    |row| row.get(0),
                )
                .map_err(write_err)?;
            if !chat_exists {
                return Err(StoreError::ChatNotFound);
            }
            if loaded.columns.chat_id != chat_id {
                let hlc = next_hlc(&tx, now.ms)?;
                let rewritten = message_ops::move_to_chat(
                    &loaded.groups(),
                    &chat_id,
                    // The outbox appears in slice 11: then "confirmed" is both a column and an outbox row (ADR-0001 §3.4).
                    loaded.server_known,
                    &hlc,
                )?;
                apply(&tx, &id, &rewritten)?;
                let day = &loaded.columns.journal_date;
                mark_dirty(&tx, &loaded.columns.chat_id, day).map_err(write_err)?;
                mark_dirty(&tx, &chat_id, day).map_err(write_err)?;
            }
            let view = load_view(&tx, &id)?;
            tx.commit().map_err(write_err)?;
            Ok(view)
        })?;
        self.wake_mirror();
        Ok(view)
    }
}
