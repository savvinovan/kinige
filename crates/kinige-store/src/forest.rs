//! The year's forest (slice 6, PRD §7): the per-day facts of one year, the entry a click on a day opens and the local view
//! state. Entries count when they are live (`deleted_at IS NULL`) and their chat is not in the trash; archived chats
//! count. One range query over the `message_day` index per year, scanned row by row, no cache table (slice 6 spec).
//! The rules are in `kinige_domain::forest` / `forest_year`; nothing here is written to the log.

use crate::attachment::summary_for;
use crate::error::{read_err, write_err};
use crate::settings::get_setting;
use crate::{Now, Store, StoreError};
use chrono::NaiveDate;
use kinige_domain::chat::is_chat_id;
use kinige_domain::forest::{FirstWeekday, NoteStat};
use kinige_domain::forest_year::{
    DayStats, FOREST_YEARS, ForestMode, ForestTarget, ForestView, ForestYear, day_chat, year_table,
};
use kinige_domain::groups::INBOX_ID;
use kinige_domain::journal::{DAY_BOUNDARY_MINUTES, is_valid_journal_date, journal_date};
use kinige_domain::on_this_day::year_of;
use rusqlite::{Connection, OptionalExtension};
use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, HashSet};

pub(crate) const KEY_FOREST_VIEW: &str = "forest_view";
pub(crate) const KEY_FOREST_MODE: &str = "forest_mode";

/// How many characters of a day's first entry the year table carries (the excerpt shown is far shorter).
const EXCERPT_CHARS: usize = 4096;

/// Live entries of chats that are not in the trash whose text is text (a damaged value of another type is skipped everywhere
/// the same way: in the year table, the day's chats, the oldest year and the entry a click opens).
const LIVE: &str = "m.deleted_at IS NULL AND c.deleted_at IS NULL AND typeof(m.body_md) = 'text'";

impl Store {
    /// The table of `year` for the forest: every day with its cell, season, notes, stage and first-entry excerpt, plus the
    /// current journal day and the year of the oldest entry. A year outside `1000..=9999` is `InvalidDate`. The
    /// facts are read in one snapshot.
    pub fn get_forest_year(
        &self,
        year: i32,
        first: FirstWeekday,
        now: &Now,
    ) -> Result<ForestYear, StoreError> {
        if !FOREST_YEARS.contains(&year) {
            return Err(StoreError::InvalidDate);
        }
        let today = journal_date(now.ms, now.utc_offset_secs, DAY_BOUNDARY_MINUTES);
        let (stats, oldest) = self.read(|conn| {
            let conn = &conn.unchecked_transaction().map_err(read_err)?;
            Ok((year_stats(conn, year)?, oldest_year(conn)?))
        })?;
        let table = year_table(year, first, &stats).ok_or(StoreError::InvalidDate)?;
        let this_year = year_of(&today).unwrap_or(year);
        Ok(ForestYear {
            year,
            first_weekday: first,
            cols: table.cols,
            min_year: oldest.map_or(this_year, |oldest| oldest.min(this_year)),
            today,
            days: table.days,
        })
    }

    /// The entry a click on a day opens: the open chat if it has entries that day, otherwise the chat with the most,
    /// the Inbox on a tie (`forest_year::day_chat`); the entry is the day's first one in that chat. `None` for an empty
    /// day. A chat id that does not exist is treated as "no open chat". A bad date is `InvalidDate`.
    pub fn forest_day_target(
        &self,
        date: &str,
        current_chat: Option<&str>,
    ) -> Result<Option<ForestTarget>, StoreError> {
        if !is_valid_journal_date(date) {
            return Err(StoreError::InvalidDate);
        }
        // The id comes from the window: something that cannot be a chat id is "no open chat", never an error.
        let current_chat = current_chat.filter(|id| is_chat_id(id));
        self.read(|conn| {
            let conn = &conn.unchecked_transaction().map_err(read_err)?;
            let counts = chat_counts(conn, date)?;
            let Some(chat_id) = day_chat(&counts, current_chat, INBOX_ID) else {
                return Ok(None);
            };
            let message_id: Option<String> = conn
                .query_row(
                    "SELECT m.id FROM message m WHERE m.chat_id = ?1 AND m.journal_date = ?2 AND m.deleted_at IS NULL \
                     AND typeof(m.body_md) = 'text' ORDER BY m.created_at, m.id LIMIT 1",
                    (chat_id, date),
                    |row| row.get(0),
                )
                .optional()
                .map_err(read_err)?;
            Ok(message_id.map(|message_id| ForestTarget {
                chat_id: chat_id.to_string(),
                message_id,
            }))
        })
    }

    /// Remembers how much of the forest is shown and whether it is the board or the calendar. Local, not synced, not
    /// written to `settings.json`; both values are saved in one transaction.
    pub fn set_forest_view(&self, view: ForestView, mode: ForestMode) -> Result<(), StoreError> {
        self.write(move |conn| {
            let tx = conn.transaction().map_err(write_err)?;
            for (key, value) in [
                (KEY_FOREST_VIEW, view.as_setting()),
                (KEY_FOREST_MODE, mode.as_setting()),
            ] {
                tx.execute(
                    "INSERT INTO app_setting(key, value) VALUES (?1, ?2) \
                     ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                    (key, value),
                )
                .map_err(write_err)?;
            }
            tx.commit().map_err(write_err)?;
            Ok(())
        })
    }
}

/// How the stored view state reads back: unknown or missing values are the defaults (the strip, the board).
pub(crate) fn stored_view(conn: &Connection) -> rusqlite::Result<(ForestView, ForestMode)> {
    let view = get_setting(conn, KEY_FOREST_VIEW)?;
    let mode = get_setting(conn, KEY_FOREST_MODE)?;
    Ok((
        ForestView::from_setting(view.as_deref()),
        ForestMode::from_setting(mode.as_deref()),
    ))
}

/// The facts of every day of `year` that has live entries, in calendar order. The rows are read over the `message_day`
/// index (one row per entry, a measured 100,000-entry bench is in ADR-0013) and each day is computed here: the weight by
/// the domain's own `day_weight`, the first entry by `created_at`, then id (the order `forest_day_target` uses). Text is
/// copied only from an entry that is the day's first so far, and only its first `EXCERPT_CHARS` characters (the excerpt is
/// far shorter). Rows whose
/// date is not a date or whose text is not text (a damaged value) are skipped.
fn year_stats(conn: &Connection, year: i32) -> Result<Vec<DayStats>, StoreError> {
    /// One day while it is being scanned: the weights of its entries and the first one so far.
    struct Day {
        notes: Vec<NoteStat>,
        first: (i64, String, String),
        first_has_files: bool,
        photos: u32,
    }
    // Which entries have attachments at all: one index-only pass over `attachment_message`. Only those entries are asked for their
    // counts, so the other nine in ten cost a hash lookup, not a probe of the table (a probe per entry cost a fifth of the pass).
    let owners = attachment_owners(conn)?;
    let mut counts = conn.prepare_cached(COUNTS_SQL).map_err(read_err)?;
    let mut statement = conn.prepare_cached(&year_sql()).map_err(read_err)?;
    let mut rows = statement
        .query((format!("{year:04}-01-01"), format!("{year:04}-12-31")))
        .map_err(read_err)?;
    let mut days: BTreeMap<NaiveDate, Day> = BTreeMap::new();
    while let Some(row) = rows.next().map_err(read_err)? {
        let (date, id, body) = (
            row.get_ref(0).map_err(read_err)?.as_str().ok(),
            row.get_ref(2).map_err(read_err)?.as_str().ok(),
            row.get_ref(3).map_err(read_err)?.as_str().ok(),
        );
        let (Some(date), Some(id), Some(body)) = (date, id, body) else {
            continue;
        };
        let Ok(date) = NaiveDate::parse_from_str(date, "%Y-%m-%d") else {
            continue;
        };
        let created: i64 = row.get(1).map_err(read_err)?;
        let (all, photos) = if owners.contains(id) {
            counts
                .query_row([id], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
                })
                .map_err(read_err)?
        } else {
            (0, 0)
        };
        let (all, photos) = (
            usize::try_from(all).unwrap_or(0),
            usize::try_from(photos).unwrap_or(0),
        );
        let note = NoteStat {
            len: body.chars().count(),
            attachments: all,
        };
        match days.entry(date) {
            Entry::Vacant(slot) => {
                slot.insert(Day {
                    notes: vec![note],
                    first: (created, id.to_string(), excerpt_of(body)),
                    first_has_files: all > 0,
                    photos: u32::try_from(photos).unwrap_or(u32::MAX),
                });
            }
            Entry::Occupied(mut slot) => {
                let day = slot.get_mut();
                day.notes.push(note);
                day.photos = day
                    .photos
                    .saturating_add(u32::try_from(photos).unwrap_or(u32::MAX));
                if (created, id) < (day.first.0, day.first.1.as_str()) {
                    day.first = (created, id.to_string(), excerpt_of(body));
                    day.first_has_files = all > 0;
                }
            }
        }
    }
    let mut stats = Vec::with_capacity(days.len());
    for (date, day) in days {
        let mut stat = DayStats::from_notes(date, &day.notes, Some(day.first.2.clone()));
        stat.photos = day.photos;
        // A first entry without text (a photo sent alone) is described by what it holds.
        if day.first.2.is_empty() && day.first_has_files {
            stat.first_summary = Some(summary_for(conn, &day.first.1)?);
        }
        stats.push(stat);
    }
    Ok(stats)
}

/// The rows of one year, `?1..=?2` being its first and last date.
fn year_sql() -> String {
    format!(
        "SELECT m.journal_date, m.created_at, m.id, m.body_md FROM message m JOIN chat c ON c.id = m.chat_id \
         WHERE m.journal_date >= ?1 AND m.journal_date <= ?2 AND {LIVE}"
    )
}

/// The live attachments of one entry (`?1`) and how many of them are photos.
const COUNTS_SQL: &str = "SELECT COUNT(*), COALESCE(SUM(kind = 'image'), 0) FROM attachment \
                          WHERE message_id = ?1 AND deleted_at IS NULL";

/// The ids of the entries that have any attachment row (live or not): the index `attachment_message` covers the column, so this reads
/// no table rows. An entry in the set is then asked for its live counts; an entry outside it has none.
fn attachment_owners(conn: &Connection) -> Result<HashSet<String>, StoreError> {
    let mut statement = conn
        .prepare_cached("SELECT DISTINCT message_id FROM attachment")
        .map_err(read_err)?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(read_err)?;
    rows.collect::<Result<_, _>>().map_err(read_err)
}

/// The start of an entry that is kept for the excerpt, cut on a character boundary.
fn excerpt_of(body: &str) -> String {
    body.chars().take(EXCERPT_CHARS).collect()
}

/// The year of the oldest live entry of a chat that is not in the trash.
fn oldest_year(conn: &Connection) -> Result<Option<i32>, StoreError> {
    let oldest: Option<String> = conn
        .query_row(
            &format!(
                "SELECT m.journal_date FROM message m JOIN chat c ON c.id = m.chat_id WHERE {LIVE} \
                 ORDER BY m.journal_date LIMIT 1"
            ),
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(read_err)?;
    Ok(oldest.as_deref().and_then(year_of))
}

/// `(chat id, live entries)` of one day.
fn chat_counts(conn: &Connection, date: &str) -> Result<Vec<(String, usize)>, StoreError> {
    let mut statement = conn
        .prepare_cached(&format!(
            "SELECT m.chat_id, COUNT(*) FROM message m JOIN chat c ON c.id = m.chat_id \
             WHERE m.journal_date = ?1 AND {LIVE} GROUP BY m.chat_id"
        ))
        .map_err(read_err)?;
    let counts = statement
        .query_map([date], |row| {
            let count: i64 = row.get(1)?;
            Ok((row.get(0)?, usize::try_from(count).unwrap_or(0)))
        })
        .map_err(read_err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(read_err)?;
    Ok(counts)
}

#[cfg(test)]
mod tests;
