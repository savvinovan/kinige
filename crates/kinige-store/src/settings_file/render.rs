//! The file from the effective state (ADR-0010): all known keys with effective values, foreign keys are kept.
//! And the reverse: whether the parsed file matches the state ("hash matched - the database wins").

use super::apply::{ChatInfo, INBOX_FILE_TITLE, load_chats, resolve_chats, shortcut_slot};
use super::parse::{Field, Parsed};
use crate::agent::load_settings;
use crate::error::read_err;
use crate::settings::{KEY_TEXT_SIZE, effective_mirror_dir, get_setting, stored_accelerator};
use crate::{Store, StoreError};
use kinige_domain::groups::INBOX_ID;
use kinige_domain::{AgentMode, ShortcutKind, TextSize};
use serde_json::{Map, Value, json};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// The effective settings state as it goes into the file.
#[derive(Debug, Clone)]
pub(crate) struct EffectiveState {
    pub text_size: TextSize,
    pub shortcuts: [String; 2],
    /// The copy folder the user chose; `None` means the default.
    pub mirror_custom: Option<String>,
    /// The copy folder being written to now (the chosen one or the default).
    pub mirror_effective: Option<PathBuf>,
    pub enabled: bool,
    pub mode: AgentMode,
    /// Chats visible to the agent: they exist, are not deleted and are not excluded from AI.
    pub visible: Vec<ChatInfo>,
    pub chats: Vec<ChatInfo>,
}

impl Store {
    pub(crate) fn effective_state(&self) -> Result<EffectiveState, StoreError> {
        let default = self.inner.default_mirror_dir.clone();
        self.read(|conn| {
            let chats = load_chats(conn)?;
            let agent = load_settings(conn)?;
            let (mirror_effective, is_default) =
                effective_mirror_dir(conn, default.as_deref()).map_err(read_err)?;
            let visible = agent
                .chats
                .iter()
                .filter_map(|id| chats.iter().find(|chat| &chat.id == id))
                .filter(|chat| !chat.ai_excluded)
                .cloned()
                .collect();
            let shortcut = |kind| stored_accelerator(conn, kind).map_err(read_err);
            Ok(EffectiveState {
                text_size: get_setting(conn, KEY_TEXT_SIZE)
                    .map_err(read_err)?
                    .map_or(TextSize::Normal, |text| TextSize::from_setting(&text)),
                shortcuts: [
                    shortcut(ShortcutKind::Capture)?,
                    shortcut(ShortcutKind::Clipboard)?,
                ],
                mirror_custom: if is_default {
                    None
                } else {
                    mirror_effective
                        .as_ref()
                        .map(|path| path.to_string_lossy().into_owned())
                },
                mirror_effective,
                enabled: agent.enabled,
                mode: agent.mode,
                visible,
                chats,
            })
        })
    }
}

impl EffectiveState {
    /// The file content: foreign keys from `base` are kept, known keys are written with effective values.
    pub(crate) fn render(&self, base: Option<&Map<String, Value>>) -> Vec<u8> {
        let mut doc = base.cloned().unwrap_or_default();
        doc.insert("text_size".into(), json!(self.text_size.as_setting()));
        set_in(&mut doc, "shortcuts", |shortcuts| {
            shortcuts.insert("capture".into(), json!(self.shortcuts[0]));
            shortcuts.insert("clipboard".into(), json!(self.shortcuts[1]));
        });
        set_in(&mut doc, "mirror", |mirror| {
            // The default folder is `null`: a personal path does not end up in a file that people share.
            mirror.insert("folder".into(), json!(self.mirror_custom));
        });
        set_in(&mut doc, "agent_access", |agent| {
            agent.insert("enabled".into(), json!(self.enabled));
            agent.insert("mode".into(), json!(self.mode.setting()));
            let chats: Vec<Value> = self
                .visible
                .iter()
                .map(|chat| json!({"id": chat.id, "title": file_title(chat)}))
                .collect();
            agent.insert("chats".into(), Value::Array(chats));
        });
        let mut bytes =
            serde_json::to_vec_pretty(&Value::Object(doc)).unwrap_or_else(|_| b"{}".to_vec());
        bytes.push(b'\n');
        bytes
    }

    /// Whether the file matches the state. Keys that are absent from the file or unusable in it do not count: they
    /// would not have been applied, and the error is already visible.
    pub(crate) fn matches(&self, parsed: &Parsed) -> bool {
        let text_size_ok = match &parsed.text_size {
            Field::Absent => true,
            Field::Default => self.text_size == TextSize::Normal,
            Field::Value(size) => *size == self.text_size,
        };
        let shortcuts_ok = ShortcutKind::ALL.into_iter().all(|kind| {
            let slot = shortcut_slot(kind);
            let (Some(wanted), Some(current)) =
                (parsed.shortcuts.get(slot), self.shortcuts.get(slot))
            else {
                return false;
            };
            match wanted {
                Field::Absent => true,
                Field::Default => current == kind.default_accelerator(),
                Field::Value(accelerator) => accelerator == current,
            }
        });
        let mirror_ok = match &parsed.mirror_folder {
            Field::Absent => true,
            Field::Default => self.mirror_custom.is_none(),
            Field::Value(path) => self
                .mirror_effective
                .as_deref()
                .is_some_and(|current| current == Path::new(path)),
        };
        let enabled_ok = match parsed.agent_enabled {
            Field::Absent => true,
            Field::Default => !self.enabled,
            Field::Value(on) => on == self.enabled,
        };
        let mode_ok = match parsed.agent_mode {
            Field::Absent => true,
            Field::Default => self.mode == AgentMode::Read,
            Field::Value(mode) => mode == self.mode,
        };
        let chats_ok = match &parsed.chats {
            Field::Absent => true,
            Field::Default => self.visible.is_empty(),
            Field::Value(refs) => {
                let wanted: BTreeSet<String> =
                    resolve_chats(&self.chats, refs).0.into_iter().collect();
                let current: BTreeSet<String> =
                    self.visible.iter().map(|chat| chat.id.clone()).collect();
                wanted == current
            }
        };
        text_size_ok && shortcuts_ok && mirror_ok && enabled_ok && mode_ok && chats_ok
    }
}

fn file_title(chat: &ChatInfo) -> &str {
    if chat.id == INBOX_ID {
        INBOX_FILE_TITLE
    } else {
        &chat.title
    }
}

/// A section-object of the document: foreign keys inside are kept; if the section is not an object, it is replaced.
fn set_in(doc: &mut Map<String, Value>, name: &str, fill: impl FnOnce(&mut Map<String, Value>)) {
    let mut section = match doc.remove(name) {
        Some(Value::Object(map)) => map,
        _ => Map::new(),
    };
    fill(&mut section);
    doc.insert(name.to_string(), Value::Object(section));
}
