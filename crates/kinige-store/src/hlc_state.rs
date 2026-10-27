//! Состояние HLC устройства в `sync_state` (`device_id`, `hlc_l`, `hlc_c`): читается и пишется в той же транзакции,
//! что и изменение (ADR-0001 §2).

use crate::StoreError;
use kinige_domain::hlc::{Clock, HlcError};
use rusqlite::{OptionalExtension, Transaction};

pub(crate) fn get_state(tx: &Transaction<'_>, key: &str) -> rusqlite::Result<Option<String>> {
    tx.query_row(
        "SELECT value FROM sync_state WHERE key = ?1",
        [key],
        |row| row.get(0),
    )
    .optional()
}

pub(crate) fn set_state(tx: &Transaction<'_>, key: &str, value: &str) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT INTO sync_state(key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        (key, value),
    )?;
    Ok(())
}

/// Сохранённое состояние часов.
pub(crate) fn load_clock(tx: &Transaction<'_>) -> Result<Option<Clock>, StoreError> {
    let write = crate::error::write_err;
    let (Some(l), Some(c)) = (
        get_state(tx, "hlc_l").map_err(write)?,
        get_state(tx, "hlc_c").map_err(write)?,
    ) else {
        return Ok(None);
    };
    match (l.parse(), c.parse()) {
        (Ok(l), Ok(c)) => Ok(Some(Clock { l, c })),
        _ => Err(StoreError::Internal("повреждены hlc_l или hlc_c".into())),
    }
}

pub(crate) fn save_clock(tx: &Transaction<'_>, clock: Clock) -> rusqlite::Result<()> {
    set_state(tx, "hlc_l", &clock.l.to_string())?;
    set_state(tx, "hlc_c", &clock.c.to_string())
}

/// Следующая метка HLC для локального события в момент `pt_ms`; новое состояние часов пишется в `sync_state`.
pub(crate) fn next_hlc(tx: &Transaction<'_>, pt_ms: i64) -> Result<String, StoreError> {
    let write = crate::error::write_err;
    let device = get_state(tx, "device_id")
        .map_err(write)?
        .ok_or_else(|| StoreError::Internal("нет device_id".into()))?;
    let clock =
        load_clock(tx)?.ok_or_else(|| StoreError::Internal("нет состояния часов".into()))?;
    let next = clock
        .tick(u64::try_from(pt_ms).unwrap_or(0))
        .map_err(|e: HlcError| StoreError::Internal(e.to_string()))?;
    save_clock(tx, next).map_err(write)?;
    Ok(next.format(&device))
}
