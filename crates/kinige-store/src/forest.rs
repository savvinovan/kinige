//! The year's forest (slice 6, PRD §7): the per-day facts of one year, the entry a click on a day opens and the local view
//! state. Entries count when they are live (`deleted_at IS NULL`) and their chat is not in the trash; archived chats
//! count. One aggregate query over the `message_day` index per year (about 365 rows), no cache table (slice 6 spec).
//! The rules are in `kinige_domain::forest` / `forest_year`; nothing here is written to the log.

use crate::error::{read_err, write_err};
use crate::settings::get_setting;
use crate::{Now, Store, StoreError};
use chrono::NaiveDate;
use kinige_domain::chat::is_chat_id;
use kinige_domain::forest::{FirstWeekday, NoteStat, day_weight};
use kinige_domain::forest_year::{
    DayStats, FOREST_YEARS, ForestMode, ForestTarget, ForestView, ForestYear, day_chat, year_table,
};
use kinige_domain::groups::INBOX_ID;
use kinige_domain::journal::{DAY_BOUNDARY_MINUTES, is_valid_journal_date, journal_date};
use kinige_domain::on_this_day::year_of;
use rusqlite::{Connection, OptionalExtension};

pub(crate) const KEY_FOREST_VIEW: &str = "forest_view";
pub(crate) const KEY_FOREST_MODE: &str = "forest_mode";

/// Live entries of chats that are not in the trash whose text is text (a damaged value of another type is skipped everywhere
/// the same way: in the year table, the day's chats, the oldest year and the entry a click opens).
const LIVE: &str = "m.deleted_at IS NULL AND c.deleted_at IS NULL AND typeof(m.body_md) = 'text'";

/// One row per day of the year: date, entries, weight, the first 4096 characters of the first entry (the excerpt is far
/// shorter, and a huge entry is not copied for it), the ordering key of that entry, and how many entries of the day contain
/// a NUL character (see `year_stats`). The conditions repeat `LIVE` and must stay in step with it. Do not add a second
/// `MIN()` or `MAX()` aggregate: it would change which row the bare columns come from.
const YEAR_QUERY: &str = "SELECT m.journal_date, COUNT(*), \
     SUM(CASE WHEN length(m.body_md) >= 3 THEN MIN(1.0, length(m.body_md) / 200.0) ELSE 0.0 END), \
     substr(m.body_md, 1, 4096), MIN(printf('%020d', MAX(m.created_at, 0)) || m.id), \
     SUM(instr(CAST(m.body_md AS BLOB), x'00') > 0) \
     FROM message m JOIN chat c ON c.id = m.chat_id \
     WHERE m.journal_date >= ?1 AND m.journal_date <= ?2 \
     AND m.deleted_at IS NULL AND c.deleted_at IS NULL AND typeof(m.body_md) = 'text' \
     GROUP BY m.journal_date ORDER BY m.journal_date";

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

/// The facts of every day of `year` that has live entries, in calendar order: one SQL aggregate per day, so about 365
/// rows reach Rust instead of one per entry. The weight is the SQL form of `forest::day_weight` (`Σ min(1, len/200)`
/// over entries of 3 characters or more; `length()` counts characters, not bytes, and there are no attachments yet),
/// proven equal to it by the `sql_weight_equals_the_domain_weight` test. The first entry of a day is the one with the
/// smallest `created_at`, then id: SQLite takes the bare column `body_md` from the row of the query's only `MIN()`
/// aggregate, whose key is the zero-padded creation time (clamped at 0: creation times are milliseconds since 1970, and a
/// negative number would sort wrongly as text) followed by the id. Only the start of the text is read.
///
/// `length()` stops at a NUL character while the domain counts every character. A day with such an entry (never written
/// by the app, but possible in imported or synced text) is measured again in Rust, entry by entry. Rows whose date is not a
/// date or whose text is not text (a damaged value) are skipped.
fn year_stats(conn: &Connection, year: i32) -> Result<Vec<DayStats>, StoreError> {
    let mut statement = conn.prepare_cached(YEAR_QUERY).map_err(read_err)?;
    let mut rows = statement
        .query((format!("{year:04}-01-01"), format!("{year:04}-12-31")))
        .map_err(read_err)?;
    let mut days = Vec::with_capacity(366);
    while let Some(row) = rows.next().map_err(read_err)? {
        let date = row.get_ref(0).map_err(read_err)?.as_str().ok();
        let Some(date) = date.and_then(|date| NaiveDate::parse_from_str(date, "%Y-%m-%d").ok())
        else {
            continue;
        };
        let notes: i64 = row.get(1).map_err(read_err)?;
        let mut weight: f64 = row.get(2).map_err(read_err)?;
        let mut first_body: Option<String> = row.get(3).map_err(read_err)?;
        let with_nul: i64 = row.get(5).map_err(read_err)?;
        if with_nul > 0 {
            (weight, first_body) = exact_day(conn, date)?;
        }
        days.push(DayStats {
            date,
            notes: u32::try_from(notes).unwrap_or(u32::MAX),
            weight,
            first_body,
        });
    }
    Ok(days)
}

/// The weight and the first entry of one day by the domain rules over every entry, the slow way (see `year_stats`).
fn exact_day(conn: &Connection, date: NaiveDate) -> Result<(f64, Option<String>), StoreError> {
    let mut statement = conn
        .prepare_cached(&format!(
            "SELECT m.created_at, m.id, m.body_md FROM message m JOIN chat c ON c.id = m.chat_id \
             WHERE m.journal_date = ?1 AND {LIVE}"
        ))
        .map_err(read_err)?;
    let rows = statement
        .query_map([date.format("%Y-%m-%d").to_string()], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(read_err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(read_err)?;
    let notes: Vec<NoteStat> = rows
        .iter()
        .map(|(_, _, body)| NoteStat {
            len: body.chars().count(),
            attachments: 0,
        })
        .collect();
    let first = rows
        .into_iter()
        .min_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)))
        .map(|(_, _, body)| body);
    Ok((day_weight(&notes), first))
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
