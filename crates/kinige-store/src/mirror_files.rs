//! The attachment files of one day in the Markdown copy (slice 7, ADR-0014): `<chat>/<year>/<YYYY-MM-DD>/<name>` next to the day
//! file. Files are cloned from `attachments/` (a clone on APFS, `fs::copy`), a file already there with the same size is not
//! copied again, and only files this copy wrote itself (`mirror_file`) are ever removed: what the user put there stays.

use crate::attachment::is_stored_name;
use crate::writer::Writer;
use kinige_domain::attachment::mirror_file_names;
use rusqlite::Connection;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// An attachment of the day as the copy sees it.
pub(crate) struct DayAttachment {
    pub message_id: String,
    pub id: String,
    pub is_image: bool,
    /// The name the user knows.
    pub filename: String,
    /// The name inside the day folder: safe and unique there.
    pub name: String,
    /// The stored file in `attachments/` if this device has it.
    pub source: Option<PathBuf>,
}

/// The live attachments of a chat day in the order of the entries (`created_at`, `id`, `sort_index`), with the day folder names.
pub(crate) fn day_attachments(
    reader: &Connection,
    attachments_dir: &Path,
    chat_id: &str,
    journal_date: &str,
) -> rusqlite::Result<Vec<DayAttachment>> {
    let mut statement = reader.prepare(
        "SELECT a.message_id, a.id, a.kind, a.filename, l.file_name, l.state \
         FROM attachment a JOIN message m ON m.id = a.message_id \
         LEFT JOIN attachment_local l ON l.attachment_id = a.id \
         WHERE m.chat_id = ?1 AND m.journal_date = ?2 AND m.deleted_at IS NULL AND a.deleted_at IS NULL \
         ORDER BY m.created_at, m.id, a.sort_index",
    )?;
    let rows: Vec<(String, String, String, String, Option<String>, Option<String>)> = statement
        .query_map((chat_id, journal_date), |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
            ))
        })?
        .collect::<rusqlite::Result<_>>()?;
    let filenames: Vec<&str> = rows.iter().map(|row| row.3.as_str()).collect();
    let names = mirror_file_names(&filenames);
    Ok(rows
        .into_iter()
        .zip(names)
        .map(
            |((message_id, id, kind, filename, file_name, state), name)| DayAttachment {
                message_id,
                id,
                is_image: kind == "image",
                filename,
                name,
                source: file_name
                    .filter(|stored| is_stored_name(stored) && state.as_deref() == Some("local"))
                    .map(|stored| attachments_dir.join(stored)),
            },
        )
        .collect())
}

/// Brings the day folder `day_dir` to match `wanted`: copies what is missing, removes files this copy wrote earlier and no longer
/// wants, and records what it wrote. Returns `false` if a file could not be written (the day stays marked and is retried).
pub(crate) fn sync_day_folder(
    reader: &Connection,
    writer: &Writer,
    day_dir: &Path,
    chat_id: &str,
    journal_date: &str,
    wanted: &[DayAttachment],
) -> bool {
    let Ok(written) = recorded(reader, chat_id, journal_date) else {
        return false;
    };
    // The common day has no attachments and never had: nothing to do, no writer job.
    if wanted.is_empty() && written.is_empty() {
        return true;
    }
    let ours: HashSet<(&str, &str)> = written
        .iter()
        .map(|(id, name)| (id.as_str(), name.as_str()))
        .collect();
    let owned_names: HashSet<&str> = written.iter().map(|(_, name)| name.as_str()).collect();
    let mut ok = true;
    for item in wanted {
        let Some(source) = &item.source else {
            continue;
        };
        let how = if ours.contains(&(item.id.as_str(), item.name.as_str())) {
            Existing::SameIfSameSize
        } else if owned_names.contains(item.name.as_str()) {
            // The name was another attachment's of ours (names shift when an entry goes): the file is ours to replace.
            Existing::Replace
        } else {
            // Not ours: something the user put there is never overwritten.
            Existing::Keep
        };
        if !copy_if_needed(source, &day_dir.join(&item.name), how) {
            ok = false;
        }
    }
    // A name that is still wanted was just written; only the names nobody wants any more are removed.
    let wanted_names: HashSet<&str> = wanted.iter().map(|item| item.name.as_str()).collect();
    for (_, name) in &written {
        if !wanted_names.contains(name.as_str()) {
            let _ = fs::remove_file(day_dir.join(name));
        }
    }
    // An empty day folder goes; a folder with something the user put there stays (`remove_dir` refuses).
    let _ = fs::remove_dir(day_dir);
    if !ok {
        return false;
    }
    let (chat, date) = (chat_id.to_string(), journal_date.to_string());
    let rows: Vec<(String, String)> = wanted
        .iter()
        .filter(|item| item.source.is_some())
        .map(|item| (item.id.clone(), item.name.clone()))
        .collect();
    matches!(
        writer.run(move |conn| {
            let tx = conn.transaction()?;
            tx.execute(
                "DELETE FROM mirror_file WHERE chat_id = ?1 AND journal_date = ?2",
                (&chat, &date),
            )?;
            for (id, name) in &rows {
                tx.execute(
                    "INSERT INTO mirror_file (chat_id, journal_date, attachment_id, name) VALUES (?1, ?2, ?3, ?4)",
                    (&chat, &date, id, name),
                )?;
            }
            tx.commit()
        }),
        Ok(Ok(()))
    )
}

/// `(attachment id, name)` pairs this copy wrote into the day folder.
fn recorded(
    reader: &Connection,
    chat_id: &str,
    journal_date: &str,
) -> rusqlite::Result<Vec<(String, String)>> {
    let mut statement = reader.prepare(
        "SELECT attachment_id, name FROM mirror_file WHERE chat_id = ?1 AND journal_date = ?2",
    )?;
    statement
        .query_map((chat_id, journal_date), |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect()
}

/// What to do when the destination already exists.
#[derive(Clone, Copy)]
enum Existing {
    /// It is this attachment's file from an earlier pass: skip it if the size matches.
    SameIfSameSize,
    /// It is another attachment's file of ours: replace it.
    Replace,
    /// It is not ours: leave it.
    Keep,
}

/// Copies `source` to `dest` through a temporary name, as `how` says when `dest` exists. A missing source is not an error (the file
/// is gone; the link stays, the state is `missing`).
fn copy_if_needed(source: &Path, dest: &Path, how: Existing) -> bool {
    let Ok(source_meta) = fs::metadata(source) else {
        return true;
    };
    if let Ok(existing) = fs::metadata(dest) {
        match how {
            Existing::SameIfSameSize if existing.len() == source_meta.len() => return true,
            Existing::Keep => return true,
            _ => {}
        }
    }
    let (Some(dir), Some(name)) = (dest.parent(), dest.file_name()) else {
        return false;
    };
    if fs::create_dir_all(dir).is_err() {
        return false;
    }
    let temp = dir.join(format!(".{}.tmp", name.to_string_lossy()));
    let _ = fs::remove_file(&temp);
    let copied = fs::copy(source, &temp).and_then(|_| fs::rename(&temp, dest));
    if copied.is_err() {
        let _ = fs::remove_file(&temp);
    }
    copied.is_ok()
}
