//! Поиск по-русски (слайс 4, ADR-0008): нормализация текста для индекса и запрос FTS5. Без IO.
//!
//! Слово — непрерывная серия `char::is_alphanumeric`. Нормализация: нижний регистр, «ё» → «е». Русское слово (только
//! буквы а–я) идёт в русский стеммер Snowball, слово из ASCII-букв — в английский, остальное (цифры, другие алфавиты)
//! остаётся как есть. В индексе лежат только основы. Последний терм запроса, если ввод не кончается пробелом или знаком,
//! ищется по префиксу основы: «моло» → `мол*` находит «молоко» (основа `молок`).

use rust_stemmers::{Algorithm, Stemmer};

mod highlight;

pub use highlight::{highlight_terms, snippet};

/// Версия правил нормализации и раскладки индекса. Меняется вместе со стеммером, правилами слов и `tokenize` таблицы:
/// при несовпадении с `app_setting.search_index_version` индекс перестраивается при старте (ADR-0008).
pub const SEARCH_INDEX_VERSION: u32 = 1;

/// Наименьшая длина основы последнего терма, при которой он ищется как префикс: один знак дал бы `м*` по четверти индекса.
const MIN_PREFIX_CHARS: usize = 2;

/// Слова текста: оригинал (срез исходной строки) и нормализованная форма (нижний регистр, «ё» → «е»).
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
                let original = &text[start..end];
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

/// Основа нормализованного слова.
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

/// Текст для индекса: основы слов через пробел.
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

/// Разобранный запрос: основы слов и признак «последний терм — префикс».
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
        // Слово не закончено, если ввод оборвался на букве или цифре; пробел или знак после него — закончено.
        let typing = input.chars().next_back().is_some_and(char::is_alphanumeric);
        let prefix_last = typing
            && terms
                .last()
                .is_some_and(|last| last.chars().count() >= MIN_PREFIX_CHARS);
        terms.shrink_to_fit();
        Some(Query { terms, prefix_last })
    }

    /// Совпадает ли основа слова текста с одним из термов запроса (последний — по префиксу, если он префикс).
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

/// Совпадает ли название чата с запросом: каждому терму находится слово названия с той же основой (у последнего терма —
/// с этим началом основы, пока слово не закончено). Названия чатов в индексе не лежат: их не больше сотни, они
/// сверяются здесь. Пустой запрос ничему не соответствует.
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

/// Запрос FTS5 по вводу пользователя: термы в кавычках через пробел (то есть «и»); последний — префикс `"основа"*`, если
/// слово не закончено (ADR-0008). В кавычки попадают только буквы и цифры, поэтому синтаксис FTS5 из ввода пройти не
/// может. Без слов (пусто, одни знаки) — `None`.
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
