//! Язык интерфейса (ТЗ §2.1, ADR-0005): выбирает Rust по предпочитаемым языкам системы, а не `navigator.language`.

use serde::Serialize;
use ts_rs::TS;

/// Языки интерфейса: английский и русский.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "Language.ts")]
pub enum Language {
    En,
    Ru,
}

/// Первый предпочитаемый язык системы: русский — русский интерфейс, любой другой (или список пуст) — английский.
/// Теги вида `ru`, `ru-RU`, `ru_RU`, `RU-ru` — русские.
pub fn pick_language<S: AsRef<str>>(preferred: &[S]) -> Language {
    let primary = preferred.first().map(|tag| {
        tag.as_ref()
            .split(['-', '_'])
            .next()
            .unwrap_or("")
            .to_ascii_lowercase()
    });
    match primary.as_deref() {
        Some("ru") => Language::Ru,
        _ => Language::En,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn russian_system_gives_russian() {
        for tag in ["ru", "ru-RU", "ru_RU", "RU-ru", "ru-UA"] {
            assert_eq!(pick_language(&[tag]), Language::Ru, "{tag}");
        }
    }

    #[test]
    fn any_other_language_gives_english() {
        for tag in ["en", "en-US", "de-DE", "uk-UA", "ruby", "zh-Hans-CN", ""] {
            assert_eq!(pick_language(&[tag]), Language::En, "{tag:?}");
        }
        assert_eq!(pick_language::<&str>(&[]), Language::En);
    }

    #[test]
    fn only_the_first_preferred_language_counts() {
        assert_eq!(pick_language(&["de-DE", "ru-RU"]), Language::En);
        assert_eq!(pick_language(&["ru-RU", "en-US"]), Language::Ru);
    }

    #[test]
    fn serializes_as_lowercase_code() {
        assert_eq!(serde_json::to_string(&Language::Ru).unwrap(), "\"ru\"");
        assert_eq!(serde_json::to_string(&Language::En).unwrap(), "\"en\"");
    }
}
