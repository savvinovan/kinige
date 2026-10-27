//! Глобальные сочетания клавиш (ТЗ §5.1, слайс 2): формат записи, проверка и сочетания по умолчанию.
//!
//! Каноничная запись — `Модификаторы+Клавиша`: модификаторы в порядке `Ctrl`, `Alt`, `Shift`, `Cmd`, клавиша — имя
//! `KeyboardEvent.code` (`KeyV`, `Digit1`, `Space`, `F5`, `Slash`). Те же имена понимает разбор сочетаний в
//! `global-hotkey`, поэтому строка идёт и в плагин, и в `app_setting` без перекодирования. Фронтенд собирает строку из
//! `event.code` и модификаторов, поэтому раскладка не влияет (⌥V на macOS даёт «√», а `code` остаётся `KeyV`).

use serde::{Deserialize, Serialize};
use std::fmt;
use ts_rs::TS;

/// Какое сочетание: окно захвата или сохранение буфера.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "ShortcutKind.ts")]
pub enum ShortcutKind {
    /// Открыть окно быстрого захвата.
    Capture,
    /// Сохранить текст из буфера обмена в Inbox без окна.
    Clipboard,
}

impl ShortcutKind {
    pub const ALL: [ShortcutKind; 2] = [ShortcutKind::Capture, ShortcutKind::Clipboard];

    /// Сочетание по умолчанию (решение пользователя 22.10.2026): ⌃⌥N и ⌥⇧V. Захват — **временно** ⌃⌥N вместо ⌥Space:
    /// ⌥Space и ⌥⇧Space перехватывают ChatGPT и Gemini, ⌃Space и ⌃⌥Space — переключение раскладки, ⌘Space — Raycast.
    pub fn default_accelerator(self) -> &'static str {
        match self {
            ShortcutKind::Capture => "Ctrl+Alt+KeyN",
            ShortcutKind::Clipboard => "Alt+Shift+KeyV",
        }
    }

    /// Ключ в `app_setting`.
    pub fn setting_key(self) -> &'static str {
        match self {
            ShortcutKind::Capture => "shortcut_capture",
            ShortcutKind::Clipboard => "shortcut_clipboard",
        }
    }
}

/// Состояние сочетания в этом запуске.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "ShortcutState.ts")]
pub enum ShortcutState {
    /// Система приняла сочетание: оно работает.
    Assigned,
    /// Не назначено: сочетание занято другой программой или системой.
    Taken,
}

/// Сочетание в «Настройках».
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "ShortcutStatus.ts")]
pub struct ShortcutStatus {
    pub kind: ShortcutKind,
    /// Каноничная запись (`Ctrl+Alt+KeyN`).
    pub accelerator: String,
    /// Сочетание совпадает с умолчанием: «Сбросить» не нужна.
    pub is_default: bool,
    pub state: ShortcutState,
}

/// Почему сочетание не принято.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutError {
    /// Строка не разобрана, неизвестная клавиша, повтор модификатора, больше одной клавиши.
    Invalid,
    /// Нет Ctrl, Alt или Cmd: одна буква или Shift+буква перехватили бы обычный ввод.
    NeedsModifier,
    /// Системное сочетание, которое нельзя отнимать (⌘Q, ⌘W, ⌘H, ⌘M, ⌘Space).
    Reserved,
}

impl fmt::Display for ShortcutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ShortcutError::Invalid => "неверная запись сочетания",
            ShortcutError::NeedsModifier => "нужен Ctrl, Alt или Cmd",
            ShortcutError::Reserved => "сочетание зарезервировано системой",
        })
    }
}

impl std::error::Error for ShortcutError {}

/// Допустимые клавиши (имена `KeyboardEvent.code`).
fn is_known_key(key: &str) -> bool {
    let letter = key
        .strip_prefix("Key")
        .is_some_and(|rest| rest.len() == 1 && rest.as_bytes()[0].is_ascii_uppercase());
    let digit = key
        .strip_prefix("Digit")
        .is_some_and(|rest| rest.len() == 1 && rest.as_bytes()[0].is_ascii_digit());
    let function = key
        .strip_prefix('F')
        .and_then(|rest| rest.parse::<u8>().ok())
        .is_some_and(|number| (1..=12).contains(&number) && key == format!("F{number}"));
    letter
        || digit
        || function
        || matches!(
            key,
            "Space"
                | "Backquote"
                | "Minus"
                | "Equal"
                | "BracketLeft"
                | "BracketRight"
                | "Backslash"
                | "Semicolon"
                | "Quote"
                | "Comma"
                | "Period"
                | "Slash"
        )
}

/// Системные сочетания, которые приложение не отнимает. Список короткий и намеренно консервативный: «занято» остальным
/// определяет сама система при регистрации.
fn is_reserved(ctrl: bool, alt: bool, shift: bool, cmd: bool, key: &str) -> bool {
    if !cmd || ctrl || alt {
        return false;
    }
    match key {
        "KeyQ" => true,
        "KeyW" | "KeyH" | "KeyM" | "Space" => !shift,
        _ => false,
    }
}

/// Разбирает запись и возвращает каноничную (порядок модификаторов, регистр имён). Модификаторы принимаются как
/// `Ctrl`/`Control`, `Alt`/`Option`, `Shift`, `Cmd`/`Command`/`Super`; клавиша — как в `KeyboardEvent.code`.
pub fn canonical_accelerator(text: &str) -> Result<String, ShortcutError> {
    let mut ctrl = false;
    let mut alt = false;
    let mut shift = false;
    let mut cmd = false;
    let mut key: Option<&str> = None;
    for token in text.split('+') {
        if key.is_some() {
            // Клавиша должна быть последней и единственной.
            return Err(ShortcutError::Invalid);
        }
        let flag = match token {
            "Ctrl" | "Control" => Some(&mut ctrl),
            "Alt" | "Option" => Some(&mut alt),
            "Shift" => Some(&mut shift),
            "Cmd" | "Command" | "Super" => Some(&mut cmd),
            _ => None,
        };
        match flag {
            Some(flag) => {
                if *flag {
                    return Err(ShortcutError::Invalid);
                }
                *flag = true;
            }
            None => key = Some(token),
        }
    }
    let key = key.ok_or(ShortcutError::Invalid)?;
    if !is_known_key(key) {
        return Err(ShortcutError::Invalid);
    }
    if !(ctrl || alt || cmd) {
        return Err(ShortcutError::NeedsModifier);
    }
    if is_reserved(ctrl, alt, shift, cmd, key) {
        return Err(ShortcutError::Reserved);
    }
    let mut out = String::new();
    for (on, name) in [(ctrl, "Ctrl"), (alt, "Alt"), (shift, "Shift"), (cmd, "Cmd")] {
        if on {
            out.push_str(name);
            out.push('+');
        }
    }
    out.push_str(key);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_canonical() {
        for kind in ShortcutKind::ALL {
            assert_eq!(
                canonical_accelerator(kind.default_accelerator()).as_deref(),
                Ok(kind.default_accelerator())
            );
        }
        assert_eq!(ShortcutKind::Capture.default_accelerator(), "Ctrl+Alt+KeyN");
        assert_eq!(
            ShortcutKind::Clipboard.default_accelerator(),
            "Alt+Shift+KeyV"
        );
    }

    #[test]
    fn modifiers_are_ordered_and_aliases_accepted() {
        assert_eq!(
            canonical_accelerator("Cmd+Shift+Alt+Ctrl+KeyK").unwrap(),
            "Ctrl+Alt+Shift+Cmd+KeyK"
        );
        assert_eq!(
            canonical_accelerator("Option+Command+Digit1").unwrap(),
            "Alt+Cmd+Digit1"
        );
        assert_eq!(canonical_accelerator("Ctrl+F5").unwrap(), "Ctrl+F5");
        assert_eq!(canonical_accelerator("Alt+Slash").unwrap(), "Alt+Slash");
    }

    #[test]
    fn malformed_strings_are_invalid() {
        for text in [
            "",
            "+",
            "Alt",
            "Alt+",
            "Alt++Space",
            "Alt+Alt+Space",
            "Alt+Space+Shift",
            "Alt+KeyA+KeyB",
            "Alt+Keya",
            "Alt+KeyAB",
            "Alt+Digit10",
            "Alt+F0",
            "Alt+F13",
            "Alt+F05",
            "Alt+Escape",
            "Alt+Enter",
            "Alt+Tab",
            "Alt+v",
            "alt+Space",
            " Alt+Space",
            "Alt+Space ",
            "Alt + Space",
        ] {
            assert_eq!(
                canonical_accelerator(text),
                Err(ShortcutError::Invalid),
                "{text:?}"
            );
        }
    }

    #[test]
    fn a_modifier_other_than_shift_is_required() {
        assert_eq!(
            canonical_accelerator("Space"),
            Err(ShortcutError::NeedsModifier)
        );
        assert_eq!(
            canonical_accelerator("KeyV"),
            Err(ShortcutError::NeedsModifier)
        );
        assert_eq!(
            canonical_accelerator("Shift+KeyV"),
            Err(ShortcutError::NeedsModifier)
        );
    }

    #[test]
    fn system_shortcuts_are_reserved() {
        for text in [
            "Cmd+KeyQ",
            "Cmd+Shift+KeyQ",
            "Cmd+KeyW",
            "Cmd+KeyH",
            "Cmd+KeyM",
            "Cmd+Space",
        ] {
            assert_eq!(
                canonical_accelerator(text),
                Err(ShortcutError::Reserved),
                "{text}"
            );
        }
        // С другим модификатором или с Shift (кроме Q) — не зарезервировано.
        assert!(canonical_accelerator("Cmd+Alt+KeyH").is_ok());
        assert!(canonical_accelerator("Cmd+Shift+KeyW").is_ok());
        assert!(canonical_accelerator("Ctrl+Space").is_ok());
    }

    #[test]
    fn setting_keys_differ() {
        assert_ne!(
            ShortcutKind::Capture.setting_key(),
            ShortcutKind::Clipboard.setting_key()
        );
    }

    #[test]
    fn status_serializes_in_camel_case() {
        let status = ShortcutStatus {
            kind: ShortcutKind::Clipboard,
            accelerator: "Alt+Shift+KeyV".into(),
            is_default: true,
            state: ShortcutState::Taken,
        };
        assert_eq!(
            serde_json::to_string(&status).unwrap(),
            r#"{"kind":"clipboard","accelerator":"Alt+Shift+KeyV","isDefault":true,"state":"taken"}"#
        );
    }
}
