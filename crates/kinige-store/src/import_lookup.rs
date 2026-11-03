//! Small lookups of the Telegram import (`import.rs`): what the export holds and what the database already has of it.

use crate::StoreError;
use crate::error::read_err;
use kinige_domain::Language;
use kinige_domain::chat::{MAX_TITLE_CHARS, normalize_title};
use kinige_domain::import::{EXT_SRC_TELEGRAM, ImportChatKind};
use kinige_domain::telegram::{
    TgChat, TgChatKind, TgMessage, render_body, render_body_attached,
};
use rusqlite::{Connection, OptionalExtension};
use std::collections::{HashMap, HashSet};

pub(crate) fn supported_kind(chat: &TgChat) -> Option<ImportChatKind> {
    match chat.kind {
        TgChatKind::Saved => Some(ImportChatKind::Saved),
        TgChatKind::Personal => Some(ImportChatKind::Personal),
        TgChatKind::Other(_) => None,
    }
}

/// Already imported messages of a Telegram chat: `ext_msg -> Kinige entry id`.
pub(crate) fn imported_messages(
    conn: &Connection,
    ext_chat: &str,
) -> Result<HashMap<String, String>, StoreError> {
    let mut statement = conn
        .prepare("SELECT ext_msg, message_id FROM message_ext WHERE src = ?1 AND ext_chat = ?2")
        .map_err(read_err)?;
    let rows = statement
        .query_map((EXT_SRC_TELEGRAM, ext_chat), |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(read_err)?;
    rows.collect::<Result<_, _>>().map_err(read_err)
}

/// The entries of a Telegram chat that already have an attachment, live or in the trash (slice 7): their files are not attached again.
pub(crate) fn entries_with_files(
    conn: &Connection,
    ext_chat: &str,
) -> Result<HashSet<String>, StoreError> {
    let mut statement = conn
        .prepare(
            "SELECT DISTINCT a.message_id FROM attachment a JOIN message_ext e ON e.message_id = a.message_id \
             WHERE e.src = ?1 AND e.ext_chat = ?2",
        )
        .map_err(read_err)?;
    let rows = statement
        .query_map((EXT_SRC_TELEGRAM, ext_chat), |row| row.get::<_, String>(0))
        .map_err(read_err)?;
    rows.collect::<Result<_, _>>().map_err(read_err)
}

/// The personal-conversation chat on a repeated import is the chat of the freshest live imported entry of this Telegram
/// chat (decision 5): a not deleted entry in a not deleted chat. None -- `None`, the first batch with entries creates the chat.
pub(crate) fn find_import_chat(
    conn: &Connection,
    ext_chat: &str,
) -> Result<Option<String>, StoreError> {
    conn.query_row(
        "SELECT m.chat_id FROM message_ext e \
         JOIN message m ON m.id = e.message_id JOIN chat c ON c.id = m.chat_id \
         WHERE e.src = ?1 AND e.ext_chat = ?2 AND m.deleted_at IS NULL AND c.deleted_at IS NULL \
         ORDER BY m.created_at DESC, m.id DESC LIMIT 1",
        (EXT_SRC_TELEGRAM, ext_chat),
        |row| row.get(0),
    )
    .optional()
    .map_err(read_err)
}

/// The Kinige chat name for a personal conversation: from the export; no name -- `Telegram <id>`; longer than the limit -- cut.
pub(crate) fn chat_title(chat: &TgChat) -> String {
    let fallback = format!("Telegram {}", chat.id);
    let Some(name) = chat.name.as_deref() else {
        return fallback;
    };
    let cut: String = name.trim().chars().take(MAX_TITLE_CHARS).collect();
    normalize_title(&cut).unwrap_or(fallback)
}

/// Author: for the other person's message in a personal chat (`from_id` = `user<chat id>`, decision 6) -- the name; own messages and "Saved Messages" -- no name.
pub(crate) fn author_of<'a>(chat: &'a TgChat, message: &'a TgMessage) -> Option<&'a str> {
    if chat.kind != TgChatKind::Personal {
        return None;
    }
    let theirs = message.from_id.as_deref() == Some(format!("user{}", chat.id).as_str());
    if !theirs {
        return None;
    }
    message.from.as_deref().or(chat.name.as_deref())
}

/// The file a message holds that the export says is in the folder: `(count, bytes)` when it is, `None` when the message has no
/// file at all.
pub(crate) fn file_of(message: &TgMessage) -> Option<(bool, u64)> {
    let file = message.file.as_ref()?;
    Some((file.path.is_some(), file.size.unwrap_or(0)))
}

/// The text of an old entry with and without its placeholder line, in both languages: the entry is found unedited if its text is
/// the first of a pair (ADR-0014).
pub(crate) fn renderings(message: &TgMessage, author: Option<&str>) -> Vec<(String, String)> {
    [Language::Ru, Language::En]
        .into_iter()
        .filter_map(|language| {
            Some((
                render_body(message, author, language)?,
                render_body_attached(message, author, language)?,
            ))
        })
        .collect()
}
