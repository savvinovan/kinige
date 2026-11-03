//! Parsing `settings.json` (ADR-0010): bytes -> key values and errors. No IO and no database. The rule: nothing panics,
//! the text of `serde_json` errors never leaves the module (it may hold values from the file), only the category and the position go out.

use kinige_domain::shortcuts::canonical_accelerator;
use kinige_domain::{
    AgentMode, FileProblem, FileProblemKind, KeyIssue, KeyIssueKind, ShortcutKind, TextSize,
};
use serde_json::{Map, Value};

/// The largest file size.
pub const MAX_BYTES: u64 = 1024 * 1024;
/// The most key errors and unknown keys kept: a file of garbage must not bloat the window.
pub(crate) const MAX_ISSUES: usize = 100;
/// How many characters of an unknown key name are shown.
const KEY_SHOWN_CHARS: usize = 64;

/// The file as a whole is unusable. Without the parse error text: it may contain values from the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileError {
    Parse { line: u32, column: u32 },
    NotObject,
    NotUtf8,
    Bom,
    TooLarge,
    Io(std::io::ErrorKind),
}

impl FileError {
    /// The category for the log: `parse`, `io`.
    pub fn category(&self) -> &'static str {
        match self {
            FileError::Io(_) => "io",
            _ => "parse",
        }
    }

    pub(crate) fn problem(&self) -> FileProblem {
        let (kind, line, column) = match *self {
            FileError::Parse { line, column } => (FileProblemKind::Parse, Some(line), Some(column)),
            FileError::NotObject => (FileProblemKind::NotObject, None, None),
            FileError::NotUtf8 => (FileProblemKind::NotUtf8, None, None),
            FileError::Bom => (FileProblemKind::Bom, None, None),
            FileError::TooLarge => (FileProblemKind::TooLarge, None, None),
            FileError::Io(_) => (FileProblemKind::Io, None, None),
        };
        FileProblem { kind, line, column }
    }
}

/// A key value: absent from the file (leave as is), `null` (the default) or a value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Field<T> {
    Absent,
    Default,
    Value(T),
}

/// An `agent_access.chats` element: `id` wins over the title.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChatRef {
    pub index: usize,
    pub id: Option<String>,
    pub title: Option<String>,
}

/// The file parsed down to the level of values. Checks that depend on state (folder, chats, shortcut conflicts) happen on apply.
#[derive(Debug)]
pub(crate) struct Parsed {
    /// The whole document: unknown keys are preserved on write.
    pub raw: Map<String, Value>,
    pub text_size: Field<TextSize>,
    pub shortcuts: [Field<String>; 2],
    pub mirror_folder: Field<String>,
    pub agent_enabled: Field<bool>,
    pub agent_mode: Field<AgentMode>,
    pub chats: Field<Vec<ChatRef>>,
    pub issues: Vec<KeyIssue>,
    pub unknown: Vec<String>,
    /// How many unknown keys did not fit into `unknown` ("and N more").
    pub unknown_more: usize,
}

pub(crate) fn parse(bytes: &[u8]) -> Result<Parsed, FileError> {
    if bytes.len() as u64 > MAX_BYTES {
        return Err(FileError::TooLarge);
    }
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return Err(FileError::Bom);
    }
    if std::str::from_utf8(bytes).is_err() {
        return Err(FileError::NotUtf8);
    }
    let value: Value = serde_json::from_slice(bytes).map_err(|error| FileError::Parse {
        line: u32::try_from(error.line()).unwrap_or(u32::MAX),
        column: u32::try_from(error.column()).unwrap_or(u32::MAX),
    })?;
    let Value::Object(raw) = value else {
        return Err(FileError::NotObject);
    };
    let mut parsed = Parsed {
        text_size: Field::Absent,
        shortcuts: [Field::Absent, Field::Absent],
        mirror_folder: Field::Absent,
        agent_enabled: Field::Absent,
        agent_mode: Field::Absent,
        chats: Field::Absent,
        issues: Vec::new(),
        unknown: Vec::new(),
        unknown_more: 0,
        raw: Map::new(),
    };
    for (key, value) in &raw {
        match key.as_str() {
            "text_size" => parsed.text_size = parsed.read_text_size(value),
            "shortcuts" => parsed.shortcuts(value),
            "mirror" => parsed.mirror(value),
            "agent_access" => parsed.agent_access(value),
            other => parsed.note_unknown(other),
        }
    }
    parsed.raw = raw;
    Ok(parsed)
}

fn shorten(key: &str) -> String {
    if key.chars().count() <= KEY_SHOWN_CHARS {
        return key.to_string();
    }
    let head: String = key.chars().take(KEY_SHOWN_CHARS).collect();
    format!("{head}…")
}

impl Parsed {
    fn note_unknown(&mut self, name: &str) {
        if self.unknown.len() < MAX_ISSUES {
            self.unknown.push(shorten(name));
        } else {
            self.unknown_more += 1;
        }
    }

    fn issue(&mut self, key: &str, kind: KeyIssueKind) {
        if self.issues.len() < MAX_ISSUES {
            self.issues.push(KeyIssue {
                key: key.to_string(),
                kind,
            });
        }
    }

    fn read_text_size(&mut self, value: &Value) -> Field<TextSize> {
        match value {
            Value::Null => Field::Default,
            Value::String(text) if text == "normal" => Field::Value(TextSize::Normal),
            Value::String(text) if text == "large" => Field::Value(TextSize::Large),
            Value::String(_) => self.bad("text_size", KeyIssueKind::Value),
            _ => self.bad("text_size", KeyIssueKind::Type),
        }
    }

    fn bad<T>(&mut self, key: &str, kind: KeyIssueKind) -> Field<T> {
        self.issue(key, kind);
        Field::Absent
    }

    /// An object section; a non-object is a section type error, nested keys are ignored.
    fn section<'v>(&mut self, key: &str, value: &'v Value) -> Option<&'v Map<String, Value>> {
        match value {
            Value::Object(map) => Some(map),
            _ => {
                self.issue(key, KeyIssueKind::Type);
                None
            }
        }
    }

    fn unknown_in(&mut self, section: &str, map: &Map<String, Value>, known: &[&str]) {
        for key in map.keys().filter(|key| !known.contains(&key.as_str())) {
            self.note_unknown(&format!("{section}.{key}"));
        }
    }

    fn shortcuts(&mut self, value: &Value) {
        let Some(map) = self.section("shortcuts", value) else {
            return;
        };
        self.unknown_in("shortcuts", map, &["capture", "clipboard"]);
        for kind in ShortcutKind::ALL {
            let (name, slot) = match kind {
                ShortcutKind::Capture => ("capture", 0),
                ShortcutKind::Clipboard => ("clipboard", 1),
            };
            let key = format!("shortcuts.{name}");
            let field = match map.get(name) {
                None => Field::Absent,
                Some(Value::Null) => Field::Default,
                Some(Value::String(text)) => match canonical_accelerator(text) {
                    Ok(canonical) => Field::Value(canonical),
                    Err(_) => self.bad(&key, KeyIssueKind::ShortcutInvalid),
                },
                Some(_) => self.bad(&key, KeyIssueKind::Type),
            };
            if let Some(target) = self.shortcuts.get_mut(slot) {
                *target = field;
            }
        }
    }

    fn mirror(&mut self, value: &Value) {
        let Some(map) = self.section("mirror", value) else {
            return;
        };
        self.unknown_in("mirror", map, &["folder"]);
        self.mirror_folder = match map.get("folder") {
            None => Field::Absent,
            Some(Value::Null) => Field::Default,
            Some(Value::String(path)) => Field::Value(path.clone()),
            Some(_) => self.bad("mirror.folder", KeyIssueKind::Type),
        };
    }

    fn agent_access(&mut self, value: &Value) {
        let Some(map) = self.section("agent_access", value) else {
            return;
        };
        self.unknown_in("agent_access", map, &["enabled", "mode", "chats"]);
        self.agent_enabled = match map.get("enabled") {
            None => Field::Absent,
            Some(Value::Null) => Field::Default,
            Some(Value::Bool(on)) => Field::Value(*on),
            Some(_) => self.bad("agent_access.enabled", KeyIssueKind::Type),
        };
        self.agent_mode = match map.get("mode") {
            None => Field::Absent,
            Some(Value::Null) => Field::Default,
            Some(Value::String(text)) if text == "read" => Field::Value(AgentMode::Read),
            Some(Value::String(text)) if text == "read_write" => Field::Value(AgentMode::ReadWrite),
            Some(Value::String(_)) => self.bad("agent_access.mode", KeyIssueKind::Value),
            Some(_) => self.bad("agent_access.mode", KeyIssueKind::Type),
        };
        self.chats = match map.get("chats") {
            None => Field::Absent,
            Some(Value::Null) => Field::Default,
            Some(Value::Array(items)) => Field::Value(self.chat_refs(items)),
            Some(_) => self.bad("agent_access.chats", KeyIssueKind::Type),
        };
    }

    /// Valid array elements; a bad element is an error for that element, the rest go on.
    fn chat_refs(&mut self, items: &[Value]) -> Vec<ChatRef> {
        let mut refs = Vec::new();
        for (index, item) in items.iter().enumerate() {
            let key = format!("agent_access.chats[{index}]");
            let Value::Object(map) = item else {
                self.issue(&key, KeyIssueKind::Type);
                continue;
            };
            let text = |name: &str| match map.get(name) {
                None | Some(Value::Null) => Ok(None),
                Some(Value::String(text)) => Ok(Some(text.clone())),
                Some(_) => Err(()),
            };
            match (text("id"), text("title")) {
                (Ok(None), Ok(None)) => self.issue(&key, KeyIssueKind::Value),
                (Ok(id), Ok(title)) => refs.push(ChatRef { index, id, title }),
                _ => self.issue(&key, KeyIssueKind::Type),
            }
        }
        refs
    }
}
