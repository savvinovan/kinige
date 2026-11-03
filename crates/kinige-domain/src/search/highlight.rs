//! Highlighting of the found words and the snippet for a result row (slice 4, task 4.5).

use super::{Query, normalize, stem, words};
use crate::mirror::{is_closing_fence, opening_fence};

/// [`highlight_terms`] returns at most this many distinct words: a two-character prefix in a long entry matches hundreds of words.
const MAX_TERMS: usize = 64;

/// Brackets deeper than this are not counted: otherwise a string of `[a](` x N would be read in quadratic time, and the
/// `inline` recursion over the link text would be limited only by the string length. Real links are not nested that deep.
const MAX_NESTING: usize = 64;

/// Distinct words of the entry text (as written there, with case) that matched the query by the rules of
/// [`super::fts_query`]: the word stem equals the term stem, and for the last term it starts with it, if the word is unfinished.
/// The frontend highlights these words as whole words.
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

/// Plain text around the first word from `terms` (case and "ё" do not matter), no longer than `max_chars` characters
/// including the "…" on the cut edges. Markdown markup is stripped, line breaks became spaces. Cuts only at a word
/// boundary (a single word longer than the remainder is cut by character) and at a UTF-8 character boundary. No match
/// -- the start of the text.
pub fn snippet(body_md: &str, terms: &[String], max_chars: usize) -> String {
    let text = plain_text(body_md);
    let chars: Vec<char> = text.chars().collect();
    if max_chars == 0 {
        return String::new();
    }
    if chars.len() <= max_chars {
        return text;
    }
    // No room for two "…" and at least one window character: just the start of the text.
    if max_chars < 3 {
        return chars.iter().take(max_chars).collect();
    }
    let wanted: Vec<String> = terms.iter().map(|term| normalize(term)).collect();
    let first_match = words(&text)
        .find(|(_, word)| wanted.contains(word))
        .map(|(original, _)| {
            // The word is a slice of `text`: its offset is the difference of the addresses.
            let offset = (original.as_ptr() as usize).saturating_sub(text.as_ptr() as usize);
            text.get(..offset).map_or(0, |head| head.chars().count())
        });
    let hit = first_match.unwrap_or(0);
    let is_alnum = |at: usize| chars.get(at).is_some_and(|c| c.is_alphanumeric());

    // Window start: a third of the window as context before the match, moved forward to the start of a word.
    let mut start = hit.saturating_sub(max_chars / 3);
    while start > 0
        && start < hit
        && (is_alnum(start - 1) || chars.get(start).is_some_and(|c| c.is_whitespace()))
    {
        start += 1;
    }
    let lead = usize::from(start > 0);
    // Window end. If the text does not fit, one character goes to the trailing "…".
    let mut end = (start + max_chars - lead).min(chars.len());
    if end < chars.len() {
        end = (start + max_chars - lead - 1).min(chars.len());
    }
    if end < chars.len() {
        let mut cut = end;
        while cut > start && is_alnum(cut - 1) && is_alnum(cut) {
            cut -= 1;
        }
        // A single word is longer than the window: cut by character.
        if cut > start {
            end = cut;
        }
    }
    let trail = end < chars.len();
    let body: String = chars.get(start..end).unwrap_or_default().iter().collect();
    format!(
        "{}{}{}",
        if lead == 1 { "…" } else { "" },
        body.trim_end(),
        if trail { "…" } else { "" }
    )
}

/// Entry Markdown -> plain text on one line: without list markers, quotes, headings, code fences, links and
/// emphasis. Not a Markdown parser, just cutting out the markup for a snippet.
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

/// Quote, heading, list marker and task box at the start of a line.
fn strip_line_markers(line: &str) -> &str {
    let mut rest = line.trim_start();
    while let Some(after) = rest.strip_prefix('>') {
        rest = after.trim_start();
    }
    let hashes = rest.chars().take_while(|c| *c == '#').count();
    let after_hashes = rest.get(hashes..).unwrap_or_default();
    if (1..=6).contains(&hashes) && after_hashes.starts_with([' ', '\t']) {
        rest = after_hashes.trim_start();
    }
    if let Some(after) = rest
        .strip_prefix(['-', '+', '*'])
        .filter(|a| a.starts_with([' ', '\t']))
    {
        rest = after.trim_start();
    } else {
        let digits = rest.chars().take_while(char::is_ascii_digit).count();
        let after_digits = rest.get(digits..).unwrap_or_default();
        let after_dot = rest.get(digits + 1..).unwrap_or_default();
        if (1..=9).contains(&digits)
            && after_digits.starts_with(['.', ')'])
            && after_dot.starts_with([' ', '\t'])
        {
            rest = after_dot.trim_start();
        }
    }
    for task in ["[ ] ", "[x] ", "[X] "] {
        if let Some(after) = rest.strip_prefix(task) {
            return after;
        }
    }
    rest
}

/// Markup inside a line: escaping, links `[text](address)` and `<address>`, backticks, `**`, `~~`, `__`, and also
/// single `*` and `_` at a word boundary (in the middle of a word, as in `snake_case`, they stay).
fn inline(line: &str) -> String {
    let chars: Vec<char> = line.chars().collect();
    let mut out = String::with_capacity(line.len());
    let stops = next_stops(&chars);
    let mut i = 0;
    while let Some(&c) = chars.get(i) {
        let prev = i.checked_sub(1).and_then(|p| chars.get(p).copied());
        let next = chars.get(i + 1).copied();
        match c {
            '\\' if next.is_some_and(|n| n.is_ascii_punctuation()) => {
                out.extend(next);
                i += 2;
                continue;
            }
            '[' => {
                if let Some(close) = matching(&chars, i, '[', ']')
                    && chars.get(close + 1) == Some(&'(')
                    && let Some(end) = matching(&chars, close + 1, '(', ')')
                {
                    let text: String = chars.get(i + 1..close).unwrap_or_default().iter().collect();
                    out.push_str(&inline(&text));
                    i = end + 1;
                    continue;
                }
                out.push(c);
            }
            '<' => {
                // `<https://…>` is an address without brackets and spaces: the first `>` or space after `<` must be `>`.
                let end = stops.get(i + 1).copied().unwrap_or(chars.len());
                match chars.get(i + 1..end) {
                    Some(address) if chars.get(end) == Some(&'>') => {
                        out.extend(address);
                        i = end + 1;
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

/// For each position, the index of the nearest `>` or whitespace at or after it (the string length if there is none).
/// Computed once per line, so that looking for the closing mark for each `<` does not make parsing quadratic in length.
fn next_stops(chars: &[char]) -> Vec<usize> {
    let mut stops = vec![chars.len(); chars.len() + 1];
    for (i, c) in chars.iter().enumerate().rev() {
        let following = stops.get(i + 1).copied().unwrap_or(chars.len());
        if let Some(slot) = stops.get_mut(i) {
            *slot = if *c == '>' || c.is_whitespace() {
                i
            } else {
                following
            };
        }
    }
    stops
}

/// Index of the bracket that closes `open` at `start`, taking nesting and escaping into account; no deeper than [`MAX_NESTING`].
fn matching(chars: &[char], start: usize, open: char, close: char) -> Option<usize> {
    let mut depth = 0usize;
    let mut i = start;
    while let Some(&current) = chars.get(i) {
        match current {
            '\\' => i += 1,
            c if c == open => {
                depth += 1;
                if depth > MAX_NESTING {
                    return None;
                }
            }
            c if c == close => {
                depth = depth.saturating_sub(1);
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
