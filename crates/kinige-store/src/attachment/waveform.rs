//! The local waveform of a voice note (slice 7a, ADR-0015): reading the bytes the player's decoder needs and saving the peaks it
//! computed. Device-local data in `attachment_local.waveform`: no HLC, no tombstone, no copy mark, nothing synced, so these calls
//! never touch `core` or the Markdown copy.

use crate::error::{read_err, write_err};
use crate::{Store, StoreError};
use kinige_domain::attachment::waveform_is_storable;
use kinige_domain::hlc::is_uuid_v4;
use rusqlite::{Connection, OptionalExtension};
use std::io::{Read, Seek, SeekFrom};

/// The decoder is not asked to read a file bigger than this (a Telegram audiobook would take seconds and gigabytes of PCM): the
/// player stores the empty "failed" marker for it instead.
pub const MAX_DECODE_BYTES: u64 = 16 * 1024 * 1024;

impl Store {
    /// The whole stored file of an `audio` attachment, for the player's decoder. Everything the `audio` route checks is checked here
    /// (the id, `kind = audio`, state `local`, the MIME allowlist, the container's first bytes); on top of that, a file over
    /// [`MAX_DECODE_BYTES`] is `AttachmentTooLarge`. Anything the route would answer 404 is `AttachmentNotFound`.
    pub fn audio_for_waveform(&self, attachment_id: &str) -> Result<Vec<u8>, StoreError> {
        let mut audio = self
            .audio_file(attachment_id)
            .ok_or(StoreError::AttachmentNotFound)?;
        if audio.len > MAX_DECODE_BYTES {
            return Err(StoreError::AttachmentTooLarge);
        }
        // The handle has read the container's first bytes to check them: start again from the top.
        audio
            .file
            .seek(SeekFrom::Start(0))
            .map_err(|_| StoreError::AttachmentUnreadable)?;
        let mut bytes = Vec::new();
        // One byte more than the limit: a file that grew under the handler is refused, never read whole.
        audio
            .file
            .take(MAX_DECODE_BYTES.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|_| StoreError::AttachmentUnreadable)?;
        if !u64::try_from(bytes.len()).is_ok_and(|len| len <= MAX_DECODE_BYTES) {
            return Err(StoreError::AttachmentTooLarge);
        }
        Ok(bytes)
    }

    /// Saves the peaks the player computed for an `audio` attachment: exactly `WAVEFORM_PEAKS` bytes, or empty for "this file cannot
    /// be decoded, do not try again". The first value wins (a second call for a note that has one is a quiet no-op), so two windows
    /// or two renders cannot fight over it. Anything else, a wrong length or an id that is not a live `audio` attachment with a local
    /// row, is an error and writes nothing: `AttachmentUnreadable` for the bytes, `AttachmentNotFound` for the id.
    pub fn set_attachment_waveform(
        &self,
        attachment_id: &str,
        peaks: &[u8],
    ) -> Result<(), StoreError> {
        if !is_uuid_v4(attachment_id) {
            return Err(StoreError::AttachmentNotFound);
        }
        if !waveform_is_storable(peaks) {
            return Err(StoreError::AttachmentUnreadable);
        }
        let id = attachment_id.to_string();
        let peaks = peaks.to_vec();
        self.write(move |conn| save(conn, &id, &peaks))
    }
}

fn save(conn: &mut Connection, id: &str, peaks: &[u8]) -> Result<(), StoreError> {
    let tx = conn.transaction().map_err(write_err)?;
    let found: Option<(String, bool)> = tx
        .query_row(
            "SELECT a.kind, l.waveform IS NOT NULL FROM attachment a \
             JOIN attachment_local l ON l.attachment_id = a.id \
             WHERE a.id = ?1 AND a.deleted_at IS NULL",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(read_err)?;
    match found {
        Some((kind, false)) if kind == "audio" => {
            tx.execute(
                "UPDATE attachment_local SET waveform = ?2 WHERE attachment_id = ?1",
                rusqlite::params![id, peaks],
            )
            .map_err(write_err)?;
            tx.commit().map_err(write_err)
        }
        Some((kind, true)) if kind == "audio" => Ok(()),
        _ => Err(StoreError::AttachmentNotFound),
    }
}
