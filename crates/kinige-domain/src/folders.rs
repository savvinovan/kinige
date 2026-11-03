//! Chat folders in the Markdown copy (PRD §2.1, slice 3): `<copy folder>/<chat name>/YYYY/YYYY-MM-DD.md`. The folder name is the chat name
//! cleaned of characters not allowed in file names. The Inbox folder is `Inbox` in any interface language and with any names
//! of other chats. Equal names (case-insensitive: the macOS file system does not distinguish case) are told apart by a suffix
//! taken from the start of the chat id; the name without a suffix stays with the oldest chat.

use crate::groups::INBOX_ID;
use crate::mirror::INBOX_FOLDER;
use std::collections::{BTreeMap, BTreeSet};

/// How many characters of the chat name go into the folder name.
pub const MAX_FOLDER_CHARS: usize = 80;
/// The folder name for a chat with an empty or entirely invalid name.
pub const FALLBACK_FOLDER: &str = "Chat";
/// How many characters of the id go into the suffix (later, if even that matches, the suffix is lengthened).
const SUFFIX_CHARS: usize = 8;

/// A chat for the folder calculation.
#[derive(Debug, Clone, Copy)]
pub struct FolderChat<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub created_at: i64,
}

/// A name from the chat title: invalid characters (`/ \ : * ? " < > |` and control ones) are replaced with `_`, surrounding spaces and dots
/// are removed, the length is limited. An empty result becomes [`FALLBACK_FOLDER`].
pub fn sanitize_folder_name(title: &str) -> String {
    let outer = title.trim_matches(|c: char| c.is_whitespace() || c == '.');
    let replaced: String = outer
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '_'
            } else {
                c
            }
        })
        .collect();
    let trimmed = replaced.trim_matches(|c: char| c.is_whitespace() || c == '.');
    let limited: String = trimmed.chars().take(MAX_FOLDER_CHARS).collect();
    let limited = limited.trim_end_matches(|c: char| c.is_whitespace() || c == '.');
    if limited.is_empty() {
        FALLBACK_FOLDER.to_string()
    } else if cfg!(windows) {
        avoid_windows_device_name(limited)
    } else {
        limited.to_string()
    }
}

/// Windows does not create files and folders named like devices (`CON`, `NUL`, `COM1`, `COM¹`, ...), and an extension
/// does not help: `NUL.txt` is the same device. The `_` goes after the stem (the part before the first dot): `CON` ->
/// `CON_`, `con.txt` -> `con_.txt`, `Nul.tar.gz` -> `Nul_.tar.gz`. Only on Windows: elsewhere the name stays as it is,
/// so the folders of existing copies are not renamed.
fn avoid_windows_device_name(name: &str) -> String {
    const DEVICES: [&str; 4] = ["CON", "PRN", "AUX", "NUL"];
    let (stem, tail) = match name.split_once('.') {
        Some((stem, tail)) => (stem, Some(tail)),
        None => (name, None),
    };
    let upper = stem.trim_end().to_ascii_uppercase();
    // COM0-COM9 and LPT0-LPT9, with the superscript digits 1-3 too.
    let numbered = |prefix: &str| {
        upper.strip_prefix(prefix).is_some_and(|rest| {
            let mut chars = rest.chars();
            matches!(
                (chars.next(), chars.next()),
                (Some('0'..='9' | '¹' | '²' | '³'), None)
            )
        })
    };
    if !(DEVICES.contains(&upper.as_str()) || numbered("COM") || numbered("LPT")) {
        return name.to_string();
    }
    match tail {
        Some(tail) => format!("{stem}_.{tail}"),
        None => format!("{stem}_"),
    }
}

/// The folder of each chat. The result does not depend on the order of `chats`.
pub fn chat_folders(chats: &[FolderChat<'_>]) -> BTreeMap<String, String> {
    let mut result = BTreeMap::new();
    let mut taken: BTreeSet<String> = BTreeSet::new();
    // Inbox takes its name first.
    if chats.iter().any(|chat| chat.id == INBOX_ID) {
        result.insert(INBOX_ID.to_string(), INBOX_FOLDER.to_string());
    }
    taken.insert(INBOX_FOLDER.to_lowercase());

    let mut others: Vec<&FolderChat<'_>> = chats.iter().filter(|c| c.id != INBOX_ID).collect();
    others.sort_by(|a, b| (a.created_at, a.id).cmp(&(b.created_at, b.id)));
    for chat in others {
        let base = sanitize_folder_name(chat.title);
        let mut name = base.clone();
        let id_len = chat.id.chars().count();
        let (mut suffix, mut number, mut whole_id_tried) = (SUFFIX_CHARS, 1, false);
        while taken.contains(&name.to_lowercase()) {
            let id_start: String = chat.id.chars().take(suffix).collect();
            name = format!("{base} ({id_start})");
            if whole_id_tried {
                // The id is exhausted (equal ids in one database are impossible): tell apart by a number.
                name = format!("{name} {number}");
                number += 1;
            }
            whole_id_tried |= suffix >= id_len;
            suffix += SUFFIX_CHARS;
        }
        taken.insert(name.to_lowercase());
        result.insert(chat.id.to_string(), name);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chat<'a>(id: &'a str, title: &'a str, created_at: i64) -> FolderChat<'a> {
        FolderChat {
            id,
            title,
            created_at,
        }
    }

    #[test]
    fn names_are_cleaned_for_the_file_system() {
        assert_eq!(sanitize_folder_name("Книги"), "Книги");
        assert_eq!(sanitize_folder_name("  Идеи / заметки  "), "Идеи _ заметки");
        assert_eq!(
            sanitize_folder_name("a:b*c?d\"e<f>g|h\\i"),
            "a_b_c_d_e_f_g_h_i"
        );
        assert_eq!(sanitize_folder_name("..."), FALLBACK_FOLDER);
        assert_eq!(sanitize_folder_name(".."), FALLBACK_FOLDER);
        assert_eq!(sanitize_folder_name(""), FALLBACK_FOLDER);
        assert_eq!(sanitize_folder_name("\n\t"), FALLBACK_FOLDER);
        assert_eq!(sanitize_folder_name(".скрытая."), "скрытая");
        assert_eq!(
            sanitize_folder_name(&"я".repeat(200)).chars().count(),
            MAX_FOLDER_CHARS
        );
        assert_eq!(sanitize_folder_name("😀 Смайлы"), "😀 Смайлы");
    }

    #[test]
    fn windows_device_names_get_an_underscore_after_the_stem_and_ordinary_names_do_not() {
        for (device, safe) in [
            ("con", "con_"),
            ("CON", "CON_"),
            ("Aux", "Aux_"),
            ("nul", "nul_"),
            ("PRN", "PRN_"),
            ("com1", "com1_"),
            ("LPT9", "LPT9_"),
            ("COM\u{b9}", "COM\u{b9}_"),
            ("lpt\u{b3}", "lpt\u{b3}_"),
            ("con.txt", "con_.txt"),
            ("Nul.tar.gz", "Nul_.tar.gz"),
            ("COM\u{b2}.log", "COM\u{b2}_.log"),
        ] {
            assert_eq!(avoid_windows_device_name(device), safe, "{device}");
        }
        for fine in [
            "Console",
            "COM",
            "COM10",
            "LPT",
            "com1a",
            "Книги",
            "a.con",
            "Inbox",
            "Cons",
        ] {
            assert_eq!(avoid_windows_device_name(fine), fine, "{fine}");
        }
    }

    #[cfg(windows)]
    #[test]
    fn on_windows_a_chat_named_like_a_device_gets_a_usable_folder_name() {
        assert_eq!(sanitize_folder_name("CON"), "CON_");
        assert_eq!(sanitize_folder_name("con.txt"), "con_.txt");
        assert_eq!(sanitize_folder_name("Console"), "Console");
    }

    #[cfg(not(windows))]
    #[test]
    fn off_windows_a_chat_named_like_a_device_keeps_its_folder_name() {
        assert_eq!(sanitize_folder_name("CON"), "CON");
    }

    #[test]
    fn inbox_is_always_inbox_and_nobody_else_takes_the_name() {
        let folders = chat_folders(&[
            chat(INBOX_ID, "", 0),
            chat("11111111-aaaa", "inbox", 5),
            chat("22222222-bbbb", "INBOX", 6),
        ]);
        assert_eq!(folders[INBOX_ID], "Inbox");
        assert_eq!(folders["11111111-aaaa"], "inbox (11111111)");
        assert_eq!(folders["22222222-bbbb"], "INBOX (22222222)");
    }

    #[test]
    fn same_names_get_an_id_suffix_and_the_oldest_keeps_the_plain_name() {
        let chats = [
            chat("bbbbbbbb-0000", "Книги", 20),
            chat("aaaaaaaa-0000", "Книги", 10),
            chat("cccccccc-0000", "книги", 30),
        ];
        let folders = chat_folders(&chats);
        assert_eq!(folders["aaaaaaaa-0000"], "Книги");
        assert_eq!(folders["bbbbbbbb-0000"], "Книги (bbbbbbbb)");
        assert_eq!(folders["cccccccc-0000"], "книги (cccccccc)");
        let mut reversed = chats;
        reversed.reverse();
        assert_eq!(
            chat_folders(&reversed),
            folders,
            "the input order does not matter"
        );
    }

    #[test]
    fn renaming_the_oldest_chat_gives_the_plain_name_to_the_next() {
        let before = chat_folders(&[chat("a-1", "Книги", 1), chat("b-2", "Книги", 2)]);
        assert_eq!(before["b-2"], "Книги (b-2)");
        let after = chat_folders(&[chat("a-1", "Фильмы", 1), chat("b-2", "Книги", 2)]);
        assert_eq!(after["b-2"], "Книги");
    }
}
