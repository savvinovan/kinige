//! Trash (slice 3, PRD §2.1, §4.1, §16; ADR-0001 §3.5): the list, "Delete forever" and auto-deletion after 30 days.
//!
//! Physical deletion (purge) is local while there is no account: the entry and its revisions leave the tables, and
//! `tombstone` keeps traces with a `purged_of` snapshot -- the HLC of **all** groups, including unknown ones. For
//! revisions `cascade_of` is the entry id and `held` is `NULL`. The traces are kept until the first binding to an account
//! and for at least 180 days; in slices 1-10 nobody erases them. The trash does not get into the Markdown copy.
//!
//! Purge reads only the columns `hlc_json` and `deleted_at`, not `groups_raw`: an entry with damaged group bytes can still
//! be deleted forever (what is readable gets into the snapshot). An entry whose `deleted` group has an unsupported `sv`
//! is deleted neither manually nor automatically (ADR-0001 §4 item 4: `purged` is created only if `deleted` is supported).

use crate::error::{read_err, write_err};
use crate::view::{VIEW_COLUMNS, VIEW_LEN, view_from_row_at};
use crate::{Now, Store, StoreError};
use kinige_domain::TrashItem;
use kinige_domain::groups::INBOX_ID;
use kinige_domain::history::{
    DAY_MS, TRASH_DAYS, days_left, deleted_group_supported, purged_of_best_effort,
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior};

/// How many entries are deleted in one transaction during auto-deletion.
const PURGE_BATCH: i64 = 200;
/// The largest number of entries in the trash list.
const TRASH_LIMIT: i64 = 1000;

/// An entry as purge sees it: without parsing the groups.
struct PurgeTarget {
    id: String,
    hlc_json: String,
    deleted_at: Option<i64>,
}

fn load_target(conn: &Connection, id: &str) -> Result<PurgeTarget, StoreError> {
    conn.query_row(
        "SELECT hlc_json, deleted_at FROM message WHERE id = ?1",
        [id],
        |row| {
            Ok(PurgeTarget {
                id: id.to_string(),
                hlc_json: row.get(0)?,
                deleted_at: row.get(1)?,
            })
        },
    )
    .optional()
    .map_err(read_err)?
    .ok_or(StoreError::MessageNotFound)
}

/// Physically deletes an entry from the trash together with its revisions and writes tombstones. The caller has opened a transaction.
fn purge(tx: &Connection, target: &PurgeTarget, now_ms: i64) -> Result<(), StoreError> {
    let message_snapshot = purged_of_best_effort(&target.hlc_json)?;
    tx.execute(
        "INSERT OR REPLACE INTO tombstone (entity, id, purged_of, purged_at, cascade_of, held) \
         VALUES ('message', ?1, ?2, ?3, NULL, NULL)",
        (&target.id, message_snapshot, now_ms),
    )
    .map_err(write_err)?;
    let revisions: Vec<(String, String)> = {
        let mut statement = tx
            .prepare("SELECT id, hlc_json FROM message_revision WHERE message_id = ?1")
            .map_err(write_err)?;
        let rows = statement
            .query_map([&target.id], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(write_err)?;
        rows.collect::<Result<_, _>>().map_err(write_err)?
    };
    for (revision_id, hlc_json) in &revisions {
        tx.execute(
            "INSERT OR REPLACE INTO tombstone (entity, id, purged_of, purged_at, cascade_of, held) \
             VALUES ('message_revision', ?1, ?2, ?3, ?4, NULL)",
            (
                revision_id,
                purged_of_best_effort(hlc_json)?,
                now_ms,
                &target.id,
            ),
        )
        .map_err(write_err)?;
    }
    tx.execute(
        "DELETE FROM message_revision WHERE message_id = ?1",
        [&target.id],
    )
    .map_err(write_err)?;
    tx.execute(
        "DELETE FROM message_tag WHERE message_id = ?1",
        [&target.id],
    )
    .map_err(write_err)?;
    crate::ext::remove(tx, &target.id).map_err(write_err)?;
    // The entry's traces in the agent access log go with it; the access row itself stays (ADR-0009).
    crate::agent_log::delete_log_messages(tx, &target.id).map_err(write_err)?;
    tx.execute("DELETE FROM message WHERE id = ?1", [&target.id])
        .map_err(write_err)?;
    // After the `message` row is deleted: a damaged index is rebuilt from the remaining entries, and the deleted one will not return to it.
    crate::search::remove(tx, &target.id).map_err(write_err)?;
    Ok(())
}

impl Store {
    /// Entries in the trash, most recently deleted first: the chat, the date, how long is left before auto-deletion.
    pub fn list_trash(&self, now: &Now) -> Result<Vec<TrashItem>, StoreError> {
        let now_ms = now.ms;
        self.read(|conn| {
            let mut statement = conn
                .prepare(&format!(
                    "SELECT {VIEW_COLUMNS}, m.deleted_at, c.title, c.icon, c.id = ?1 \
                     FROM message m LEFT JOIN chat c ON c.id = m.chat_id \
                     WHERE m.deleted_at IS NOT NULL \
                     ORDER BY m.deleted_at DESC, m.id DESC LIMIT ?2"
                ))
                .map_err(read_err)?;
            let rows = statement
                .query_map(rusqlite::params![INBOX_ID, TRASH_LIMIT], |row| {
                    let deleted_at: i64 = row.get(VIEW_LEN)?;
                    Ok(TrashItem {
                        message: view_from_row_at(row, 0)?,
                        chat_title: row
                            .get::<_, Option<String>>(VIEW_LEN + 1)?
                            .unwrap_or_default(),
                        chat_icon: row.get(VIEW_LEN + 2)?,
                        chat_is_inbox: row.get::<_, Option<bool>>(VIEW_LEN + 3)?.unwrap_or(false),
                        deleted_at,
                        days_left: days_left(deleted_at, now_ms),
                    })
                })
                .map_err(read_err)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(read_err)
        })
    }

    /// "Delete forever" for an entry in the trash: the same as auto-deletion after 30 days. No such entry --
    /// `MessageNotFound`, the entry is not in the trash -- `NotInTrash`: a live entry cannot be deleted this way. A
    /// `deleted` group newer than supported -- `ReadOnly`.
    pub fn purge_message(&self, id: &str, now: &Now) -> Result<(), StoreError> {
        let (id, now_ms) = (id.to_string(), now.ms);
        self.write(move |conn| {
            let tx = conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(write_err)?;
            let target = load_target(&tx, &id)?;
            if target.deleted_at.is_none() {
                return Err(StoreError::NotInTrash);
            }
            if !deleted_group_supported(&target.hlc_json) {
                return Err(StoreError::ReadOnly);
            }
            purge(&tx, &target, now_ms)?;
            tx.commit().map_err(write_err)
        })
    }

    /// Auto-deletion: entries that lay in the trash for 30 days are deleted physically (at start and once a day). Returns
    /// how many were deleted. A damaged entry does not stop the others: it stays in the trash, and the last error is
    /// returned after the rest are processed. Entries with a `deleted` group in an unsupported `sv` are skipped silently.
    pub fn purge_expired(&self, now: &Now) -> Result<usize, StoreError> {
        match self.purge_expired_report(now) {
            (purged, None) => Ok(purged),
            (_, Some(error)) => Err(error),
        }
    }

    /// The same, but the result is partial and the error does not interrupt the caller: how many were deleted and the last error (if any).
    pub(crate) fn purge_expired_report(&self, now: &Now) -> (usize, Option<StoreError>) {
        let border = now.ms.saturating_sub(TRASH_DAYS * DAY_MS);
        let now_ms = now.ms;
        let mut purged = 0usize;
        let mut skip: Vec<String> = Vec::new();
        let mut last_error = None;
        loop {
            let skipped = skip.clone();
            let outcome = self.write(move |conn| {
                let mut tx = conn
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .map_err(write_err)?;
                let ids: Vec<String> = {
                    let mut statement = tx
                        .prepare(
                            "SELECT id FROM message WHERE deleted_at IS NOT NULL AND deleted_at <= ?1 \
                             ORDER BY deleted_at, id LIMIT ?2",
                        )
                        .map_err(write_err)?;
                    let rows = statement
                        .query_map(
                            rusqlite::params![border, PURGE_BATCH + skipped.len() as i64],
                            |row| row.get::<_, String>(0),
                        )
                        .map_err(write_err)?;
                    rows.collect::<Result<Vec<_>, _>>().map_err(write_err)?
                };
                let mut done = 0usize;
                let mut bad = Vec::new();
                let mut error = None;
                for id in ids.into_iter().filter(|id| !skipped.contains(id)) {
                    // A savepoint per entry: a damaged one is rolled back alone, the already deleted ones stay.
                    let step = tx.savepoint().map_err(write_err)?;
                    let result = load_target(&step, &id).and_then(|target| {
                        if deleted_group_supported(&target.hlc_json) {
                            purge(&step, &target, now_ms).map(|()| true)
                        } else {
                            Ok(false)
                        }
                    });
                    match result {
                        Ok(true) => {
                            step.commit().map_err(write_err)?;
                            done += 1;
                        }
                        Ok(false) => {
                            drop(step);
                            bad.push(id);
                        }
                        Err(e) => {
                            drop(step);
                            bad.push(id);
                            error = Some(e);
                        }
                    }
                }
                tx.commit().map_err(write_err)?;
                Ok((done, bad, error))
            });
            let (done, bad, error) = match outcome {
                Ok(found) => found,
                Err(error) => {
                    last_error = Some(error);
                    break;
                }
            };
            purged += done;
            // Skipped entries (an unsupported group, damaged JSON) do not block the healthy ones behind them: the
            // selection in the next pass is longer by the number of skipped ones, the loop ends only when no new entries are left.
            let seen_new = done + bad.len();
            skip.extend(bad);
            if error.is_some() {
                last_error = error;
            }
            if seen_new == 0 {
                break;
            }
        }
        (purged, last_error)
    }
}
