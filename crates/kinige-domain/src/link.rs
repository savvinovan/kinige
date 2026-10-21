//! Проверка адреса перед открытием в системном браузере (ТЗ §6, §12.3): только `http`, `https`, `mailto`.

use std::fmt;
use url::Url;

/// Самый длинный допустимый адрес.
pub const MAX_URL_LEN: usize = 8192;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkError {
    /// Пустой, слишком длинный, с пробелами по краям или управляющими символами, относительный или не разбирается.
    Invalid,
    /// Схема не из списка (`file:`, `javascript:`, `data:`, `tauri:`, `asset:`, `ipc:` и любые другие).
    SchemeNotAllowed,
}

impl fmt::Display for LinkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LinkError::Invalid => f.write_str("неверный адрес"),
            LinkError::SchemeNotAllowed => f.write_str("схема адреса не разрешена"),
        }
    }
}

impl std::error::Error for LinkError {}

/// Проверенный адрес в нормализованном виде (его и открывает система). Разбор — как у браузера (WHATWG, крейт `url`);
/// управляющие символы и пробелы по краям отвергаются до разбора: разбор молча вырезал бы их, и система открыла бы
/// не ту строку, что проверена.
pub fn validate_external_url(input: &str) -> Result<String, LinkError> {
    if input.is_empty()
        || input.len() > MAX_URL_LEN
        || input.trim() != input
        || input.chars().any(char::is_control)
    {
        return Err(LinkError::Invalid);
    }
    let url = Url::parse(input).map_err(|_| LinkError::Invalid)?;
    match url.scheme() {
        "http" | "https" => {
            if url.host_str().is_none_or(str::is_empty) {
                return Err(LinkError::Invalid);
            }
        }
        "mailto" => {
            if url.path().is_empty() {
                return Err(LinkError::Invalid);
            }
        }
        _ => return Err(LinkError::SchemeNotAllowed),
    }
    Ok(url.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowed_schemes_are_normalized() {
        assert_eq!(
            validate_external_url("https://example.com").unwrap(),
            "https://example.com/"
        );
        assert_eq!(
            validate_external_url("http://example.com/a?b=c#d").unwrap(),
            "http://example.com/a?b=c#d"
        );
        assert_eq!(
            validate_external_url("HTTPS://Example.COM/Path").unwrap(),
            "https://example.com/Path"
        );
        assert_eq!(
            validate_external_url("mailto:me@example.com").unwrap(),
            "mailto:me@example.com"
        );
        assert_eq!(
            validate_external_url("https://fs.blog/habits").unwrap(),
            "https://fs.blog/habits"
        );
    }

    #[test]
    fn dangerous_and_internal_schemes_are_rejected() {
        for bad in [
            "file:///etc/passwd",
            "javascript:alert(1)",
            "JavaScript:alert(1)",
            "data:text/html,<script>alert(1)</script>",
            "tauri://localhost/",
            "asset://localhost/x",
            "ipc://localhost/app_info",
            "ftp://example.com/",
            "tel:+10000000000",
            "sms:+10000000000",
            "blob:https://example.com/uuid",
            "x-apple.systempreferences:com.apple.preference",
        ] {
            assert_eq!(
                validate_external_url(bad),
                Err(LinkError::SchemeNotAllowed),
                "{bad}"
            );
        }
    }

    #[test]
    fn relative_empty_and_malformed_are_rejected() {
        for bad in [
            "",
            "   ",
            "/path",
            "//example.com",
            "example.com",
            "../x",
            "#anchor",
            "?q=1",
            "https://",
            "http:///",
            "mailto:",
            "https://exa mple.com",
        ] {
            assert_eq!(
                validate_external_url(bad),
                Err(LinkError::Invalid),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn control_characters_and_padding_are_rejected_before_parsing() {
        for bad in [
            " https://example.com",
            "https://example.com ",
            "https://example.com\n",
            "https://exa\tmple.com",
            "java\nscript:alert(1)",
            "https://example.com/\u{0}",
            "https://example.com/\u{7f}",
            "\u{a0}https://example.com",
        ] {
            assert_eq!(
                validate_external_url(bad),
                Err(LinkError::Invalid),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn overlong_url_is_rejected() {
        let long = format!("https://example.com/{}", "a".repeat(MAX_URL_LEN));
        assert_eq!(validate_external_url(&long), Err(LinkError::Invalid));
    }
}
