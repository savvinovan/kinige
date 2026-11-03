//! The only place where the bridge writes to stderr (ADR-0009). The MCP client reads the bridge's stderr as diagnostics; frames, arguments and
//! entry texts never get here -- only the error category and the request `id` (truncated and sanitized).

/// How many characters of `id` go into the line.
const ID_CHARS: usize = 40;

// The bridge has no dependencies, so no `log` crate either: this output deliberately goes straight to stderr, through one function.
#[allow(clippy::print_stderr)]
pub fn note(category: &str, id: Option<&str>) {
    match id {
        Some(id) => eprintln!("kinige-mcp: {category} id={}", clean_id(id)),
        None => eprintln!("kinige-mcp: {category}"),
    }
}

/// The `id` from the client message is foreign data: only letters, digits and `-_.` stay, the rest is replaced.
fn clean_id(id: &str) -> String {
    id.chars()
        .take(ID_CHARS)
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '?'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_in_the_log_line_is_short_and_has_no_control_characters() {
        assert_eq!(clean_id("\"abc\"\n\u{1b}[31m"), "?abc????31m");
        assert_eq!(clean_id(&"x".repeat(500)).len(), ID_CHARS);
    }
}
