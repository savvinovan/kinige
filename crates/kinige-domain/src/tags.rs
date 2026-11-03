//! Tags `#tag` (PRD §2.1, slice 3, decision 3). No IO.
//!
//! A tag is `#` followed by letters of any alphabet, digits and `_`; at least one character is not a digit (`#1` is a number, not a tag;
//! Obsidian decides the same way). Case does not matter: a tag is stored in lower case. Not a tag:
//! - inside code: fenced blocks ``` and ~~~, strings in backticks;
//! - inside links: `[text](address)`, `<http://…>`, bare addresses `https://…`, HTML tags;
//! - `#` after a letter, a digit, `_` or another `#` (`a#b`, `##x`), after `&` (the entity `&#38;`) and after `\` (escaping);
//! - `# ` with a space (a heading) -- there are no tag characters after `#`.

use crate::mirror::{is_closing_fence, opening_fence};

/// The longest tag length in characters; longer is not a tag.
pub const MAX_TAG_CHARS: usize = 64;

/// Brackets deeper than this are not counted: otherwise a line of `[a]([a](…` would be read in quadratic time (every `(` looks for its closing one up to
/// the end of the line). Real links are not nested that deep.
const MAX_NESTING: usize = 64;

fn is_tag_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// The tags of an entry in lower case, without repeats, in order of first appearance.
pub fn parse_tags(body_md: &str) -> Vec<String> {
    let mut tags: Vec<String> = Vec::new();
    let mut fence: Option<(char, usize)> = None;
    for line in body_md.split('\n') {
        if let Some((marker, length)) = fence {
            if is_closing_fence(line, marker, length) {
                fence = None;
            }
            continue;
        }
        if let Some(opened) = opening_fence(line) {
            fence = Some(opened);
            continue;
        }
        scan_line(line, &mut tags);
    }
    tags
}

fn scan_line(line: &str, tags: &mut Vec<String>) {
    let chars: Vec<char> = line.chars().collect();
    // The last `>` of the line: without it `<` does not open an address or a tag, and there is no need to look for the closing one for every `<` (otherwise a line of
    // `<a<a<a…` without `>` would be read in time quadratic in its length).
    let last_gt = chars.iter().rposition(|c| *c == '>');
    let mut i = 0;
    while let Some(&c) = chars.get(i) {
        match c {
            '`' => i = skip_code_span(&chars, i),
            '[' => i = skip_link(&chars, i),
            '<' => i = skip_angle(&chars, i, last_gt),
            '\\' => i += 2,
            '#' => {
                let before = i.checked_sub(1).and_then(|p| chars.get(p).copied());
                let allowed = before.is_none_or(|b| !(is_tag_char(b) || matches!(b, '#' | '&')));
                let after = chars.get(i + 1..).unwrap_or_default();
                let len = after.iter().take_while(|c| is_tag_char(**c)).count();
                let word = after.get(..len).unwrap_or_default();
                if allowed
                    && len > 0
                    && len <= MAX_TAG_CHARS
                    && word.iter().any(|c| !c.is_ascii_digit())
                {
                    let tag: String = word.iter().collect::<String>().to_lowercase();
                    if !tags.contains(&tag) {
                        tags.push(tag);
                    }
                }
                i += 1 + len;
            }
            _ if starts_url(&chars, i) => {
                while chars.get(i).is_some_and(|c| !c.is_whitespace()) {
                    i += 1;
                }
            }
            _ => i += 1,
        }
    }
}

/// A bare address starts at a word boundary: `https://…`, `http://…`, `mailto:…`, `www.…`.
fn starts_url(chars: &[char], i: usize) -> bool {
    if i.checked_sub(1)
        .and_then(|p| chars.get(p))
        .is_some_and(|c| is_tag_char(*c))
    {
        return false;
    }
    let rest: String = chars
        .get(i..chars.len().min(i + 8))
        .unwrap_or_default()
        .iter()
        .collect::<String>()
        .to_lowercase();
    ["http://", "https://", "mailto:", "www."]
        .iter()
        .any(|prefix| rest.starts_with(prefix))
}

/// A string in backticks: a run of n backticks and a closing run of the same length. If not closed, the backticks are plain text.
fn skip_code_span(chars: &[char], start: usize) -> usize {
    let run = chars
        .get(start..)
        .unwrap_or_default()
        .iter()
        .take_while(|c| **c == '`')
        .count();
    let mut i = start + run;
    while let Some(&c) = chars.get(i) {
        if c == '`' {
            let close = chars
                .get(i..)
                .unwrap_or_default()
                .iter()
                .take_while(|c| **c == '`')
                .count();
            if close == run {
                return i + close;
            }
            i += close;
        } else {
            i += 1;
        }
    }
    start + run
}

/// `[text](address)`: skipped whole, together with the text. Otherwise (`[ ]`, `[x]`, footnotes) `[` is a plain character.
fn skip_link(chars: &[char], start: usize) -> usize {
    let Some(close) = matching(chars, start, '[', ']') else {
        return start + 1;
    };
    if chars.get(close + 1) != Some(&'(') {
        return start + 1;
    }
    matching(chars, close + 1, '(', ')').map_or(start + 1, |end| end + 1)
}

/// `<https://…>` and HTML tags: skipped up to `>` if `<` is followed by a letter, `/` or `!`.
fn skip_angle(chars: &[char], start: usize, last_gt: Option<usize>) -> usize {
    let looks_like_tag = chars
        .get(start + 1)
        .is_some_and(|c| c.is_alphabetic() || matches!(c, '/' | '!'));
    if !looks_like_tag || last_gt.is_none_or(|last| last < start) {
        return start + 1;
    }
    chars
        .get(start..)
        .unwrap_or_default()
        .iter()
        .position(|c| *c == '>')
        .map_or(start + 1, |offset| start + offset + 1)
}

/// The index of the bracket that closes `open` at `start`, taking nesting into account; within the line and not deeper than [`MAX_NESTING`].
fn matching(chars: &[char], start: usize, open: char, close: char) -> Option<usize> {
    let mut depth = 0usize;
    let mut i = start;
    while let Some(&c) = chars.get(i) {
        if c == '\\' {
            i += 2;
            continue;
        }
        if c == open {
            depth += 1;
            if depth > MAX_NESTING {
                return None;
            }
        } else if c == close {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(text: &str) -> Vec<String> {
        parse_tags(text)
    }

    #[test]
    fn a_pathological_line_is_read_in_linear_time() {
        // Before, `[a](` x40,000 took about 12 s to read, and so did `<a` x100,000 without `>`: the closing bracket was searched from every bracket.
        for line in [
            "[a](".repeat(40_000),
            "<a".repeat(100_000),
            "(".repeat(100_000),
            format!("{}#тег", "[[".repeat(50_000)),
        ] {
            let started = std::time::Instant::now();
            let _ = parse_tags(&line);
            assert!(
                started.elapsed().as_secs_f64() < 2.0,
                "{:?}",
                started.elapsed()
            );
        }
        assert_eq!(
            parse_tags(&format!("{} #тег", "[a](".repeat(40_000))),
            ["тег"]
        );
    }

    #[test]
    fn plain_tags_in_any_alphabet_digits_and_underscore() {
        assert_eq!(tags("#идея купить молоко"), ["идея"]);
        assert_eq!(
            tags("Сегодня #Работа и #work_2026, #日本語!"),
            ["работа", "work_2026", "日本語"]
        );
        assert_eq!(
            tags("(#скобки) и «#кавычки» и \"#прямые\""),
            ["скобки", "кавычки", "прямые"]
        );
    }

    #[test]
    fn case_does_not_matter_and_repeats_are_dropped() {
        assert_eq!(tags("#Идея #ИДЕЯ #идея #Книги"), ["идея", "книги"]);
    }

    #[test]
    fn heading_with_space_is_not_a_tag_but_without_space_it_is() {
        assert_eq!(tags("# Заголовок\n## Второй"), Vec::<String>::new());
        assert_eq!(tags("#тег\n###"), ["тег"]);
    }

    #[test]
    fn hash_after_a_letter_digit_underscore_or_hash_is_not_a_tag() {
        assert_eq!(tags("a#b x_#y 7#z ##двойной"), Vec::<String>::new());
        assert_eq!(tags("a #b"), ["b"]);
        assert_eq!(tags("&#38; и \\#экранирован"), Vec::<String>::new());
    }

    #[test]
    fn numbers_alone_are_not_tags() {
        assert_eq!(tags("задача #1 и #2024 и #1a и #12_"), ["1a", "12_"]);
    }

    #[test]
    fn code_is_skipped() {
        assert_eq!(tags("до\n```\n#в_блоке\n```\nпосле #снаружи"), ["снаружи"]);
        assert_eq!(tags("~~~sh\n#нет\n~~~\n#да"), ["да"]);
        assert_eq!(tags("`#в_коде` и #вне"), ["вне"]);
        assert_eq!(tags("``код с ` и #внутри`` #вне"), ["вне"]);
        assert_eq!(tags("незакрытая `кавычка #тег"), ["тег"]);
        assert_eq!(tags("```\n#до_конца_записи"), Vec::<String>::new());
    }

    #[test]
    fn links_are_skipped() {
        assert_eq!(
            tags("[текст #в_ссылке](https://x.y/#якорь) #снаружи"),
            ["снаружи"]
        );
        assert_eq!(tags("<https://x.y/#a> и <b class=\"#c\"> #d"), ["d"]);
        assert_eq!(tags("смотри https://x.y/page#якорь #тег"), ["тег"]);
        assert_eq!(tags("www.site.ru/#a mailto:a@b.c#d #e"), ["e"]);
        assert_eq!(
            tags("- [ ] сделать #дело\n- [x] #готово"),
            ["дело", "готово"]
        );
    }

    #[test]
    fn too_long_tags_are_ignored() {
        let long = "а".repeat(MAX_TAG_CHARS + 1);
        assert_eq!(tags(&format!("#{long} #ok")), ["ok"]);
        let fits = "а".repeat(MAX_TAG_CHARS);
        assert_eq!(tags(&format!("#{fits}")), [fits]);
    }

    #[test]
    fn empty_and_hash_only() {
        assert!(tags("").is_empty());
        assert!(tags("#").is_empty());
        assert!(tags("# #  #").is_empty());
    }
}
