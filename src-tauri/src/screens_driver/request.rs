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
/// file sees all of it. An id that is not valid writes nothing.
pub fn write_reply(dir: &Path, reply: &Reply) -> io::Result<PathBuf> {
    if !valid_id(&reply.id) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "bad id"));
    }
    let replies = dir.join(REPLIES_DIR);
    fs::create_dir_all(&replies)?;
    let target = replies.join(format!("{}.json", reply.id));
    // A leading dot: the script looks for `<id>.json` only.
    let temporary = replies.join(format!(".{}.tmp", reply.id));
    let bytes = serde_json::to_vec(reply).map_err(io::Error::other)?;
    let mut file = File::create(&temporary)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    drop(file);
    fs::rename(&temporary, &target)?;
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Result<Request, Reject> {
        parse_request("r1", text.as_bytes())
    }

    #[test]
    fn a_press_parses_with_and_without_the_optional_fields() {
        let request =
            parse(r#"{"id":"r1","window":"main","step":{"kind":"press","name":"Play"}}"#).unwrap();
        assert_eq!(request.window, Window::Main);
        assert_eq!(
            request.step,
            Step::Press {
                name: "Play".into(),
                role: None,
                nth: None
            }
        );
        let request = parse(
            r#"{"id":"r1","window":"settings","step":{"kind":"press","name":"x","role":"button","nth":2}}"#,
        )
        .unwrap();
        assert_eq!(request.window.label(), "settings");
        assert!(matches!(request.step, Step::Press { nth: Some(2), .. }));
    }

    #[test]
    fn every_step_kind_parses() {
        for step in [
            r#"{"kind":"press","name":"a"}"#,
            r#"{"kind":"menu","name":"a"}"#,
            r#"{"kind":"type","text":"abc"}"#,
            r#"{"kind":"type","name":"Search","text":"abc"}"#,
            r#"{"kind":"key","key":"k","code":"KeyK","mods":["meta","shift"]}"#,
            r#"{"kind":"wait","name":"a","gone":true,"timeoutMs":5000}"#,
            r#"{"kind":"has","name":"a"}"#,
            r#"{"kind":"scroll","name":"a"}"#,
            r#"{"kind":"text"}"#,
            r#"{"kind":"front"}"#,
        ] {
            let text = format!(r#"{{"id":"r1","window":"capture","step":{step}}}"#);
            assert!(parse(&text).is_ok(), "{step}");
        }
    }

    #[test]
    fn the_step_is_forwarded_in_the_shape_the_page_reads() {
        let request = parse(
            r#"{"id":"r1","window":"main","step":{"kind":"wait","name":"a","timeoutMs":900}}"#,
        )
        .unwrap();
        assert_eq!(
            serde_json::to_value(&request.step).unwrap(),
            serde_json::json!({"kind": "wait", "name": "a", "timeoutMs": 900})
        );
    }

    #[test]
    fn garbage_is_refused_with_a_category_and_never_panics() {
        let cases: &[(&str, Reject)] = &[
            ("", Reject::BadJson),
            ("not json", Reject::BadJson),
            ("[]", Reject::BadJson),
            ("null", Reject::BadJson),
            ("\"r1\"", Reject::BadJson),
            ("{", Reject::BadJson),
            ("{}", Reject::IdMismatch),
            (
                r#"{"id":"other","window":"main","step":{"kind":"text"}}"#,
                Reject::IdMismatch,
            ),
            (
                r#"{"id":7,"window":"main","step":{"kind":"text"}}"#,
                Reject::IdMismatch,
            ),
            (
                r#"{"id":"r1","step":{"kind":"text"}}"#,
                Reject::UnknownWindow,
            ),
            (
                r#"{"id":"r1","window":"Main","step":{"kind":"text"}}"#,
                Reject::UnknownWindow,
            ),
            (
                r#"{"id":"r1","window":"debug","step":{"kind":"text"}}"#,
                Reject::UnknownWindow,
            ),
            (r#"{"id":"r1","window":"main"}"#, Reject::UnknownStep),
            (
                r#"{"id":"r1","window":"main","step":"text"}"#,
                Reject::UnknownStep,
            ),
            (
                r#"{"id":"r1","window":"main","step":{"kind":"eval","code":"1"}}"#,
                Reject::UnknownStep,
            ),
            (
                r#"{"id":"r1","window":"main","step":{"kind":"invoke","cmd":"x"}}"#,
                Reject::UnknownStep,
            ),
            (
                r#"{"id":"r1","window":"main","step":{"kind":7}}"#,
                Reject::UnknownStep,
            ),
            (
                r#"{"id":"r1","window":"main","step":{"kind":"press"}}"#,
                Reject::InvalidStep,
            ),
            (
                r#"{"id":"r1","window":"main","step":{"kind":"press","name":5}}"#,
                Reject::InvalidStep,
            ),
            (
                r#"{"id":"r1","window":"main","step":{"kind":"press","name":""}}"#,
                Reject::InvalidStep,
            ),
            (
                r#"{"id":"r1","window":"main","step":{"kind":"press","name":"a","extra":1}}"#,
                Reject::InvalidStep,
            ),
            (
                r#"{"id":"r1","window":"main","step":{"kind":"press","name":"a","nth":-1}}"#,
                Reject::InvalidStep,
            ),
            (
                r#"{"id":"r1","window":"main","step":{"kind":"press","name":"a","nth":99999}}"#,
                Reject::InvalidStep,
            ),
            (
                r#"{"id":"r1","window":"main","step":{"kind":"press","name":"a","role":"Button"}}"#,
                Reject::InvalidStep,
            ),
            (
                r#"{"id":"r1","window":"main","step":{"kind":"wait","name":"a"}}"#,
                Reject::InvalidStep,
            ),
            (
                r#"{"id":"r1","window":"main","step":{"kind":"wait","name":"a","timeoutMs":999999999}}"#,
                Reject::InvalidStep,
            ),
            (
                r#"{"id":"r1","window":"main","step":{"kind":"key","key":""}}"#,
                Reject::InvalidStep,
            ),
            (
                r#"{"id":"r1","window":"main","step":{"kind":"key","key":"k","mods":["super"]}}"#,
                Reject::InvalidStep,
            ),
            (
                r#"{"id":"r1","window":"main","step":{"kind":"type"}}"#,
                Reject::InvalidStep,
            ),
            (
                r#"{"id":"r1","window":"main","step":{"kind":"text","name":"x"}}"#,
                Reject::InvalidStep,
            ),
            (
                r#"{"id":"r1","window":"main","step":{"kind":"text"},"extra":1}"#,
                Reject::InvalidStep,
            ),
        ];
        for (text, want) in cases {
            assert_eq!(parse(text), Err(*want), "{text}");
        }
        // Invalid UTF-8 and a NUL.
        assert_eq!(
            parse_request("r1", &[0xff, 0xfe, 0x00]),
            Err(Reject::BadJson)
        );
        assert_eq!(parse_request("r1", b"\0\0\0"), Err(Reject::BadJson));
    }

    #[test]
    fn too_long_names_and_texts_are_refused() {
        let long = "я".repeat(MAX_NAME_CHARS + 1);
        let text =
            format!(r#"{{"id":"r1","window":"main","step":{{"kind":"press","name":"{long}"}}}}"#);
        assert_eq!(parse(&text), Err(Reject::InvalidStep));
        let long = "x".repeat(MAX_TYPED_CHARS + 1);
        let text =
            format!(r#"{{"id":"r1","window":"main","step":{{"kind":"type","text":"{long}"}}}}"#);
        assert_eq!(parse(&text), Err(Reject::InvalidStep));
    }

    #[test]
    fn an_id_that_is_a_path_is_not_an_id() {
        for bad in [
            "",
            ".",
            "..",
            "../x",
            "a/b",
            "a\\b",
            "a.b",
            "/etc/passwd",
            "a b",
            "a\0b",
            "é",
            "..\\..\\x",
            "x/..",
            "%2e%2e",
        ] {
            assert!(!valid_id(bad), "{bad:?}");
        }
        assert!(!valid_id(&"a".repeat(65)));
        for good in ["a", "r-1", "1730-42_7", "ABC", &"a".repeat(64)] {
            assert!(valid_id(good), "{good}");
        }
        assert_eq!(id_of_file_name("r1.json"), Some("r1"));
        for bad in [
            "r1.json.tmp",
            "..json",
            ".json",
            "r1.txt",
            "a b.json",
            "../r1.json",
            "r1",
            ".r1.json",
        ] {
            assert_eq!(id_of_file_name(bad), None, "{bad}");
        }
    }

    #[test]
    fn a_reply_is_written_atomically_inside_the_replies_directory() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_reply(dir.path(), &Reply::ok("r1", Some("yes".into()))).unwrap();
        assert_eq!(path, dir.path().join("replies").join("r1.json"));
        let text = fs::read_to_string(&path).unwrap();
        assert_eq!(text, r#"{"id":"r1","ok":true,"value":"yes"}"#);
        let text =
            fs::read_to_string(write_reply(dir.path(), &Reply::error("r2", "not found")).unwrap())
                .unwrap();
        assert_eq!(text, r#"{"id":"r2","ok":false,"error":"not found"}"#);
        let names: Vec<_> = fs::read_dir(dir.path().join("replies"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(names.len(), 2, "no temporary file is left: {names:?}");
    }

    #[test]
    fn a_reply_with_a_bad_id_writes_nothing() {
        let parent = tempfile::tempdir().unwrap();
        let dir = parent.path().join("driver");
        fs::create_dir(&dir).unwrap();
        for bad in ["../escape", "a/b", "", "..", "x\0y"] {
            assert!(write_reply(&dir, &Reply::ok(bad, None)).is_err(), "{bad:?}");
        }
        assert!(!parent.path().join("escape.json").exists());
        assert!(!dir.join("replies").exists());
    }

    #[test]
    fn request_files_are_read_with_a_size_limit_and_only_if_regular() {
        let dir = tempfile::tempdir().unwrap();
        let small = dir.path().join("a.json");
        fs::write(&small, b"{}").unwrap();
        assert_eq!(read_request_file(&small).unwrap(), b"{}");
        let big = dir.path().join("b.json");
        fs::write(&big, vec![b' '; MAX_REQUEST_BYTES as usize + 1]).unwrap();
        assert_eq!(read_request_file(&big), Err(Reject::TooLarge));
        let exact = dir.path().join("c.json");
        fs::write(&exact, vec![b' '; MAX_REQUEST_BYTES as usize]).unwrap();
        assert!(read_request_file(&exact).is_ok());
        assert_eq!(read_request_file(dir.path()), Err(Reject::NotAFile));
        assert_eq!(
            read_request_file(&dir.path().join("none.json")),
            Err(Reject::Unreadable)
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_link_is_not_followed() {
        let dir = tempfile::tempdir().unwrap();
        let secret = dir.path().join("secret");
        fs::write(&secret, b"{}").unwrap();
        let link = dir.path().join("l.json");
        std::os::unix::fs::symlink(&secret, &link).unwrap();
        assert_eq!(read_request_file(&link), Err(Reject::NotAFile));
    }
}
