//! The sticker sweep (slice 5c, ADR-0021): like the attachment sweep, a **writer job** that runs after the first frame, so it cannot see a
//! file that a pack write has renamed but not yet written a row for. It removes the staging folders of other processes (`stickers/.tmp/`)
//! and library files without a row. It never touches `attachments/`: a sent sticker's own file belongs to its attachment.

use crate::attachment::{Dirs, remove_other_processes};
use crate::error::write_err;
use crate::{Store, StoreError};
use rusqlite::Connection;
use std::collections::HashSet;
use std::fs;

/// What a sticker sweep did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StickerSweep {
    /// Staging folders of other processes removed.
    pub stale_dirs: usize,
    /// Library files without a row, deleted.
    pub files_removed: usize,
}

impl Store {
    /// Cleans up after crashes. Safe at any time and concurrently with a pack write: both are writer jobs.
    pub fn sweep_stickers(&self) -> Result<StickerSweep, StoreError> {
        let dirs = self.dirs();
        self.write(move |conn| sweep(conn, &dirs))
    }
}

fn sweep(conn: &mut Connection, dirs: &Dirs) -> Result<StickerSweep, StoreError> {
    let stale_dirs = remove_other_processes(&dirs.stickers_tmp_root(), dirs.process());
    let owned: HashSet<String> = {
        let mut statement = conn
            .prepare("SELECT id || '.' || ext FROM sticker")
            .map_err(write_err)?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(write_err)?;
        rows.collect::<Result<_, _>>().map_err(write_err)?
    };
    let mut files_removed = 0;
    if let Ok(entries) = fs::read_dir(dirs.stickers()) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            // Only `<uuid>.<ext>` files are ours; `.tmp` and anything else stay where they are.
            let Some(path) = dirs.library_file(&name) else {
                continue;
            };
            if !owned.contains(&name) && path.is_file() && fs::remove_file(path).is_ok() {
                files_removed += 1;
            }
        }
    }
    Ok(StickerSweep {
        stale_dirs,
        files_removed,
    })
}
