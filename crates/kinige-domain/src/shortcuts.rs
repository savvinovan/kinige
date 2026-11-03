//! Global keyboard shortcuts (PRD §5.1, slice 2): the notation format, validation and the default shortcuts.
//!
//! The canonical notation is `Modifiers+Key`: modifiers in the order `Ctrl`, `Alt`, `Shift`, `Cmd`, the key is a
//! `KeyboardEvent.code` name (`KeyV`, `Digit1`, `Space`, `F5`, `Slash`). The shortcut parser in `global-hotkey`
//! understands the same names, so the string goes both to the plugin and to `app_setting` without re-encoding. The
//! frontend builds the string from `event.code` and modifiers, so the keyboard layout has no effect (⌥V on macOS gives
//! "√", but `code` stays `KeyV`).

use serde::{Deserialize, Serialize};
use std::fmt;
use ts_rs::TS;

/// Which shortcut: the capture window or saving the clipboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "ShortcutKind.ts")]
pub enum ShortcutKind {
    /// Open the quick capture window.
    Capture,
    /// Save the text from the clipboard to Inbox without a window.
    Clipboard,
}

impl ShortcutKind {
    pub const ALL: [ShortcutKind; 2] = [ShortcutKind::Capture, ShortcutKind::Clipboard];

    /// The default shortcut (the user's decision of 22.10.2026): ⌃⌥N and ⌥⇧V. Capture is **temporarily** ⌃⌥N instead of
    /// ⌥Space: ⌥Space and ⌥⇧Space are taken by ChatGPT and Gemini, ⌃Space and ⌃⌥Space switch the layout, ⌘Space is Raycast.
    pub fn default_accelerator(self) -> &'static str {
        match self {
            ShortcutKind::Capture => "Ctrl+Alt+KeyN",
            ShortcutKind::Clipboard => "Alt+Shift+KeyV",
        }
    }

    /// The key in `app_setting`.
    pub fn setting_key(self) -> &'static str {
        match self {
            ShortcutKind::Capture => "shortcut_capture",
            ShortcutKind::Clipboard => "shortcut_clipboard",
        }
    }
}

/// The state of the shortcut in this run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "ShortcutState.ts")]
pub enum ShortcutState {
    /// The system accepted the shortcut: it works.
    Assigned,
    /// Not assigned: the shortcut is taken by another program or by the system.
    Taken,
}

/// A shortcut in "Settings".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "ShortcutStatus.ts")]
pub struct ShortcutStatus {
    pub kind: ShortcutKind,
    /// The canonical notation (`Ctrl+Alt+KeyN`).
    pub accelerator: String,
    /// The shortcut equals the default: "Reset" is not needed.
    pub is_default: bool,
    pub state: ShortcutState,
}

/// Why the shortcut was not accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutError {
    /// The string did not parse, an unknown key, a repeated modifier, more than one key.
    Invalid,
    /// No Ctrl, Alt or Cmd: a single letter or Shift+letter would intercept normal typing.
    NeedsModifier,
    /// A system shortcut that must not be taken away (⌘Q, ⌘W, ⌘H, ⌘M, ⌘Space).
    Reserved,
}

impl fmt::Display for ShortcutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ShortcutError::Invalid => "invalid shortcut notation",
            ShortcutError::NeedsModifier => "Ctrl, Alt or Cmd is required",
            ShortcutError::Reserved => "shortcut is reserved by the system",
        })
    }
}

impl std::error::Error for ShortcutError {}

/// Allowed keys (`KeyboardEvent.code` names).
fn is_known_key(key: &str) -> bool {
    let letter = key
        .strip_prefix("Key")
        .is_some_and(|rest| matches!(rest.as_bytes(), [byte] if byte.is_ascii_uppercase()));
    let digit = key
        .strip_prefix("Digit")
        .is_some_and(|rest| matches!(rest.as_bytes(), [byte] if byte.is_ascii_digit()));
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

/// System shortcuts that the app does not take away. The list is short and deliberately conservative: whether the rest
/// is "taken" is decided by the system itself at registration.
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

/// Parses the notation and returns the canonical one (modifier order, name case). Modifiers are accepted as
/// `Ctrl`/`Control`, `Alt`/`Option`, `Shift`, `Cmd`/`Command`/`Super`; the key is as in `KeyboardEvent.code`.
pub fn canonical_accelerator(text: &str) -> Result<String, ShortcutError> {
    let mut ctrl = false;
    let mut alt = false;
    let mut shift = false;
    let mut cmd = false;
    let mut key: Option<&str> = None;
    for token in text.split('+') {
        if key.is_some() {
            // The key must be the last and the only one.
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
        // With another modifier or with Shift (except Q) it is not reserved.
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
