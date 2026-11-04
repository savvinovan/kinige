//! Search in Russian (slice 4, ADR-0008): text normalization for the index and the FTS5 query. No IO.
//!
//! A word is a contiguous run of `char::is_alphanumeric`. Normalization: lower case, "ё" -> "е". A Russian word (only
//! the letters а-я) goes to the Russian Snowball stemmer, a word of ASCII letters goes to the English one, the rest
//! (digits, other alphabets) stays as is. The index holds only stems. The last query term, if the input does not end
//! with a space or a punctuation mark, is searched by stem prefix: "моло" -> `мол*` finds "молоко" (stem `молок`).

use rust_stemmers::{Algorithm, Stemmer};

mod highlight;

pub use highlight::{highlight_terms, snippet};

/// Version of the normalization rules and the index layout. Changes together with the stemmer, the word rules and the
/// table's `tokenize`: on a mismatch with `app_setting.search_index_version` the index is rebuilt at start (ADR-0008).
/// Version 2 (slice 7): an entry's text also holds the stems of its attachments' file names.
pub const SEARCH_INDEX_VERSION: u32 = 2;

/// Shortest stem length of the last term at which it is searched as a prefix: one character would give `м*` over a quarter of the index.
const MIN_PREFIX_CHARS: usize = 2;

/// Words of the text: the original (a slice of the source string) and the normalized form (lower case, "ё" -> "е").
pub(crate) fn words(text: &str) -> impl Iterator<Item = (&str, String)> {
    let mut chars = text.char_indices().peekable();
    std::iter::from_fn(move || {
        while let Some(&(start, c)) = chars.peek() {
            if c.is_alphanumeric() {
                let mut end = start;
                while let Some(&(i, c)) = chars.peek() {
                    if !c.is_alphanumeric() {
                        break;
                    }
                    end = i + c.len_utf8();
                    chars.next();
                }
                // The bounds come from `char_indices`: `get` does not fail here, and it does not panic either.
                let original = text.get(start..end).unwrap_or_default();
                return Some((original, normalize(original)));
            }
            chars.next();
        }
        None
    })
}

fn normalize(word: &str) -> String {
    word.chars()
        .flat_map(char::to_lowercase)
        .map(|c| if c == 'ё' { 'е' } else { c })
        .collect()
}

/// Stem of a normalized word.
pub(crate) fn stem(word: &str) -> String {
    let algorithm = if word.chars().all(|c| ('а'..='я').contains(&c)) {
        Algorithm::Russian
    } else if word.chars().all(|c| c.is_ascii_alphabetic()) {
        Algorithm::English
    } else {
        return word.to_string();
    };
    Stemmer::create(algorithm).stem(word).into_owned()
}

/// Text for the index: word stems separated by spaces.
pub fn index_text(body_md: &str) -> String {
    let mut out = String::with_capacity(body_md.len() / 2);
    for (_, word) in words(body_md) {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(&stem(&word));
    }
    out
}

/// A parsed query: word stems and the flag "the last term is a prefix".
pub(crate) struct Query {
    pub terms: Vec<String>,
    pub prefix_last: bool,
}

impl Query {
    pub(crate) fn parse(input: &str) -> Option<Query> {
        let mut terms: Vec<String> = words(input).map(|(_, word)| stem(&word)).collect();
        if terms.is_empty() {
            return None;
        }
        // The word is unfinished if the input stops on a letter or a digit; a space or a mark after it means finished.
        let typing = input.chars().next_back().is_some_and(char::is_alphanumeric);
        let prefix_last = typing
            && terms
                .last()
                .is_some_and(|last| last.chars().count() >= MIN_PREFIX_CHARS);
        terms.shrink_to_fit();
        Some(Query { terms, prefix_last })
    }

    /// Whether the stem of a text word matches one of the query terms (the last one by prefix, if it is a prefix).
    pub(crate) fn matches(&self, stem: &str) -> bool {
        let last = self.terms.len() - 1;
        self.terms.iter().enumerate().any(|(i, term)| {
            if i == last && self.prefix_last {
                stem.starts_with(term.as_str())
            } else {
                stem == term
            }
        })
    }
}

/// Whether a chat title matches the query: each term finds a title word with the same stem (for the last term, with
/// that stem start, while the word is unfinished). Chat titles are not in the index: there are at most a hundred of
/// them, they are checked here. An empty query matches nothing.
pub fn title_matches(title: &str, input: &str) -> bool {
    let Some(query) = Query::parse(input) else {
        return false;
    };
    let stems: Vec<String> = words(title).map(|(_, word)| stem(&word)).collect();
    let last = query.terms.len() - 1;
    query.terms.iter().enumerate().all(|(i, term)| {
        stems.iter().any(|s| {
            if i == last && query.prefix_last {
                s.starts_with(term.as_str())
            } else {
                s == term
            }
        })
    })
}

/// FTS5 query from the user's input: quoted terms separated by spaces (that is, "and"); the last one is a prefix
/// `"stem"*` if the word is unfinished (ADR-0008). Only letters and digits get into the quotes, so FTS5 syntax cannot
/// pass through from the input. Without words (empty, only marks) -- `None`.
pub fn fts_query(input: &str) -> Option<String> {
    let query = Query::parse(input)?;
    let last = query.terms.len() - 1;
    let parts: Vec<String> = query
        .terms
        .iter()
        .enumerate()
        .map(|(i, term)| {
            if i == last && query.prefix_last {
                format!("\"{term}\"*")
            } else {
                format!("\"{term}\"")
            }
        })
        .collect();
    Some(parts.join(" "))
}

#[cfg(test)]
mod tests;
