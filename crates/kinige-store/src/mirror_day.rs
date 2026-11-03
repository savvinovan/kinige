//! The file of one day of the Markdown copy: the day's entries with a reply quote, rewriting through a temporary file
//! and deleting an empty day.

use crate::time::local_hhmm;
use kinige_domain::MirrorError;
use kinige_domain::mirror::{
    MirrorEntry, MirrorReply, day_file_segments, relative_day_link, render_day_file,
};
use rusqlite::Connection;
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::Path;

/// Rewrites the file of one day of one chat, or deletes it if no entries are left.
pub(crate) fn write_day(
    reader: &Connection,
    root: &Path,
    chat_id: &str,
    folder: &str,
    folders: &BTreeMap<String, String>,
    journal_date: &str,
) -> Result<(), MirrorError> {
    let Some(segments) = day_file_segments(folder, journal_date) else {
        // Invalid date or folder name: there can be no file, so we clear the mark (otherwise it would never go away).
        return Ok(());
    };
    let entries = day_entries(reader, chat_id, folder, folders, journal_date)
        .map_err(|_| MirrorError::WriteFailed)?;
    let [chat_dir, year_dir, file_name] = segments;
    let year_path = root.join(&chat_dir).join(&year_dir);
    let file_path = year_path.join(&file_name);

    if entries.is_empty() {
        match fs::remove_file(&file_path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(MirrorError::WriteFailed),
        }
        // Remove the year and chat folders if they became empty; the copy root is left alone.
        let _ = fs::remove_dir(&year_path);
        let _ = fs::remove_dir(root.join(&chat_dir));
        return Ok(());
    }

    let text = render_day_file(journal_date, &entries);
    write_atomically(&year_path, &file_name, text.as_bytes()).map_err(|_| MirrorError::WriteFailed)
}

/// A temporary file next to the target -> write -> `fsync` -> rename: a reader sees either the old or the new file whole.
fn write_atomically(dir: &Path, name: &str, bytes: &[u8]) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    let temp = dir.join(format!(".{name}.tmp"));
    let result = (|| {
        let mut file = fs::File::create(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&temp, dir.join(name))?;
        // The directory entry (the rename itself) is flushed to disk too: otherwise after a power loss the file could
        // end up under the old name. A directory cannot be opened on Windows, so the error is ignored there.
        if let Ok(directory) = fs::File::open(dir) {
            let _ = directory.sync_all();
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

/// The day's entries with a reply quote (slice 5): the old entry is taken by `LEFT JOIN` on `reply_to_id`, without a
/// foreign key. The link leads to the day file of the old entry in its chat's folder; an entry in the trash does not
/// get into the copy, so its line has no link.
fn day_entries(
    reader: &Connection,
    chat_id: &str,
    folder: &str,
    folders: &BTreeMap<String, String>,
    journal_date: &str,
) -> rusqlite::Result<Vec<MirrorEntry>> {
    let mut statement = reader.prepare(
        "SELECT m.body_md, m.created_at, m.created_tz, m.reply_to_id, \
                r.chat_id, r.journal_date, r.created_at, r.created_tz, r.deleted_at IS NOT NULL \
         FROM message m LEFT JOIN message r ON r.id = m.reply_to_id \
         WHERE m.chat_id = ?1 AND m.journal_date = ?2 AND m.deleted_at IS NULL ORDER BY m.created_at, m.id",
    )?;
    statement
        .query_map((chat_id, journal_date), |row| {
            let created_at: i64 = row.get(1)?;
            let tz: String = row.get(2)?;
            let reply_id: Option<String> = row.get(3)?;
            let original: Option<(String, String, i64, String, bool)> =
                match row.get::<_, Option<String>>(4)? {
                    Some(old_chat) => {
                        Some((old_chat, row.get(5)?, row.get(6)?, row.get(7)?, row.get(8)?))
                    }
                    None => None,
                };
            let reply = reply_id.map(|_| match original {
                None => MirrorReply::Gone,
                Some((old_chat, date, old_at, old_tz, in_trash)) => {
                    let link = (!in_trash)
                        .then(|| folders.get(&old_chat))
                        .flatten()
                        .and_then(|old_folder| day_file_segments(old_folder, &date))
                        .map(|segments| relative_day_link(folder, &segments));
                    MirrorReply::Original {
                        time: local_hhmm(old_at, &old_tz),
                        date,
                        link,
                    }
                }
            });
            Ok(MirrorEntry {
                time: local_hhmm(created_at, &tz),
                body_md: row.get(0)?,
                reply,
            })
        })?
        .collect()
}
