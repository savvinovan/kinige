//! Единственное место, где мост пишет в stderr (ADR-0009). Клиент MCP читает stderr моста как диагностику; кадры, аргументы и
//! тексты записей сюда не попадают никогда — только категория ошибки и `id` запроса (обрезанный и очищенный).

/// Сколько знаков `id` попадает в строку.
const ID_CHARS: usize = 40;

// У моста нет зависимостей, значит нет и крейта `log`: этот вывод осознанно идёт напрямую в stderr, одной функцией.
#[allow(clippy::print_stderr)]
pub fn note(category: &str, id: Option<&str>) {
    match id {
        Some(id) => eprintln!("kinige-mcp: {category} id={}", clean_id(id)),
        None => eprintln!("kinige-mcp: {category}"),
    }
}

/// `id` из сообщения клиента — чужие данные: только буквы, цифры и `-_.`, остальное заменяется.
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
