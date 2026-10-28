//! Нормализация и стемминг (то, что станет `kinige_domain::search`).
use rust_stemmers::{Algorithm, Stemmer};

pub struct Norm {
    ru: Stemmer,
    en: Stemmer,
}

impl Norm {
    pub fn new() -> Self {
        Norm { ru: Stemmer::create(Algorithm::Russian), en: Stemmer::create(Algorithm::English) }
    }

    /// Слова: непрерывные серии `is_alphanumeric`, нижний регистр, «ё» -> «е».
    pub fn words(text: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut cur = String::new();
        for ch in text.chars() {
            if ch.is_alphanumeric() {
                for lc in ch.to_lowercase() {
                    cur.push(if lc == 'ё' { 'е' } else { lc });
                }
            } else if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
        }
        if !cur.is_empty() {
            out.push(cur);
        }
        out
    }

    pub fn stem(&self, word: &str) -> String {
        let has_digit = word.chars().any(|c| c.is_ascii_digit());
        if has_digit {
            return word.to_string();
        }
        if word.chars().any(|c| ('а'..='я').contains(&c)) {
            self.ru.stem(word).into_owned()
        } else if word.chars().all(|c| c.is_ascii_alphabetic()) {
            self.en.stem(word).into_owned()
        } else {
            word.to_string()
        }
    }

    /// Текст для индекса. `with_source`: рядом со стеммами лежат и исходные слова.
    pub fn index_text(&self, text: &str, with_source: bool) -> String {
        let mut out = String::with_capacity(text.len());
        for w in Self::words(text) {
            if !out.is_empty() {
                out.push(' ');
            }
            let s = self.stem(&w);
            out.push_str(&s);
            if with_source && s != w {
                out.push(' ');
                out.push_str(&w);
            }
        }
        out
    }
}
