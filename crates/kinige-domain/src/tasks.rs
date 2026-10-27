//! Пункты задач `- [ ] текст` внутри записи (ТЗ §6): щелчок по квадрату в ленте правит текст записи, поэтому правило
//! «какой это пункт» живёт здесь, а не во фронтенде. Без IO.
//!
//! Пункт — строка, которая (после отступа и знаков цитаты `>`) начинается с маркера списка (`-`, `*`, `+`, `1.` или `1)`),
//! пробела и `[ ]`, `[x]` или `[X]`, затем пробела и непустого текста — так же его находит рендер ленты. Строки внутри
//! ограждённых блоков кода не считаются. Номер пункта — порядковый в тексте с нуля.

use crate::mirror::{is_closing_fence, opening_fence};

/// Почему пункт не переключён.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskItemError {
    /// В тексте нет пункта с таким номером (запись успели изменить).
    NotFound,
    /// Пункт уже в нужном состоянии: окно показывало устаревший текст.
    AlreadyThere,
}

/// Состояние пункта по номеру: `Some(true)` — отмечен; `None` — такого пункта нет.
pub fn task_item_state(body_md: &str, index: usize) -> Option<bool> {
    find_item(body_md, index).map(|(_, checked)| checked)
}

/// Текст записи с пунктом `index`, переведённым в состояние `checked`. Остальные байты текста не меняются.
pub fn set_task_item(body_md: &str, index: usize, checked: bool) -> Result<String, TaskItemError> {
    let (at, current) = find_item(body_md, index).ok_or(TaskItemError::NotFound)?;
    if current == checked {
        return Err(TaskItemError::AlreadyThere);
    }
    let mut text = String::with_capacity(body_md.len());
    text.push_str(&body_md[..at]);
    text.push(if checked { 'x' } else { ' ' });
    text.push_str(&body_md[at + 1..]);
    Ok(text)
}

/// Байтовое смещение символа внутри скобок пункта `index` и его состояние.
fn find_item(body_md: &str, index: usize) -> Option<(usize, bool)> {
    let mut seen = 0usize;
    let mut fence: Option<(char, usize)> = None;
    let mut offset = 0usize;
    for line in body_md.split('\n') {
        let line_start = offset;
        offset += line.len() + 1;
        if let Some((marker, length)) = fence {
            if is_closing_fence(line, marker, length) {
                fence = None;
            }
            continue;
        }
        if let Some(opened) = opening_fence(line) {
            fence = Some(opened);
            continue;
        }
        if let Some((inside, checked)) = marker_in_line(line) {
            if seen == index {
                return Some((line_start + inside, checked));
            }
            seen += 1;
        }
    }
    None
}

/// Смещение символа в скобках и состояние, если строка — пункт задачи.
fn marker_in_line(line: &str) -> Option<(usize, bool)> {
    let bytes = line.as_bytes();
    let mut i = 0;
    // Отступ и знаки цитаты.
    while i < bytes.len() && matches!(bytes[i], b' ' | b'\t' | b'>') {
        i += 1;
    }
    // Маркер списка.
    match bytes.get(i)? {
        b'-' | b'*' | b'+' => i += 1,
        b'0'..=b'9' => {
            let digits = bytes[i..].iter().take_while(|b| b.is_ascii_digit()).count();
            if digits > 9 || !matches!(bytes.get(i + digits), Some(b'.' | b')')) {
                return None;
            }
            i += digits + 1;
        }
        _ => return None,
    }
    // Пробел после маркера, затем `[ ]`, пробел и непустой текст.
    let gap = bytes[i..]
        .iter()
        .take_while(|b| matches!(**b, b' ' | b'\t'))
        .count();
    if gap == 0 {
        return None;
    }
    i += gap;
    if bytes.get(i) != Some(&b'[') || bytes.get(i + 2) != Some(&b']') {
        return None;
    }
    let checked = match bytes.get(i + 1)? {
        b' ' => false,
        b'x' | b'X' => true,
        _ => return None,
    };
    let tail = &line[i + 3..];
    let spaces = tail
        .bytes()
        .take_while(|b| matches!(*b, b' ' | b'\t'))
        .count();
    if spaces == 0 || tail[spaces..].trim().is_empty() {
        return None;
    }
    Some((i + 1, checked))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggles_the_nth_item_and_leaves_every_other_byte() {
        let body = "Планы:\n- [ ] первый\n- [x] второй\n  - [ ] вложенный\nконец";
        assert_eq!(
            set_task_item(body, 0, true).unwrap(),
            "Планы:\n- [x] первый\n- [x] второй\n  - [ ] вложенный\nконец"
        );
        assert_eq!(
            set_task_item(body, 1, false).unwrap(),
            "Планы:\n- [ ] первый\n- [ ] второй\n  - [ ] вложенный\nконец"
        );
        assert_eq!(
            set_task_item(body, 2, true).unwrap(),
            "Планы:\n- [ ] первый\n- [x] второй\n  - [x] вложенный\nконец"
        );
    }

    #[test]
    fn works_with_cyrillic_before_the_marker_and_other_markers() {
        let body = "Привет, ёж\n1. [X] нумерованный\n* [ ] звёздочка\n+ [ ] плюс\n2) [ ] скобка";
        assert_eq!(task_item_state(body, 0), Some(true));
        assert_eq!(task_item_state(body, 1), Some(false));
        assert_eq!(task_item_state(body, 4), None);
        assert_eq!(
            set_task_item(body, 3, true).unwrap(),
            "Привет, ёж\n1. [X] нумерованный\n* [ ] звёздочка\n+ [ ] плюс\n2) [x] скобка"
        );
    }

    #[test]
    fn blockquotes_count_and_code_does_not() {
        let body = "> - [ ] в цитате\n```\n- [ ] в коде\n```\n- [ ] после";
        assert_eq!(task_item_state(body, 0), Some(false));
        assert_eq!(
            set_task_item(body, 1, true).unwrap(),
            "> - [ ] в цитате\n```\n- [ ] в коде\n```\n- [x] после"
        );
        assert_eq!(task_item_state(body, 2), None);
    }

    #[test]
    fn lines_that_are_not_items_are_skipped() {
        for body in [
            "[ ] без маркера",
            "- [] пустые скобки",
            "- [y] чужая буква",
            "-[ ] без пробела",
            "- [ ]",
            "- [ ]   ",
            "- [ ]текст",
            "- обычный пункт",
            "1234567890. [ ] слишком длинный номер",
        ] {
            assert_eq!(task_item_state(body, 0), None, "{body:?}");
        }
    }

    #[test]
    fn stale_requests_are_refused() {
        assert_eq!(
            set_task_item("- [x] a", 0, true),
            Err(TaskItemError::AlreadyThere)
        );
        assert_eq!(
            set_task_item("- [x] a", 1, true),
            Err(TaskItemError::NotFound)
        );
        assert_eq!(
            set_task_item("текст", 0, true),
            Err(TaskItemError::NotFound)
        );
    }
}
