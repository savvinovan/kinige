//! Черновики (ТЗ §5, §5.1): правила, которые не зависят от хранилища.

/// Текст поля, когда в черновик окна захвата приходит ещё и текст из буфера (сбой хоткея буфера, ТЗ §1.2 п. 9): прежний
/// непустой черновик не затирается — новый текст идёт под ним отдельным абзацем. Пустой (или из одних пробелов) черновик
/// заменяется текстом буфера.
pub fn joined_draft(existing: &str, clipboard: &str) -> String {
    if existing.trim().is_empty() {
        clipboard.to_string()
    } else {
        format!("{existing}\n\n{clipboard}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_draft_is_replaced_by_the_clipboard_text() {
        assert_eq!(joined_draft("", "из буфера"), "из буфера");
        assert_eq!(joined_draft("  \n\t", "из буфера"), "из буфера");
    }

    #[test]
    fn a_non_empty_draft_is_kept_and_the_clipboard_text_goes_below_it() {
        assert_eq!(
            joined_draft("недописанное", "из буфера"),
            "недописанное\n\nиз буфера"
        );
        assert_eq!(
            joined_draft("a\n", "b"),
            "a\n\n\nb",
            "текст черновика не правится"
        );
    }
}
