//! "On this day" (slice 5, PRD §2.1): picking the Inbox entries of this calendar day in past years and hiding the card
//! until the next journal day. The domain computes the dates of past years; the query goes over the index
//! `message_feed (chat_id, journal_date, …)` for each date, not over `substr` of the whole table.

use crate::error::{read_err, write_err};
use crate::settings::get_setting;
use crate::view::{VIEW_COLUMNS, view_from_row};
use crate::{Now, Store, StoreError};
use kinige_domain::groups::INBOX_ID;
use kinige_domain::journal::{DAY_BOUNDARY_MINUTES, is_valid_journal_date, journal_date};
use kinige_domain::on_this_day::{
    DETAILED_YEARS, ENTRIES_PER_YEAR, OnThisDay, OnThisDayEntry, OnThisDayYear, past_dates,
    year_of, years_ago,
};
use rusqlite::{Connection, OptionalExtension};

/// `app_setting`: the `journal_date` until the end of which the card is hidden. Local, not synced, not written to
/// `settings.json` (it is not a setting but the state of the day).
const DISMISSED_KEY: &str = "on_this_day_dismissed";

impl Store {
    /// The "On this day" card for today's journal day. `None` if there are no entries of this calendar day in past
    /// years or the card was hidden for today.
    pub fn get_on_this_day(&self, now: &Now) -> Result<Option<OnThisDay>, StoreError> {
        let today = journal_date(now.ms, now.utc_offset_secs, DAY_BOUNDARY_MINUTES);
        self.read(|conn| {
            // One read transaction: the database snapshot is shared by the counter and the year's entries, so `total`
            // never differs from `entries`, even if the writer adds an entry between the queries.
            let conn = &conn.unchecked_transaction().map_err(read_err)?;
            let dismissed = get_setting(conn, DISMISSED_KEY).map_err(read_err)?;
            if dismissed.as_deref() == Some(today.as_str()) {
                return Ok(None);
            }
            let Some(oldest) = oldest_year(conn)? else {
                return Ok(None);
            };
            let mut years = Vec::new();
            for date in past_dates(&today, oldest) {
                let date_text = date.format("%Y-%m-%d").to_string();
                let detailed = years.len() < DETAILED_YEARS;
                if let Some(year) = load_year(conn, &date_text, years_ago(&today, &date), detailed)?
                {
                    years.push(year);
                }
            }
            Ok((!years.is_empty()).then_some(OnThisDay { today, years }))
        })
    }

    /// "Hide until tomorrow": the card is not shown until the end of the journal day `date`. Not a date means `InvalidDate`.
    pub fn dismiss_on_this_day(&self, date: &str) -> Result<(), StoreError> {
        if !is_valid_journal_date(date) {
            return Err(StoreError::InvalidDate);
        }
        let date = date.to_string();
        self.write(move |conn| {
            conn.execute(
                "INSERT INTO app_setting(key, value) VALUES (?1, ?2) \
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                (DISMISSED_KEY, date),
            )
            .map_err(write_err)?;
            Ok(())
        })
    }
}

/// The year of the oldest Inbox entry, trash excluded; `None` means there are no entries.
fn oldest_year(conn: &Connection) -> Result<Option<i32>, StoreError> {
    let oldest: Option<String> = conn
        .query_row(
            "SELECT journal_date FROM message WHERE chat_id = ?1 AND deleted_at IS NULL \
             ORDER BY journal_date LIMIT 1",
            [INBOX_ID],
            |row| row.get(0),
        )
        .optional()
        .map_err(read_err)?;
    Ok(oldest.as_deref().and_then(year_of))
}

/// A card year for one day: the number of entries and the first entry; a detailed year also has the entries
/// themselves. `None` if there are no Inbox entries on that day.
fn load_year(
    conn: &Connection,
    journal_date: &str,
    years_ago: i64,
    detailed: bool,
) -> Result<Option<OnThisDayYear>, StoreError> {
    let total: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM message WHERE chat_id = ?1 AND journal_date = ?2 AND deleted_at IS NULL",
            (INBOX_ID, journal_date),
            |row| row.get(0),
        )
        .map_err(read_err)?;
    if total == 0 {
        return Ok(None);
    }
    let take = if detailed { ENTRIES_PER_YEAR } else { 1 };
    let mut statement = conn
        .prepare_cached(&format!(
            "SELECT {VIEW_COLUMNS} FROM message m WHERE m.chat_id = ?1 AND m.journal_date = ?2 AND m.deleted_at IS NULL \
             ORDER BY m.created_at, m.id LIMIT ?3"
        ))
        .map_err(read_err)?;
    let first = statement
        .query_map((INBOX_ID, journal_date, take as i64), view_from_row)
        .map_err(read_err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(read_err)?;
    let Some(first_id) = first.first().map(|view| view.id.clone()) else {
        return Ok(None);
    };
    Ok(Some(OnThisDayYear {
        journal_date: journal_date.to_string(),
        years_ago,
        total,
        first_id,
        entries: if detailed {
            first.into_iter().map(OnThisDayEntry::new).collect()
        } else {
            Vec::new()
        },
    }))
}
