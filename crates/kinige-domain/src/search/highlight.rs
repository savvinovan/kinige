//! Подсветка найденных слов и фрагмент для строки результата (слайс 4, задача 4.5).

use super::{Query, normalize, stem, words};
use crate::mirror::{is_closing_fence, opening_fence};

/// Не больше стольких различных слов отдаёт [`highlight_terms`]: префикс из двух знаков в длинной записи совпадает с сотнями слов.
const MAX_TERMS: usize = 64;

/// Различные слова текста записи (как они в нём написаны, с регистром), которые совпали с запросом по правилам
/// [`super::fts_query`]: основа слова равна основе терма, а у последнего терма — начинается с неё, если слово не закончено.
/// Фронтенд подсвечивает эти слова как целые.
pub fn highlight_terms(body_md: &str, input: &str) -> Vec<String> {
    let Some(query) = Query::parse(input) else {
        return Vec::new();
    };
    let mut found: Vec<String> = Vec::new();
    for (original, word) in words(body_md) {
        if found.len() >= MAX_TERMS {
            break;
        }
        if query.matches(&stem(&word)) && !found.iter().any(|f| f == original) {
            found.push(original.to_string());
        }
    }
    found
}

/// Простой текст вокруг первого слова из `terms` (регистр и «ё» не важны), не длиннее `max_chars` знаков вместе с «…»
/// по обрезанным краям. Разметка Markdown снята, переводы строк стали пробелами. Режет только по границе слова
/// (одно слово длиннее остатка режется по знаку) и по границе знака UTF-8. Совпадения нет — начало текста.
pub fn snippet(body_md: &str, terms: &[String], max_chars: usize) -> String {
    let text = plain_text(body_md);
    let chars: Vec<char> = text.chars().collect();
    if max_chars == 0 {
        return String::new();
    }
    if chars.len() <= max_chars {
        return text;
    }
    // Под два «…» и хоть один знак окна места нет: просто начало текста.
    if max_chars < 3 {
        return chars.iter().take(max_chars).collect();
    }
    let wanted: Vec<String> = terms.iter().map(|term| normalize(term)).collect();
    let first_match = words(&text)
        .find(|(_, word)| wanted.contains(word))
        .map(|(original, _)| {
            text[..original.as_ptr() as usize - text.as_ptr() as usize]
                .chars()
                .count()
        });
    let hit = first_match.unwrap_or(0);

    // Начало окна: треть окна контекста до совпадения, сдвинутая вперёд к началу слова.
    let mut start = hit.saturating_sub(max_chars / 3);
    while start > 0
        && start < hit
        && (chars[start - 1].is_alphanumeric() || chars[start].is_whitespace())
    {
        start += 1;
    }
    let lead = usize::from(start > 0);
    // Конец окна. Если текст не помещается, один знак уходит под «…» в конце.
    let mut end = (start + max_chars - lead).min(chars.len());
    if end < chars.len() {
        end = (start + max_chars - lead - 1).min(chars.len());
    }
    if end < chars.len() {
        let mut cut = end;
        while cut > start && chars[cut - 1].is_alphanumeric() && chars[cut].is_alphanumeric() {
            cut -= 1;
        }
        // Одно слово длиннее окна: режем по знаку.
        if cut > start {
            end = cut;
        }
    }
    let trail = end < chars.len();
    let body: String = chars[start..end].iter().collect();
    format!(
        "{}{}{}",
        if lead == 1 { "…" } else { "" },
        body.trim_end(),
        if trail { "…" } else { "" }
    )
}

/// Markdown записи → простой текст в одну строку: без маркеров списков, цитат, заголовков, оград кода, ссылок и
/// выделения. Не разбор Markdown, а вырезание разметки для фрагмента.
fn plain_text(body_md: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut fence: Option<(char, usize)> = None;
    for line in body_md.split('\n') {
        if let Some((marker, length)) = fence {
            if is_closing_fence(line, marker, length) {
                fence = None;
            } else {
                lines.push(line.to_string());
            }
            continue;
        }
        if let Some(opened) = opening_fence(line) {
            fence = Some(opened);
            continue;
        }
        lines.push(inline(strip_line_markers(line)));
    }
    lines
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Цитата, заголовок, маркер списка и квадрат задачи в начале строки.
fn strip_line_markers(line: &str) -> &str {
    let mut rest = line.trim_start();
    while let Some(after) = rest.strip_prefix('>') {
        rest = after.trim_start();
    }
    let hashes = rest.chars().take_while(|c| *c == '#').count();
    if (1..=6).contains(&hashes) && rest[hashes..].starts_with([' ', '\t']) {
        rest = rest[hashes..].trim_start();
    }
    if let Some(after) = rest
        .strip_prefix(['-', '+', '*'])
        .filter(|a| a.starts_with([' ', '\t']))
    {
        rest = after.trim_start();
    } else {
        let digits = rest.chars().take_while(char::is_ascii_digit).count();
        if (1..=9).contains(&digits)
            && rest[digits..].starts_with(['.', ')'])
            && rest[digits + 1..].starts_with([' ', '\t'])
        {
            rest = rest[digits + 1..].trim_start();
        }
    }
    for task in ["[ ] ", "[x] ", "[X] "] {
        if let Some(after) = rest.strip_prefix(task) {
            return after;
        }
    }
    rest
}

/// Разметка внутри строки: экранирование, ссылки `[текст](адрес)` и `<адрес>`, обратные кавычки, `**`, `~~`, `__`, а
/// также одиночные `*` и `_` на границе слова (в середине слова, как в `snake_case`, остаются).
fn inline(line: &str) -> String {
    let chars: Vec<char> = line.chars().collect();
    let mut out = String::with_capacity(line.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let prev = if i == 0 { None } else { Some(chars[i - 1]) };
        let next = chars.get(i + 1).copied();
        match c {
            '\\' if next.is_some_and(|n| n.is_ascii_punctuation()) => {
                out.push(chars[i + 1]);
                i += 2;
                continue;
            }
            '[' => {
                if let Some(close) = matching(&chars, i, '[', ']')
                    && chars.get(close + 1) == Some(&'(')
                    && let Some(end) = matching(&chars, close + 1, '(', ')')
                {
                    let text: String = chars[i + 1..close].iter().collect();
                    out.push_str(&inline(&text));
                    i = end + 1;
                    continue;
                }
                out.push(c);
            }
            '<' => {
                // `<https://…>` — адрес без скобок.
                let close = chars[i + 1..]
                    .iter()
                    .position(|x| *x == '>')
                    .map(|p| i + 1 + p);
                match close {
                    Some(close) if chars[i + 1..close].iter().all(|x| !x.is_whitespace()) => {
                        out.extend(&chars[i + 1..close]);
                        i = close + 1;
                        continue;
                    }
                    _ => out.push(c),
                }
            }
            '`' | '~' => {}
            '*' | '_' => {
                let inside_word = prev.is_some_and(char::is_alphanumeric)
                    && next.is_some_and(char::is_alphanumeric);
                if inside_word {
                    out.push(c);
                }
            }
            _ => out.push(c),
        }
        i += 1;
    }
    out
}

/// Индекс скобки, закрывающей `open` в `start`, с учётом вложенности и экранирования.
fn matching(chars: &[char], start: usize, open: char, close: char) -> Option<usize> {
    let mut depth = 0usize;
    let mut i = start;
    while i < chars.len() {
        match chars[i] {
            '\\' => i += 1,
            c if c == open => depth += 1,
            c if c == close => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}
