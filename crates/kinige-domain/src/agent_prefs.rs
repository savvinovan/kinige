//! The two settings tools of MCP (slice 5d, ADR-0022): which setting names an agent may name, which of them it may change,
//! and the shape of `get_settings`. No IO. The reply is built from its own structs, never from the window's `Settings`:
//! a field added there later does not reach an agent until it is added here on purpose.

use crate::agent::ArgError;
use crate::{AgentMode, TextSize};
use serde::Serialize;
use serde_json::{Map, Value};

/// `key` in the log for a name that is not one of [`SettingKey`]: the agent's own text never goes into the log.
pub const KEY_OTHER: &str = "other";

/// A setting an agent may name. Seven names; only the first is writable. A name is matched exactly: a different case,
/// surrounding spaces and the raw `app_setting` names (`mcp_enabled`, `mirror_dir`, ...) are not names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingKey {
    TextSize,
    ShortcutCapture,
    ShortcutClipboard,
    AgentEnabled,
    AgentMode,
    AgentChats,
    MirrorFolder,
}

impl SettingKey {
    pub const ALL: [SettingKey; 7] = [
        SettingKey::TextSize,
        SettingKey::ShortcutCapture,
        SettingKey::ShortcutClipboard,
        SettingKey::AgentEnabled,
        SettingKey::AgentMode,
        SettingKey::AgentChats,
        SettingKey::MirrorFolder,
    ];

    pub fn name(self) -> &'static str {
        match self {
            SettingKey::TextSize => "text_size",
            SettingKey::ShortcutCapture => "shortcuts.capture",
            SettingKey::ShortcutClipboard => "shortcuts.clipboard",
            SettingKey::AgentEnabled => "agent_access.enabled",
            SettingKey::AgentMode => "agent_access.mode",
            SettingKey::AgentChats => "agent_access.chats",
            SettingKey::MirrorFolder => "mirror.folder",
        }
    }

    pub fn from_name(name: &str) -> Option<SettingKey> {
        SettingKey::ALL.into_iter().find(|key| key.name() == name)
    }

    /// Only the text size: shortcuts, agent access and the copy folder are the user's, in the settings window.
    pub fn writable(self) -> bool {
        self == SettingKey::TextSize
    }
}

/// The names an agent may change, for the `writable` field of `get_settings`.
pub fn writable_names() -> Vec<&'static str> {
    SettingKey::ALL
        .into_iter()
        .filter(|key| key.writable())
        .map(SettingKey::name)
        .collect()
}

/// `args_json` of a `set_setting` call: `key` (a known name or `other`) and, on success only, the canonical `value`.
/// `raw_key` is what the agent sent; it is never written itself.
pub fn set_setting_args_json(raw_key: Option<&str>, value: Option<TextSize>) -> String {
    let key = raw_key
        .and_then(SettingKey::from_name)
        .map_or(KEY_OTHER, SettingKey::name);
    let mut out = serde_json::Map::new();
    out.insert("key".into(), key.into());
    if let Some(value) = value {
        out.insert("value".into(), value.as_setting().into());
    }
    serde_json::Value::Object(out).to_string()
}

/// The arguments of `set_setting`: the name, and for the writable setting its value. A name the agent may not change is
/// accepted here with no value: the store refuses it with `setting_not_writable`, whatever the value is.
pub(crate) fn parse_set_setting(
    args: &Map<String, Value>,
) -> Result<(SettingKey, Option<TextSize>), ArgError> {
    let Some(Value::String(name)) = args.get("key") else {
        return Err(ArgError("key is required and must be a string".into()));
    };
    let Some(key) = SettingKey::from_name(name) else {
        return Err(ArgError(
            "unknown setting; get_settings lists them, and `writable` those you may change".into(),
        ));
    };
    if !key.writable() {
        return Ok((key, None));
    }
    match args.get("value") {
        Some(Value::String(text)) => parse_text_size(text).map(|size| (key, Some(size))),
        _ => None,
    }
    .ok_or_else(|| ArgError("value must be \"normal\" or \"large\"".into()))
}

/// A text size from `set_setting`: exactly `normal` or `large`.
pub fn parse_text_size(value: &str) -> Option<TextSize> {
    match value {
        "normal" => Some(TextSize::Normal),
        "large" => Some(TextSize::Large),
        _ => None,
    }
}

/// One shortcut: what it is now and what the default is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ShortcutValue {
    pub value: String,
    pub default: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ShortcutsReply {
    pub capture: ShortcutValue,
    pub clipboard: ShortcutValue,
}

/// Where the Markdown copy goes, without the path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MarkdownCopy {
    DefaultFolder,
    ChosenFolder,
    Off,
}

/// Agent access as the agent may know it: the mode and the chats it can see (ids only; titles come from `list_chats`).
/// That access is on is implied by the call itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgentAccessReply {
    pub mode: AgentMode,
    pub visible_chat_ids: Vec<String>,
}

/// The reply of `get_settings` and of `set_setting`. The key set is pinned by a test.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SettingsReply {
    pub text_size: TextSize,
    pub shortcuts: ShortcutsReply,
    pub markdown_copy: MarkdownCopy,
    pub agent_access: AgentAccessReply,
    pub writable: Vec<&'static str>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_text_size_is_writable_and_the_names_are_exact() {
        assert_eq!(writable_names(), ["text_size"]);
        for key in SettingKey::ALL {
            assert_eq!(SettingKey::from_name(key.name()), Some(key));
        }
        for wrong in [
            "Text_Size",
            " text_size",
            "text_size ",
            "text_size\n",
            "textSize",
            "TEXT_SIZE",
            "shortcut_capture",
            "shortcut_clipboard",
            "mcp_enabled",
            "mcp_mode",
            "mcp_chats",
            "mirror_dir",
            "settings_file_hash",
            "forest_view",
            "",
        ] {
            assert_eq!(SettingKey::from_name(wrong), None, "{wrong:?}");
        }
    }

    #[test]
    fn the_log_args_hold_a_known_name_and_a_value_only_when_given() {
        assert_eq!(
            set_setting_args_json(Some("text_size"), Some(TextSize::Large)),
            r#"{"key":"text_size","value":"large"}"#
        );
        assert_eq!(
            set_setting_args_json(Some("shortcuts.capture"), None),
            r#"{"key":"shortcuts.capture"}"#
        );
        for raw in [Some("MARKER-secret"), Some("mcp_chats"), None] {
            assert_eq!(
                set_setting_args_json(raw, None),
                r#"{"key":"other"}"#,
                "{raw:?}"
            );
        }
    }

    #[test]
    fn a_text_size_is_exactly_normal_or_large() {
        assert_eq!(parse_text_size("normal"), Some(TextSize::Normal));
        assert_eq!(parse_text_size("large"), Some(TextSize::Large));
        for wrong in ["Large", " large", "huge", "", "1.25"] {
            assert_eq!(parse_text_size(wrong), None);
        }
    }
}
