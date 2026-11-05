//! Requests and replies of the screens driver (KINIG-92, ADR-0016): the shapes, their limits, the id rule and the files. Pure data and
//! file handling, no windows. Everything here is fed by a file a scenario script wrote, so every step is checked and garbage is an
//! answer (`Reject`), never a panic (ADR-0011) and never a write outside the driver directory.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

/// A request file over this size is refused without being read (a step is a few hundred bytes; the longest is a typed text).
pub const MAX_REQUEST_BYTES: u64 = 64 * 1024;
/// The longest value or error text a window may answer with (the `text` step of a big feed is the largest).
pub const MAX_REPLY_TEXT_BYTES: usize = 4 * 1024 * 1024;
const MAX_ID_LEN: usize = 64;
const MAX_NAME_CHARS: usize = 512;
const MAX_TYPED_CHARS: usize = 8 * 1024;
const MAX_NTH: u32 = 1000;
const MAX_WAIT_MS: u32 = 120_000;
const MAX_KEY_CHARS: usize = 32;

pub const REQUESTS_DIR: &str = "requests";
pub const REPLIES_DIR: &str = "replies";

/// A request: `{ "id", "window", "step": {…} }`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub id: String,
    pub window: Window,
    pub step: Step,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Window {
    Main,
    Settings,
    Capture,
}

/// The v1 steps. The window runs all of them but `front`, which the app does itself.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Step {
    Press {
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        role: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        nth: Option<u32>,
    },
    Menu {
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        role: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        nth: Option<u32>,
    },
    Type {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        text: String,
    },
    Key {
        key: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        code: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mods: Option<Vec<String>>,
    },
    Wait {
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        role: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        gone: Option<bool>,
        #[serde(rename = "timeoutMs")]
        timeout_ms: u32,
    },
    Has {
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        role: Option<String>,
    },
    Scroll {
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        role: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        nth: Option<u32>,
    },
    // Struct variants, not unit ones: serde ignores `deny_unknown_fields` for a unit variant of a tagged enum.
    Text {},
    Front {},
}

/// What a window sends back through `screens_driver_reply`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Message {
    /// The page is listening: steps for this window may be sent now.
    Ready,
    Reply {
        id: String,
        ok: bool,
        #[serde(default)]
        value: Option<String>,
        #[serde(default)]
        error: Option<String>,
    },
}

/// The file `replies/<id>.json`: `{ "id", "ok", "value"?, "error"? }`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Reply {
    pub id: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Why a request was refused; the category goes to the reply and the log, never the content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reject {
    NotAFile,
    TooLarge,
    Unreadable,
    BadJson,
    IdMismatch,
    UnknownWindow,
    UnknownStep,
    InvalidStep,
}

impl Reject {
    pub fn category(self) -> &'static str {
        match self {
            Reject::NotAFile => "not_a_file",
            Reject::TooLarge => "too_large",
            Reject::Unreadable => "unreadable",
            Reject::BadJson => "bad_json",
            Reject::IdMismatch => "id_mismatch",
            Reject::UnknownWindow => "unknown_window",
            Reject::UnknownStep => "unknown_step",
            Reject::InvalidStep => "invalid_step",
        }
    }
}

impl Reply {
    pub fn ok(id: &str, value: Option<String>) -> Self {
        Reply {
            id: id.to_string(),
            ok: true,
            value,
            error: None,
        }
    }

    pub fn error(id: &str, error: impl Into<String>) -> Self {
        Reply {
            id: id.to_string(),
            ok: false,
            value: None,
            error: Some(error.into()),
        }
    }
}

impl Window {
    /// The window label of `tauri.conf.json` and `settings_window`.
    pub fn label(self) -> &'static str {
        match self {
            Window::Main => "main",
            Window::Settings => "settings",
            Window::Capture => "capture",
        }
    }
}

/// An id that is safe as a file name: 1 to 64 of `A-Z a-z 0-9 _ -`. No dots, separators or anything else, so `../x`, `a/b`, `a\b`, `.`
/// and `..` all fail.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_ID_LEN
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
}

/// The id of a request file name `<id>.json`, if it is one and the id is valid.
pub fn id_of_file_name(name: &str) -> Option<&str> {
    name.strip_suffix(".json").filter(|id| valid_id(id))
}

/// Reads a request file: a regular file (not a link, not a directory), at most `MAX_REQUEST_BYTES`.
pub fn read_request_file(path: &Path) -> Result<Vec<u8>, Reject> {
    let meta = fs::symlink_metadata(path).map_err(|_| Reject::Unreadable)?;
    if !meta.is_file() {
        return Err(Reject::NotAFile);
    }
    if meta.len() > MAX_REQUEST_BYTES {
        return Err(Reject::TooLarge);
    }
    let file = File::open(path).map_err(|_| Reject::Unreadable)?;
    // The file may grow between the check and the read: the reader is bounded too.
    let mut bytes = Vec::new();
    file.take(MAX_REQUEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Reject::Unreadable)?;
    if bytes.len() as u64 > MAX_REQUEST_BYTES {
        return Err(Reject::TooLarge);
    }
    Ok(bytes)
}

/// Parses and checks a request. `file_id` is the id from the file name; the id inside must be the same one.
pub fn parse_request(file_id: &str, bytes: &[u8]) -> Result<Request, Reject> {
    let value: Value = serde_json::from_slice(bytes).map_err(|_| Reject::BadJson)?;
    let object = value.as_object().ok_or(Reject::BadJson)?;
    if object.get("id").and_then(Value::as_str) != Some(file_id) {
        return Err(Reject::IdMismatch);
    }
    match object.get("window").and_then(Value::as_str) {
        Some("main" | "settings" | "capture") => {}
        _ => return Err(Reject::UnknownWindow),
    }
    let kind = object
        .get("step")
        .and_then(Value::as_object)
        .and_then(|step| step.get("kind"))
        .and_then(Value::as_str);
    if !matches!(
        kind,
        Some("press" | "menu" | "type" | "key" | "wait" | "has" | "scroll" | "text" | "front")
    ) {
        return Err(Reject::UnknownStep);
    }
    let request: Request = serde_json::from_value(value).map_err(|_| Reject::InvalidStep)?;
    if request.step.within_limits() {
        Ok(request)
    } else {
        Err(Reject::InvalidStep)
    }
}

impl Step {
    /// The size limits and shapes the page relies on: names are short, roles are lower-case words, `nth` and the wait are bounded.
    fn within_limits(&self) -> bool {
        let name_ok = |name: &str| !name.is_empty() && name.chars().count() <= MAX_NAME_CHARS;
        let role_ok = |role: &Option<String>| {
            role.as_deref().is_none_or(|role| {
                !role.is_empty()
                    && role.len() <= 32
                    && role.bytes().all(|byte| byte.is_ascii_lowercase())
            })
        };
        let nth_ok = |nth: &Option<u32>| nth.is_none_or(|nth| nth <= MAX_NTH);
        match self {
            Step::Press { name, role, nth }
            | Step::Menu { name, role, nth }
            | Step::Scroll { name, role, nth } => name_ok(name) && role_ok(role) && nth_ok(nth),
            Step::Type { name, text } => {
                name.as_deref().is_none_or(name_ok) && text.chars().count() <= MAX_TYPED_CHARS
            }
            Step::Key { key, code, mods } => {
                !key.is_empty()
                    && key.chars().count() <= MAX_KEY_CHARS
                    && code
                        .as_deref()
                        .is_none_or(|code| code.chars().count() <= MAX_KEY_CHARS)
                    && mods.as_deref().is_none_or(|mods| {
                        mods.len() <= 4
                            && mods
                                .iter()
                                .all(|m| matches!(m.as_str(), "ctrl" | "shift" | "alt" | "meta"))
                    })
            }
            Step::Wait {
                name,
                role,
                timeout_ms,
                ..
            } => name_ok(name) && role_ok(role) && *timeout_ms <= MAX_WAIT_MS,
            Step::Has { name, role } => name_ok(name) && role_ok(role),
            Step::Text {} | Step::Front {} => true,
        }
    }
}

/// `<dir>/replies/<id>.json`, written atomically (a temporary file in the same directory, then a rename), so a script that sees the
/// file sees all of it. An id that is not valid writes nothing, and neither does a `replies` that is a link (it is not followed) or a
/// temporary name that something else already holds (`create_new` does not follow a link either).
///
/// Not closed, and accepted: a process that can write into the driver directory could swap `requests/<id>.json` between the check in
/// `read_request_file` and the read. It could write that request itself, so nothing is gained; the directory is the runner's temp.
pub fn write_reply(dir: &Path, reply: &Reply) -> io::Result<PathBuf> {
    if !valid_id(&reply.id) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "bad id"));
    }
    let replies = dir.join(REPLIES_DIR);
    if fs::symlink_metadata(&replies).is_err() {
        fs::create_dir_all(&replies)?;
    }
    if !fs::symlink_metadata(&replies)?.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "replies is not a directory",
        ));
    }
    let target = replies.join(format!("{}.json", reply.id));
    // A leading dot: the script looks for `<id>.json` only.
    let temporary = replies.join(format!(".{}.tmp", reply.id));
    let bytes = serde_json::to_vec(reply).map_err(io::Error::other)?;
    // A leftover of a crashed write goes (a link is removed, not followed); then the file must be new.
    match fs::remove_file(&temporary) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
        _ => {}
    }
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    drop(file);
    fs::rename(&temporary, &target)?;
    Ok(target)
}

#[cfg(test)]
#[path = "request_tests.rs"]
mod tests;
