//! Drafts (PRD §5, §5.1): rules that do not depend on the storage.

/// The field text when text from the clipboard also arrives in the capture window draft (clipboard hotkey failure, PRD §1.2 item 9): the previous
/// non-empty draft is not overwritten -- the new text goes under it as a separate paragraph. An empty draft (or one with only spaces)
/// is replaced with the clipboard text.
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
            "the draft text is not edited"
        );
    }
}
