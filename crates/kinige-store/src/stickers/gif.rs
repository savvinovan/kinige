//! The GIFs tab (slice 5c): the GIFs already in the journal, so a click sends one again as a new entry. No catalogue and no copy of anyone's
//! media: a journal keeps what it shows (PRD §2.3).

use crate::error::read_err;
use crate::{Store, StoreError};
use kinige_domain::stickers::JournalGif;
use std::collections::HashSet;

/// How many GIFs the tab lists.
const MAX_GIFS: usize = 60;

impl Store {
    /// The live, uncovered GIFs of live entries whose file is on this device, newest entry first, one per content hash.
    pub fn list_journal_gifs(&self) -> Result<Vec<JournalGif>, StoreError> {
        self.read(|conn| {
            let mut statement = conn
                .prepare(
                    "SELECT a.id, a.sha256, a.width, a.height, a.byte_size FROM attachment a \
                     JOIN message m ON m.id = a.message_id \
                     JOIN attachment_local l ON l.attachment_id = a.id AND l.state = 'local' \
                     WHERE a.kind = 'image' AND a.mime = 'image/gif' AND a.spoiler = 0 AND a.deleted_at IS NULL \
                       AND m.deleted_at IS NULL AND a.width IS NOT NULL AND a.height IS NOT NULL \
                     ORDER BY m.created_at DESC, a.id",
                )
                .map_err(read_err)?;
            let rows = statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, u32>(2)?,
                        row.get::<_, u32>(3)?,
                        row.get::<_, i64>(4)?,
                    ))
                })
                .map_err(read_err)?;
            let mut seen = HashSet::new();
            let mut gifs = Vec::new();
            for row in rows {
                let (attachment_id, sha256, width, height, byte_size) = row.map_err(read_err)?;
                if seen.insert(sha256) {
                    gifs.push(JournalGif {
                        attachment_id,
                        width,
                        height,
                        byte_size,
                    });
                    if gifs.len() >= MAX_GIFS {
                        break;
                    }
                }
            }
            Ok(gifs)
        })
    }
}
