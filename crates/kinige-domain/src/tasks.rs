//! Task items `- [ ] text` inside an entry (PRD §6): a click on the box in the feed edits the entry text, so the rule
//! "which item is this" lives here, not in the frontend. No IO.
//!
//! An item is a line that (after indentation and quote marks `>`) starts with a list marker (`-`, `*`, `+`, `1.` or `1)`),
//! a space and `[ ]`, `[x]` or `[X]`, then a space and non-empty text -- the same way the feed renderer finds it. Lines inside
//! fenced code blocks do not count. The item number is its zero-based order in the text.

use crate::mirror::{is_closing_fence, opening_fence};
use serde::Serialize;

/// Why the item was not toggled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskItemError {
    /// The text has no item with this number (the entry was changed in the meantime).
    NotFound,
    /// The item is already in the requested state: the window showed stale text.
    AlreadyThere,
}

/// State of an item by number: `Some(true)` -- checked; `None` -- there is no such item.
pub fn task_item_state(body_md: &str, index: usize) -> Option<bool> {
    find_item(body_md, index).map(|(_, checked)| checked)
}

/// The entry text with item `index` set to the `checked` state. The other bytes of the text do not change.
pub fn set_task_item(body_md: &str, index: usize, checked: bool) -> Result<String, TaskItemError> {
    let (at, current) = find_item(body_md, index).ok_or(TaskItemError::NotFound)?;
    if current == checked {
        return Err(TaskItemError::AlreadyThere);
    }
    // `at` is the offset of the one-byte character in the brackets found by `find_item`; `get` does not fail, and does not panic.
    let (head, tail) = body_md
        .get(..at)
        .zip(body_md.get(at + 1..))
        .ok_or(TaskItemError::NotFound)?;
    let mut text = String::with_capacity(body_md.len());
    text.push_str(head);
    text.push(if checked { 'x' } else { ' ' });
    text.push_str(tail);
    Ok(text)
}

/// A task item in the list for the agent (ADR-0009, slice 4a): the number is the same as in [`set_task_item`], the text is the rest of the line
/// after `[ ]` without spaces at the edges.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TaskItem {
    pub item: usize,
    pub done: bool,
    pub text: String,
}

/// All items of the entry in order, with the same rules as [`task_item_state`] (code does not count, a quote does).
pub fn task_items(body_md: &str) -> Vec<TaskItem> {
    let mut items = Vec::new();
    scan_items(body_md, |found| {
        items.push(TaskItem {
            item: found.index,
            done: found.checked,
            text: found.text.to_string(),
        });
        false
    });
    items
}

/// A found item: its number, the byte offset of the character in the brackets, the state and the text.
struct Found<'a> {
    index: usize,
    mark: usize,
    checked: bool,
    text: &'a str,
}

/// Walks the items in order; `visit` returns `true` to stop.
fn scan_items(body_md: &str, mut visit: impl FnMut(&Found<'_>) -> bool) {
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
            let found = Found {
                index: seen,
                mark: line_start + inside,
                checked,
                // `inside` is the character in the brackets, then `]`; both are one byte.
                text: line.get(inside + 2..).unwrap_or_default().trim(),
            };
            if visit(&found) {
                return;
            }
            seen += 1;
        }
    }
}

/// The byte offset of the character inside the brackets of item `index` and its state.
fn find_item(body_md: &str, index: usize) -> Option<(usize, bool)> {
    let mut result = None;
    scan_items(body_md, |found| {
        if found.index == index {
            result = Some((found.mark, found.checked));
            true
        } else {
            false
        }
    });
    result
}

/// The offset of the character in the brackets and the state, if the line is a task item.
fn marker_in_line(line: &str) -> Option<(usize, bool)> {
    let bytes = line.as_bytes();
    let mut i = 0;
    // Indentation and quote marks.
    while matches!(bytes.get(i), Some(b' ' | b'\t' | b'>')) {
        i += 1;
    }
    // List marker.
    match bytes.get(i)? {
        b'-' | b'*' | b'+' => i += 1,
        b'0'..=b'9' => {
            let digits = bytes
                .get(i..)
                .unwrap_or_default()
                .iter()
                .take_while(|b| b.is_ascii_digit())
                .count();
            if digits > 9 || !matches!(bytes.get(i + digits), Some(b'.' | b')')) {
                return None;
            }
            i += digits + 1;
        }
        _ => return None,
    }
    // A space after the marker, then `[ ]`, a space and non-empty text.
    let gap = bytes
        .get(i..)
        .unwrap_or_default()
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
    let tail = line.get(i + 3..)?;
    let spaces = tail
        .bytes()
        .take_while(|b| matches!(*b, b' ' | b'\t'))
        .count();
    if spaces == 0 || tail.get(spaces..)?.trim().is_empty() {
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
    fn the_item_list_uses_the_same_numbers_text_and_rules_as_the_toggle() {
        let body = "Планы:\n- [ ] первый  \n- [x] второй\n```\n- [ ] в коде\n```\n> 1. [X]\tцитата\n  * [ ] вложенный";
        let items = task_items(body);
        let got: Vec<(usize, bool, &str)> = items
            .iter()
            .map(|i| (i.item, i.done, i.text.as_str()))
            .collect();
        assert_eq!(
            got,
            [
                (0, false, "первый"),
                (1, true, "второй"),
                (2, true, "цитата"),
                (3, false, "вложенный"),
            ]
        );
        // Every number from the list is toggled by exactly this function and nothing else.
        for item in &items {
            assert_eq!(task_item_state(body, item.item), Some(item.done));
        }
        assert_eq!(task_item_state(body, items.len()), None);
        assert!(task_items("без пунктов\n- обычный").is_empty());
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
