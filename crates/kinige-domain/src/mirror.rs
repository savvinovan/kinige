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

/// Содержимое файла дня. Записи — в порядке `(created_at, id)`, как в ленте.
pub fn render_day_file(journal_date: &str, entries: &[MirrorEntry]) -> String {
    let mut out = format!("# {journal_date}\n");
    for entry in entries {
        out.push_str(&format!("\n## {}\n\n", entry.time));
        out.push_str(&demote_headings(&entry.body_md));
        out.push('\n');
    }
    out
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
mod tests {
    use super::*;

    fn entry(time: &str, body: &str) -> MirrorEntry {
        MirrorEntry {
            time: time.to_string(),
            body_md: body.to_string(),
        }
    }

    #[test]
    fn day_file_has_iso_date_and_24_hour_time_headings() {
        let text = render_day_file(
            "2026-10-21",
            &[
                entry("09:12", "Купить **молоко**\n- [ ] позвонить маме"),
                entry("21:05", "Вечером: код"),
            ],
        );
        assert_eq!(
            text,
            "# 2026-10-21\n\n## 09:12\n\nКупить **молоко**\n- [ ] позвонить маме\n\n## 21:05\n\nВечером: код\n"
        );
    }

    #[test]
    fn empty_day_has_only_the_date_heading() {
        assert_eq!(render_day_file("2026-10-21", &[]), "# 2026-10-21\n");
    }

    #[test]
    fn atx_headings_inside_entries_are_demoted_below_the_entry_level() {
        let body = "# Заголовок\nтекст\n## Второй\n### Третий\n#### Четвёртый\n##### Пятый\n###### Шестой\n#не заголовок\n#";
        assert_eq!(
            demote_headings(body),
            "### Заголовок\nтекст\n#### Второй\n##### Третий\n###### Четвёртый\n###### Пятый\n###### Шестой\n#не заголовок\n###"
        );
    }

    #[test]
    fn entry_cannot_fake_another_entry() {
        let text = render_day_file(
            "2026-10-21",
            &[entry("09:12", "## 10:00\nподделка\n# 2026-01-01")],
        );
        let headings: Vec<&str> = text
            .lines()
            .filter(|l| l.starts_with("# ") || l.starts_with("## "))
            .collect();
        assert_eq!(headings, ["# 2026-10-21", "## 09:12"]);
    }

    #[test]
    fn setext_headings_become_atx_below_the_entry_level() {
        assert_eq!(
            demote_headings("Название\n=======\nтекст"),
            "### Название\nтекст"
        );
        assert_eq!(
            demote_headings("Название\n---\nтекст"),
            "#### Название\nтекст"
        );
        let text = render_day_file("2026-10-21", &[entry("09:12", "Название\n===\nтекст")]);
        assert!(text.lines().all(|l| !l.starts_with("===")));
        assert_eq!(
            text.lines()
                .filter(|l| l.starts_with("# ") || l.starts_with("## "))
                .count(),
            2
        );
    }

    #[test]
    fn thematic_break_and_lists_are_not_mistaken_for_setext() {
        assert_eq!(
            demote_headings("абзац\n\n---\nещё"),
            "абзац\n\n---\nещё",
            "после пустой строки это разделитель"
        );
        assert_eq!(demote_headings("- пункт\n---"), "- пункт\n---");
        assert_eq!(demote_headings("1. пункт\n---"), "1. пункт\n---");
        assert_eq!(demote_headings("> цитата\n==="), "> цитата\n===");
    }

    #[test]
    fn fenced_code_is_left_untouched() {
        let body = "до\n```sh\n# комментарий\nТитул\n===\n```\n# после";
        assert_eq!(
            demote_headings(body),
            "до\n```sh\n# комментарий\nТитул\n===\n```\n### после"
        );
        let tilde = "~~~\n# внутри\n~~~\n# снаружи";
        assert_eq!(demote_headings(tilde), "~~~\n# внутри\n~~~\n### снаружи");
    }

    #[test]
    fn fence_closes_only_with_a_long_enough_matching_marker() {
        let body = "````\n```\n# всё ещё внутри\n````\n# снаружи";
        assert_eq!(
            demote_headings(body),
            "````\n```\n# всё ещё внутри\n````\n### снаружи"
        );
        // Незакрытое ограждение до конца записи: заголовки внутри не трогаем (как делает рендерер Markdown).
        assert_eq!(demote_headings("```\n# внутри"), "```\n# внутри");
    }

    #[test]
    fn indented_code_is_not_a_heading() {
        assert_eq!(demote_headings("    # код"), "    # код");
        assert_eq!(demote_headings("   # заголовок"), "   ### заголовок");
    }

    #[test]
    fn paths_use_chat_folder_year_and_date() {
        assert_eq!(
            day_file_segments(INBOX_FOLDER, "2026-10-21"),
            Some([
                "Inbox".to_string(),
                "2026".to_string(),
                "2026-10-21.md".to_string()
            ])
        );
    }

    #[test]
    fn paths_reject_anything_but_a_plain_date_and_folder() {
        for date in ["", "../../x", "2026-10-21/../..", "2026-1-1"] {
            assert_eq!(day_file_segments("Inbox", date), None, "{date}");
        }
        for folder in ["", "..", ".", "a/b", "a\\b"] {
            assert_eq!(day_file_segments(folder, "2026-10-21"), None, "{folder}");
        }
    }
}
