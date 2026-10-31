//! Разбор `settings.json` (ADR-0010): байты → значения ключей и ошибки. Без IO и без базы. Правило: ничего не паникует,
//! текст ошибок `serde_json` не покидает модуль (в нём могут быть значения из файла), наружу идут только категория и позиция.

use kinige_domain::shortcuts::canonical_accelerator;
use kinige_domain::{
    AgentMode, FileProblem, FileProblemKind, KeyIssue, KeyIssueKind, ShortcutKind, TextSize,
};
use serde_json::{Map, Value};

/// Наибольший размер файла.
pub const MAX_BYTES: u64 = 1024 * 1024;
/// Сколько ошибок ключей и неизвестных ключей хранится самое большее: файл из мусора не должен раздувать окно.
pub(crate) const MAX_ISSUES: usize = 100;
/// Сколько знаков имени неизвестного ключа показывается.
const KEY_SHOWN_CHARS: usize = 64;

/// Файл целиком не годится. Без текста ошибки разбора: он может содержать значения из файла.
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
    /// Категория для лога: `parse`, `io`.
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

/// Значение ключа: нет в файле (не менять), `null` (по умолчанию) или значение.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Field<T> {
    Absent,
    Default,
    Value(T),
}

/// Элемент `agent_access.chats`: `id` главнее названия.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChatRef {
    pub index: usize,
    pub id: Option<String>,
    pub title: Option<String>,
}

/// Файл, разобранный до уровня значений. Проверки, зависящие от состояния (папка, чаты, занятость сочетаний), — при применении.
#[derive(Debug)]
pub(crate) struct Parsed {
    /// Документ целиком: неизвестные ключи сохраняются при записи.
    pub raw: Map<String, Value>,
    pub text_size: Field<TextSize>,
    pub shortcuts: [Field<String>; 2],
    pub mirror_folder: Field<String>,
    pub agent_enabled: Field<bool>,
    pub agent_mode: Field<AgentMode>,
    pub chats: Field<Vec<ChatRef>>,
    pub issues: Vec<KeyIssue>,
    pub unknown: Vec<String>,
    /// Сколько неизвестных ключей не поместилось в `unknown` («и ещё N»).
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

    /// Раздел-объект; не объект — ошибка типа раздела, вложенные ключи игнорируются.
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
            self.shortcuts[slot] = match map.get(name) {
                None => Field::Absent,
                Some(Value::Null) => Field::Default,
                Some(Value::String(text)) => match canonical_accelerator(text) {
                    Ok(canonical) => Field::Value(canonical),
                    Err(_) => self.bad(&key, KeyIssueKind::ShortcutInvalid),
                },
                Some(_) => self.bad(&key, KeyIssueKind::Type),
            };
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

    /// Годные элементы массива; негодный элемент — ошибка этого элемента, остальные идут дальше.
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
