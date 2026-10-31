//! Восстановление из копии и файл настроек (ADR-0010).

use super::apply::{KEY_FILE_HASH, check_folder};
use super::io::{self, ReadOutcome};
use super::parse::{Field, parse};
use crate::agent::{KEY_CHATS, KEY_ENABLED};
use crate::agent_settings::put;
use rusqlite::Connection;
use std::path::{Path, PathBuf};

/// Папка Markdown-копии из `settings.json` для восстановления: только чтение. Файл небезопасен, не разобран или значение
/// негодное — `None` (такую папку файл всё равно не применил бы). `null` в файле — папка по умолчанию (`default_mirror`):
/// после применения файла копия пойдёт в неё, значит, её тоже нужно увести, если там лежат дни, появившиеся после снимка.
pub(crate) fn mirror_folder_from_file(
    data_dir: &Path,
    default_mirror: Option<&Path>,
) -> Option<PathBuf> {
    let ReadOutcome::Content {
        bytes,
        unsafe_reason: None,
        ..
    } = io::read_file(&io::file_path(data_dir), data_dir)
    else {
        return None;
    };
    match parse(&bytes).ok()?.mirror_folder {
        Field::Value(path) => check_folder(&path, data_dir).ok(),
        Field::Default => default_mirror.map(Path::to_path_buf),
        Field::Absent => None,
    }
}

/// В транзакции восстановления: доступ агента выключен, видимых чатов нет (режим остаётся из копии), хэша файла нет.
pub(crate) fn reset_access_after_restore(conn: &Connection) -> rusqlite::Result<()> {
    put(conn, KEY_ENABLED, "0")?;
    put(conn, KEY_CHATS, "[]")?;
    conn.execute("DELETE FROM app_setting WHERE key = ?1", [KEY_FILE_HASH])?;
    Ok(())
}
