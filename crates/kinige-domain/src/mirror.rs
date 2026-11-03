//! Markdown copy (PRD §2.1): the contents of the file of one day `Kinige/<chat>/YYYY/YYYY-MM-DD.md`. The format does not
//! depend on the interface language: ISO date, 24-hour time. Writing the file and the schedule are in `kinige-store`.
//!
//! ```text
//! # 2026-10-21
//!
//! ## 09:12
//!
//! Buy **milk**
//! - [ ] call mom
//!
//! ## 09:14
//! …
//! ```
//!
//! A reply to an entry (slice 5) is marked with a `↩` line under the time heading: `↩ [YYYY-MM-DD HH:MM](<path to the day
//! file of the old entry>)`. The format does not depend on the interface language.
//!
//! The file structure rests on first- and second-level headings. Headings inside entries (ATX `# …` and underlined
//! `===` / `---`) are shifted two levels down (not deeper than the sixth), so that an entry does not split the file into "entries";
//! code in ``` and ~~~ fences stays as is.

use crate::journal::is_journal_date;

/// An entry for the day file: the time is already converted to the entry's time zone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MirrorEntry {
    /// `HH:MM`, 24 hours.
    pub time: String,
    pub body_md: String,
    /// The entry is a reply: to what (slice 5).
    pub reply: Option<MirrorReply>,
}

/// The entry that was replied to, as it appears in the day file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MirrorReply {
    /// The old entry does not exist (deleted forever or not yet arrived): a `↩` line without a date and a link.
    Gone,
    /// The old entry exists: its day, time and the path to the file of its day relative to the reply file. There is no
    /// path if the file is not in the copy (the entry is in the trash): the line stays without a link.
    Original {
        /// `YYYY-MM-DD`.
        date: String,
        /// `HH:MM`.
        time: String,
        link: Option<String>,
    },
}

/// Chat folder name: for Inbox it is `Inbox` in any interface language (the path does not change with the language).
pub const INBOX_FOLDER: &str = "Inbox";

/// The path of the day file relative to the copy folder: segments `[<chat>, <year>, <YYYY-MM-DD>.md]`. `None` if the date
/// is not in the `YYYY-MM-DD` format (nothing but digits may get into the path).
pub fn day_file_segments(chat_folder: &str, journal_date: &str) -> Option<[String; 3]> {
    if !is_journal_date(journal_date)
        || chat_folder.is_empty()
        || chat_folder.contains(['/', '\\', '\0'])
        || chat_folder == ".."
        || chat_folder == "."
    {
        return None;
    }
    Some([
        chat_folder.to_string(),
        journal_date.get(..4)?.to_string(),
        format!("{journal_date}.md"),
    ])
}

/// The path to the day file `to` from the day file of chat `from_chat_folder` (both are `day_file_segments`), for the `↩`
/// link: the same chat -- `../<year>/<file>`, another one -- `../../<chat>/<year>/<file>`. The reply file lives in
/// `<chat>/<year>/`.
pub fn relative_day_link(from_chat_folder: &str, to: &[String; 3]) -> String {
    if to[0] == from_chat_folder {
        format!("../{}/{}", to[1], to[2])
    } else {
        format!("../../{}/{}/{}", to[0], to[1], to[2])
    }
}

/// The contents of the day file. Entries are in the order `(created_at, id)`, as in the feed.
pub fn render_day_file(journal_date: &str, entries: &[MirrorEntry]) -> String {
    let mut out = format!("# {journal_date}\n");
    for entry in entries {
        out.push_str(&format!("\n## {}\n\n", entry.time));
        if let Some(reply) = &entry.reply {
            out.push_str(&reply_line(reply));
            out.push_str("\n\n");
        }
        out.push_str(&demote_headings(&entry.body_md));
        out.push('\n');
    }
    out
}

/// The `↩` line of a reply. The address is in angle brackets: a chat folder name may contain spaces and parentheses (`<`, `>` and `/` are already stripped from names).
fn reply_line(reply: &MirrorReply) -> String {
    match reply {
        MirrorReply::Gone => "↩".to_string(),
        MirrorReply::Original {
            date,
            time,
            link: Some(link),
        } => format!("↩ [{date} {time}](<{link}>)"),
        MirrorReply::Original {
            date,
            time,
            link: None,
        } => format!("↩ {date} {time}"),
    }
}

/// Shifts headings inside an entry two levels down (ATX) and turns underlined ones into ATX (levels 3 and 4).
/// An underlined heading over a multi-line paragraph becomes a heading of the last line only: the file structure
/// does not break, no text is lost.
fn demote_headings(body: &str) -> String {
    let lines: Vec<&str> = body.split('\n').collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut fence: Option<(char, usize)> = None;
    let mut i = 0;
    while let Some(&line) = lines.get(i) {
        if let Some((marker, length)) = fence {
            if is_closing_fence(line, marker, length) {
                fence = None;
            }
            out.push(line.to_string());
            i += 1;
            continue;
        }
        if let Some(opened) = opening_fence(line) {
            fence = Some(opened);
            out.push(line.to_string());
            i += 1;
            continue;
        }
        if let Some(shifted) = demote_atx(line) {
            out.push(shifted);
            i += 1;
            continue;
        }
        if let Some(underline) = lines.get(i + 1).and_then(|next| setext_level(next))
            && is_plain_paragraph_line(line)
        {
            out.push(format!("{} {}", "#".repeat(underline + 2), line.trim()));
            i += 2;
            continue;
        }
        out.push(line.to_string());
        i += 1;
    }
    out.join("\n")
}

/// The start of a fenced block: up to three spaces, then three or more "```" or "~~~" (for "`" the info string has no "`").
pub(crate) fn opening_fence(line: &str) -> Option<(char, usize)> {
    let rest = strip_indent(line)?;
    let marker = rest.chars().next().filter(|c| *c == '`' || *c == '~')?;
    let length = rest.chars().take_while(|c| *c == marker).count();
    if length < 3 || (marker == '`' && rest.get(length..).unwrap_or_default().contains('`')) {
        return None;
    }
    Some((marker, length))
}

pub(crate) fn is_closing_fence(line: &str, marker: char, length: usize) -> bool {
    let Some(rest) = strip_indent(line) else {
        return false;
    };
    let run = rest.chars().take_while(|c| *c == marker).count();
    run >= length && rest.get(run..).unwrap_or_default().trim().is_empty()
}

/// The line without its 0-3 space indent; `None` if the indent is larger (that is a code block, not markup).
fn strip_indent(line: &str) -> Option<&str> {
    let spaces = line.bytes().take_while(|b| *b == b' ').count();
    if spaces > 3 {
        return None;
    }
    line.get(spaces..)
}

/// An ATX heading `#`…`######` + a space or end of line -> the same heading two levels deeper (not deeper than the sixth).
fn demote_atx(line: &str) -> Option<String> {
    let rest = strip_indent(line)?;
    let hashes = rest.bytes().take_while(|b| *b == b'#').count();
    if !(1..=6).contains(&hashes) {
        return None;
    }
    let after = rest.get(hashes..)?;
    if !(after.is_empty() || after.starts_with(' ') || after.starts_with('\t')) {
        return None;
    }
    let indent = line.get(..line.len().saturating_sub(rest.len()))?;
    Some(format!(
        "{indent}{}{after}",
        "#".repeat((hashes + 2).min(6))
    ))
}

/// Heading underline: `===` is level 1, `---` is level 2.
fn setext_level(line: &str) -> Option<usize> {
    let rest = strip_indent(line)?.trim_end();
    if !rest.is_empty() && rest.bytes().all(|b| b == b'=') {
        Some(1)
    } else if !rest.is_empty() && rest.bytes().all(|b| b == b'-') {
        Some(2)
    } else {
        None
    }
}

/// A line of a plain paragraph: not empty and not the start of a list, quote, heading, table or fence.
fn is_plain_paragraph_line(line: &str) -> bool {
    let Some(rest) = strip_indent(line) else {
        return false;
    };
    let rest = rest.trim_end();
    if rest.is_empty() {
        return false;
    }
    let first = rest.chars().next().unwrap_or(' ');
    if matches!(
        first,
        '-' | '*' | '+' | '>' | '#' | '|' | '`' | '~' | '=' | '<'
    ) {
        return false;
    }
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    !(digits > 0
        && matches!(
            rest.get(digits..).unwrap_or_default().chars().next(),
            Some('.' | ')')
        ))
}

#[cfg(test)]
mod tests;
