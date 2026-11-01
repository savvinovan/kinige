//! Markdown-копия (ТЗ §2.1): содержимое файла одного дня `Kinige/<чат>/YYYY/YYYY-MM-DD.md`. Формат не зависит от языка
//! интерфейса: дата ISO, время 24 часа. Запись файла и расписание — в `kinige-store`.
//!
//! ```text
//! # 2026-10-21
//!
//! ## 09:12
//!
//! Купить **молоко**
//! - [ ] позвонить маме
//!
//! ## 09:14
//! …
//! ```
//!
//! Ответ на запись (слайс 5) отмечен строкой `↩` под заголовком времени: `↩ [ГГГГ-ММ-ДД ЧЧ:ММ](<путь к файлу дня старой
//! записи>)`. Формат не зависит от языка интерфейса.
//!
//! Структура файла держится на заголовках первого и второго уровня. Заголовки внутри записей (ATX `# …` и
//! подчёркнутые `===` / `---`) сдвигаются на два уровня вниз (не глубже шестого), чтобы запись не разрывала файл на «записи»;
//! код в ограждениях ``` и ~~~ остаётся как есть.

use crate::journal::is_journal_date;

/// Запись для файла дня: время уже переведено в часовой пояс записи.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MirrorEntry {
    /// `HH:MM`, 24 часа.
    pub time: String,
    pub body_md: String,
    /// Запись — ответ: на что (слайс 5).
    pub reply: Option<MirrorReply>,
}

/// Запись, на которую ответили, как её видно в файле дня.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MirrorReply {
    /// Старой записи нет (удалена навсегда или ещё не пришла): строка `↩` без даты и ссылки.
    Gone,
    /// Старая запись есть: её день, время и путь к файлу её дня относительно файла ответа. Пути нет, если файла в копии
    /// нет (запись в корзине): строка остаётся без ссылки.
    Original {
        /// `YYYY-MM-DD`.
        date: String,
        /// `HH:MM`.
        time: String,
        link: Option<String>,
    },
}

/// Имя папки чата: у Inbox — `Inbox` при любом языке интерфейса (путь не меняется от языка).
pub const INBOX_FOLDER: &str = "Inbox";

/// Путь файла дня относительно папки копии: сегменты `[<чат>, <год>, <YYYY-MM-DD>.md]`. `None`, если дата не в формате
/// `YYYY-MM-DD` (в путь не должно попадать ничего, кроме цифр).
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
        journal_date[..4].to_string(),
        format!("{journal_date}.md"),
    ])
}

/// Путь к файлу дня `to` из файла дня чата `from_chat_folder` (оба — `day_file_segments`), для ссылки `↩`: тот же чат —
/// `../<год>/<файл>`, другой — `../../<чат>/<год>/<файл>`. Файл ответа лежит в `<чат>/<год>/`.
pub fn relative_day_link(from_chat_folder: &str, to: &[String; 3]) -> String {
    if to[0] == from_chat_folder {
        format!("../{}/{}", to[1], to[2])
    } else {
        format!("../../{}/{}/{}", to[0], to[1], to[2])
    }
}

/// Содержимое файла дня. Записи — в порядке `(created_at, id)`, как в ленте.
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

/// Строка `↩` ответа. Адрес в угловых скобках: в имени папки чата бывают пробелы и скобки (`<`, `>` и `/` в именах уже вычищены).
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

/// Сдвигает заголовки внутри записи на два уровня вниз (ATX) и превращает подчёркнутые в ATX (уровни 3 и 4).
/// Подчёркнутый заголовок над абзацем из нескольких строк превращается в заголовок только из последней строки: структура
/// файла не ломается, текст не теряется.
fn demote_headings(body: &str) -> String {
    let lines: Vec<&str> = body.split('\n').collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut fence: Option<(char, usize)> = None;
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
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

/// Начало ограждённого блока: до трёх пробелов, затем три и более «```» или «~~~» (у «`» в строке информации нет «`»).
pub(crate) fn opening_fence(line: &str) -> Option<(char, usize)> {
    let rest = strip_indent(line)?;
    let marker = rest.chars().next().filter(|c| *c == '`' || *c == '~')?;
    let length = rest.chars().take_while(|c| *c == marker).count();
    if length < 3 || (marker == '`' && rest[length..].contains('`')) {
        return None;
    }
    Some((marker, length))
}

pub(crate) fn is_closing_fence(line: &str, marker: char, length: usize) -> bool {
    let Some(rest) = strip_indent(line) else {
        return false;
    };
    let run = rest.chars().take_while(|c| *c == marker).count();
    run >= length && rest[run..].trim().is_empty()
}

/// Строка без отступа в 0–3 пробела; `None`, если отступ больше (это блок кода, не разметка).
fn strip_indent(line: &str) -> Option<&str> {
    let spaces = line.bytes().take_while(|b| *b == b' ').count();
    (spaces <= 3).then(|| &line[spaces..])
}

/// ATX-заголовок `#`…`######` + пробел или конец строки → тот же заголовок на два уровня глубже (не глубже шестого).
fn demote_atx(line: &str) -> Option<String> {
    let rest = strip_indent(line)?;
    let hashes = rest.bytes().take_while(|b| *b == b'#').count();
    if !(1..=6).contains(&hashes) {
        return None;
    }
    let after = &rest[hashes..];
    if !(after.is_empty() || after.starts_with(' ') || after.starts_with('\t')) {
        return None;
    }
    let indent = &line[..line.len() - rest.len()];
    Some(format!(
        "{indent}{}{after}",
        "#".repeat((hashes + 2).min(6))
    ))
}

/// Подчёркивание заголовка: `===` — уровень 1, `---` — уровень 2.
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

/// Строка обычного абзаца: не пустая и не начало списка, цитаты, заголовка, таблицы или ограждения.
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
    !(digits > 0 && matches!(rest[digits..].chars().next(), Some('.' | ')')))
}

#[cfg(test)]
mod tests;
