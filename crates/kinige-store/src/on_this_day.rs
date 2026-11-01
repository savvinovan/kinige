//! «В этот день» (слайс 5, ТЗ §2.1): отбор записей Inbox этого числа прошлых лет и скрытие карточки до следующего дня
//! журнала. Даты прошлых лет считает домен; запрос идёт по индексу `message_feed (chat_id, journal_date, …)` на каждую
//! дату, а не по `substr` всей таблицы.

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

/// `app_setting`: `journal_date`, до конца которого карточка скрыта. Локально, не синхронизируется, в `settings.json` не
/// пишется (это не настройка, а состояние дня).
const DISMISSED_KEY: &str = "on_this_day_dismissed";

impl Store {
    /// Карточка «В этот день» для сегодняшнего дня журнала. `None`, если записей этого числа в прошлые годы нет или
    /// карточку на сегодня скрыли.
    pub fn get_on_this_day(&self, now: &Now) -> Result<Option<OnThisDay>, StoreError> {
        let today = journal_date(now.ms, now.utc_offset_secs, DAY_BOUNDARY_MINUTES);
        self.read(|conn| {
            // Одна транзакция чтения: снимок базы общий для счётчика и записей года, `total` не расходится с `entries`, даже
            // если писатель добавляет запись между запросами.
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

    /// «Скрыть до завтра»: карточка не показывается до конца дня журнала `date`. Не дата — `InvalidDate`.
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

/// Год самой старой записи Inbox, кроме корзины; `None` — записей нет.
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

/// Год карточки за один день: число записей и первая запись; у подробного года ещё и сами записи. `None`, если в этот
/// день записей Inbox нет.
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
