//! Reactions (slice 5a, PRD §4.1, §4.2, ADR-0001 appendix §C.9). One group `reactions` on the message holds them
//! (`kinige_domain::reactions`); this file is the transaction around a change and the local derived rows the interface reads.
//!
//! `message_reaction` and `message_reaction_keys` are built from the group bytes by one function, [`write_rows`]: used by a write, by purge
//! (which deletes them), by migration 10 and by the rebuild on open when `app_setting.reaction_rules_version` differs from the domain's.
//! A write is not an edit: no revision, no `edited_at`, the favorite untouched. The recent list is device-local on purpose.

use crate::error::{read_err, write_err};
use crate::hlc_state::next_hlc;
use crate::messages::load;
use crate::send::mark_dirty;
use crate::settings::get_setting;
use crate::time::journal_day_in;
use crate::view::load_view;
use crate::{Now, Store, StoreError};
use kinige_domain::MessageView;
use kinige_domain::ReactionView;
use kinige_domain::emoji::is_emoji_sequence;
use kinige_domain::hlc::is_uuid_v4;
use kinige_domain::reactions::present::{parse_recent, push_recent};
use kinige_domain::reactions::write::{apply, plan};
use kinige_domain::reactions::{RULES_VERSION, Reaction, shown};
use rusqlite::{Connection, Transaction, TransactionBehavior};
use serde_json::Value;

/// `app_setting`: the rules version the derived rows were built with.
const VERSION_KEY: &str = "reaction_rules_version";
/// `app_setting`: the reactions put on this device, freshest first (JSON array). Not synced and not in `settings.json`.
pub(crate) const RECENT_KEY: &str = "recent_reactions";

/// The two columns of a view query for the table `m`: what the entry has had (`{keys, items, live}`: the emoji keys, the number of items
/// and the number of live ones), and its live reactions as an array of `{k, e, a, z}` (key, emoji, put at, zone). The order is restored in
/// Rust, because the aggregate does not promise one.
macro_rules! reactions_columns {
    () => {
        "(SELECT json_object('keys', json(k.keys), 'items', k.item_count, 'live', k.live_count) \
          FROM message_reaction_keys k WHERE k.message_id = m.id), \
         (SELECT json_group_array(json_object('k', r.key, 'e', r.emoji, 'a', r.put_at, 'z', r.put_tz)) \
          FROM message_reaction r WHERE r.message_id = m.id)"
    };
}
pub(crate) use reactions_columns;

/// What an entry has had: the emoji keys, how many items its group holds and how many are live (hidden ones included in both counts).
#[derive(Debug, Default)]
pub(crate) struct Had {
    pub(crate) keys: Vec<String>,
    pub(crate) items: u32,
    pub(crate) live: u32,
}

/// The entry's reactions and what it has had, from the [`reactions_columns`] values. A damaged element is skipped.
pub(crate) fn parse_columns(
    had: Option<String>,
    json: Option<String>,
    entry_date: &str,
) -> (Vec<ReactionView>, Had) {
    let had = parse_had(had.as_deref());
    let Some(Value::Array(items)) = json.and_then(|text| serde_json::from_str(&text).ok()) else {
        return (Vec::new(), had);
    };
    let mut reactions: Vec<Reaction> = items.iter().filter_map(parse_one).collect();
    reactions.sort_by(|a, b| (a.put_at, &a.key).cmp(&(b.put_at, &b.key)));
    let views = reactions
        .into_iter()
        .map(|reaction| {
            let put_date = journal_day_in(reaction.put_at, &reaction.put_tz);
            ReactionView::new(reaction, entry_date, &put_date)
        })
        .collect();
    (views, had)
}

fn parse_had(text: Option<&str>) -> Had {
    let Some(Value::Object(object)) = text.and_then(|text| serde_json::from_str(text).ok()) else {
        return Had::default();
    };
    let count = |name: &str| {
        object
            .get(name)
            .and_then(Value::as_u64)
            .and_then(|n| u32::try_from(n).ok())
            .unwrap_or(0)
    };
    let keys = match object.get("keys") {
        Some(Value::Array(list)) => list
            .iter()
            .filter_map(|k| k.as_str().map(str::to_string))
            .collect(),
        _ => Vec::new(),
    };
    Had {
        keys,
        items: count("items"),
        live: count("live"),
    }
}

fn parse_one(item: &Value) -> Option<Reaction> {
    let text = |key: &str| item.get(key)?.as_str().map(str::to_string);
    Some(Reaction {
        key: text("k")?,
        emoji: text("e")?,
        put_at: item.get("a")?.as_i64()?,
        put_tz: text("z")?,
    })
}

/// The one function that builds the derived rows of an entry from its group bytes: the old rows go, the live reactions the reader lets
/// through come in, and the number of items is kept. A group that is absent, not well-formed or of another `sv` leaves no rows.
pub(crate) fn write_rows(
    conn: &Connection,
    id: &str,
    groups_raw: &str,
    hlc_json: &str,
) -> rusqlite::Result<()> {
    remove_rows(conn, id)?;
    let group = shown(groups_raw, hlc_json);
    if group.item_count == 0 {
        return Ok(());
    }
    // The counts are bounded by the group's 8 KiB, far below `i64`; a value that did not fit would be stored as the largest one.
    let count = |n: usize| i64::try_from(n).unwrap_or(i64::MAX);
    conn.execute(
        "INSERT INTO message_reaction_keys (message_id, keys, item_count, live_count) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![
            id,
            serde_json::to_string(&group.keys).unwrap_or_else(|_| "[]".to_string()),
            count(group.item_count),
            count(group.live_count)
        ],
    )?;
    let mut insert = conn.prepare_cached(
        "INSERT INTO message_reaction (message_id, key, emoji, put_at, put_tz) VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;
    for reaction in &group.reactions {
        insert.execute(rusqlite::params![
            id,
            reaction.key,
            reaction.emoji,
            reaction.put_at,
            reaction.put_tz
        ])?;
    }
    Ok(())
}

/// The entry's derived rows go (a write rebuilds them; purge removes them with the entry).
pub(crate) fn remove_rows(conn: &Connection, id: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM message_reaction WHERE message_id = ?1", [id])?;
    conn.execute(
        "DELETE FROM message_reaction_keys WHERE message_id = ?1",
        [id],
    )?;
    Ok(())
}

/// Every derived row is rebuilt from the groups, and the rules version is stored.
pub(crate) fn rebuild_all(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM message_reaction", [])?;
    conn.execute("DELETE FROM message_reaction_keys", [])?;
    let mut statement = conn.prepare(
        "SELECT id, groups_raw, hlc_json FROM message WHERE instr(groups_raw, '\"reactions\"') > 0",
    )?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let (id, groups_raw, hlc_json): (String, String, String) =
            (row.get(0)?, row.get(1)?, row.get(2)?);
        write_rows(conn, &id, &groups_raw, &hlc_json)?;
    }
    conn.execute(
        "INSERT INTO app_setting (key, value) VALUES (?1, ?2) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        (VERSION_KEY, RULES_VERSION.to_string()),
    )?;
    Ok(())
}

/// On opening the database: the rules version stored differs from the app's, so the rows are rebuilt in one transaction.
pub(crate) fn rebuild_if_stale(conn: &mut Connection) -> Result<(), StoreError> {
    let stored = get_setting(conn, VERSION_KEY).map_err(write_err)?;
    if stored.as_deref() == Some(RULES_VERSION.to_string().as_str()) {
        return Ok(());
    }
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(write_err)?;
    rebuild_all(&tx).map_err(write_err)?;
    tx.commit().map_err(write_err)
}

impl Store {
    /// Puts (`on`) or removes a reaction on an entry, in one writer transaction; returns the entry as it looks now. A put of an emoji the
    /// entry already has, and a removal of one it does not have, write nothing (the time of a live reaction is kept). A write is not an
    /// edit: no revision, `edited_at` unchanged, the favorite untouched.
    ///
    /// Errors, nothing written: `MessageNotFound` (not a UUID v4 too), `MessageInTrash`, `ReadOnly`, `ReactionInvalid` (not one emoji),
    /// `ReactionLimit` (a put when the entry has 10 live reactions or would get a 33rd key) and, for a stored group that is not
    /// well-formed, `Internal` (a corrupt group, as §C.7 treats one).
    pub fn set_message_reaction(
        &self,
        id: &str,
        emoji: &str,
        on: bool,
        now: &Now,
    ) -> Result<MessageView, StoreError> {
        // Both arguments are checked before anything is read: a garbage emoji is `ReactionInvalid` whatever the id is (PRD §12.3).
        if !is_emoji_sequence(emoji) {
            return Err(StoreError::ReactionInvalid);
        }
        if !is_uuid_v4(id) {
            return Err(StoreError::MessageNotFound);
        }
        let (id, emoji, now) = (id.to_string(), emoji.to_string(), now.clone());
        let faults = std::sync::Arc::clone(&self.inner.faults);
        let (view, written) = self.write(move |conn| {
            let tx = conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(write_err)?;
            let loaded = load(&tx, &id)?;
            if loaded.columns.deleted_at.is_some() {
                return Err(StoreError::MessageInTrash);
            }
            loaded.ensure_writable()?;
            let planned = plan(&loaded.groups(), &emoji, on)?;
            let written = planned.is_some();
            if let Some(planned) = planned {
                let hlc = next_hlc(&tx, now.ms)?;
                let done = apply(&loaded.groups(), &planned, (now.ms, &now.tz), &hlc)?;
                tx.execute(
                    "UPDATE message SET groups_raw = ?1, hlc_json = ?2, schema_version = ?3 WHERE id = ?4",
                    rusqlite::params![done.groups_raw, done.hlc_json, done.schema_version, id],
                )
                .map_err(write_err)?;
                write_rows(&tx, &id, &done.groups_raw, &done.hlc_json).map_err(write_err)?;
                if on {
                    remember(&tx, &emoji)?;
                }
                let columns = &loaded.columns;
                mark_dirty(&tx, &columns.chat_id, &columns.journal_date).map_err(write_err)?;
            }
            faults.run_before_reaction_commit();
            let view = load_view(&tx, &id)?;
            tx.commit().map_err(write_err)?;
            Ok((view, written))
        })?;
        if written {
            self.wake_mirror();
        }
        Ok(view)
    }
}

/// The recent list of this device: `emoji` goes first. Read and written inside the reaction's own transaction.
fn remember(tx: &Transaction<'_>, emoji: &str) -> Result<(), StoreError> {
    let stored = get_setting(tx, RECENT_KEY).map_err(read_err)?;
    let list = push_recent(&parse_recent(stored.as_deref()), emoji);
    let text = serde_json::to_string(&list).map_err(|e| StoreError::Internal(e.to_string()))?;
    tx.execute(
        "INSERT INTO app_setting (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        (RECENT_KEY, text),
    )
    .map_err(write_err)?;
    Ok(())
}
