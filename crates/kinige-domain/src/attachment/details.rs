//! The details of an attachment (slice 7b, ADR-0001 appendix §C.8): a display name, a description and a spoiler flag, each in a
//! group of its own (`name`, `description`, `privacy`). This module counts and normalizes what a person typed, builds the bytes of
//! the three groups and reads them back. Reading is total: it never errors, so a record from another client can always be shown.

use super::names::{ext_suffix, extension, sanitize_filename};
use super::{AttachmentKind, MAX_FILENAME_BYTES};
use crate::groups::SCHEMA_VERSION;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer};
use serde_json::{Map, Value, json, value::RawValue};
use std::collections::BTreeMap;
use std::fmt;
use ts_rs::TS;

/// The longest description, in Unicode scalar values after normalization (Discord's alt-text limit; not verified).
pub const MAX_DESCRIPTION_CHARS: usize = 1024;
/// The same limit in UTF-8 bytes: four bytes per scalar value is the most, so it always holds with the count above, and the groups'
/// bytes stay far below the envelope's 1 MiB (ADR-0001 §5.3).
pub const MAX_DESCRIPTION_BYTES: usize = 4096;

/// The groups the details live in.
pub const NAME_GROUP: &str = "name";
pub const DESCRIPTION_GROUP: &str = "description";
pub const PRIVACY_GROUP: &str = "privacy";

/// A detail the rules refuse (`attachment_details_invalid`). The text names the rule, never the value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DetailsError(pub &'static str);

impl fmt::Display for DetailsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "attachment details are invalid: {}", self.0)
    }
}

impl std::error::Error for DetailsError {}

/// The changes a command carries (`set_attachment_details`, `set_draft_attachment_details`): each field is "leave" (the key is
/// absent) or a value. `null` clears the name or the description; the spoiler takes `true` or `false`, never `null`; a key that is not
/// one of the three is refused. Counting and normalizing are Rust's (`normalize_name`, `normalize_description`).
#[derive(Debug, Clone, Default, PartialEq, Eq, TS)]
#[ts(export, export_to = "AttachmentDetailsChanges.ts")]
pub struct AttachmentDetailsChanges {
    /// The stem of the display name: the original extension is a fixed suffix Rust appends. Empty or `null` is the original.
    #[ts(optional = nullable, as = "Option<String>")]
    pub name: Option<Option<String>>,
    /// Plain text; empty or `null` is none.
    #[ts(optional = nullable, as = "Option<String>")]
    pub description: Option<Option<String>>,
    /// Only for an `image`. `null` is refused.
    #[ts(optional, as = "Option<bool>")]
    pub spoiler: Option<bool>,
}

/// The wire form: only an object reads (serde would also read a struct from an array, so `["x"]` would be a name), no unknown key, no
/// `null` for the spoiler. An error says what is wrong, never the value: serde's own text quotes an unexpected string, and an error
/// reaches the frontend and may reach a log.
impl<'de> Deserialize<'de> for AttachmentDetailsChanges {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            #[serde(default, with = "sent_value")]
            name: Option<Option<String>>,
            #[serde(default, with = "sent_value")]
            description: Option<Option<String>>,
            #[serde(default, with = "sent_flag")]
            spoiler: Option<bool>,
        }
        let value = Value::deserialize(deserializer)?;
        if !value.is_object() {
            return Err(D::Error::custom("the changes must be an object"));
        }
        let wire: Wire = serde_json::from_value(value)
            .map_err(|_| D::Error::custom("the changes do not read"))?;
        Ok(AttachmentDetailsChanges {
            name: wire.name,
            description: wire.description,
            spoiler: wire.spoiler,
        })
    }
}

/// A key that is present: its value, `null` included. (A key that is absent never reaches here: `default` leaves `None`.)
mod sent_value {
    use serde::{Deserialize, Deserializer};

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<Option<String>>, D::Error> {
        Option::<String>::deserialize(deserializer).map(Some)
    }
}

/// A key that is present must hold a boolean: `null` is an error, not "leave".
mod sent_flag {
    use serde::{Deserialize, Deserializer};

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<bool>, D::Error> {
        bool::deserialize(deserializer).map(Some)
    }
}

/// What the three groups amount to for a row: the output of [`read_details`], also what the table columns hold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Details {
    /// The whole display name with its extension; `None` shows the original `filename`.
    pub display_name: Option<String>,
    /// Plain text; `None` is none.
    pub description: Option<String>,
    /// Covered in the window. Only an `image` can be.
    pub spoiler: bool,
}

/// Which fields a kind takes. An `image` takes all three, a `file` a name and a description (not a spoiler), and an `audio` (a voice note,
/// or a song attached as a file) takes none: the window shows no details for it, so a value Rust accepted would be synced bytes that the
/// window could neither show nor undo. Refused at the input, whatever the value (`null` included). [`read_details`] stays kind-agnostic
/// for the name and the description, so a later slice can allow them for `audio` without a change of the format.
pub fn check_kind(
    kind: AttachmentKind,
    changes: &AttachmentDetailsChanges,
) -> Result<(), DetailsError> {
    let any = changes.name.is_some() || changes.description.is_some() || changes.spoiler.is_some();
    match kind {
        AttachmentKind::Audio if any => Err(DetailsError("an audio attachment has no details")),
        _ if kind != AttachmentKind::Image && changes.spoiler.is_some() => {
            Err(DetailsError("only a photo can be hidden"))
        }
        _ => Ok(()),
    }
}

/// The name the person typed (the stem) as the whole display name, or `None` for "the original": trimmed, no C0, DEL, C1, path
/// separator or bidi override, the extension of `original` appended, at most 255 bytes. An empty stem and a name equal to the
/// original are `None`. The rule the read function applies to a stored name is the same ([`valid_display_name`]).
pub fn normalize_name(stem: &str, original: &str) -> Result<Option<String>, DetailsError> {
    let stem = stem.trim();
    if stem.is_empty() {
        return Ok(None);
    }
    let bad = |c: char| c.is_control() || matches!(c, '/' | '\\') || is_bidi_override(c);
    if stem.chars().any(bad) {
        return Err(DetailsError("name has a forbidden character"));
    }
    let full = format!("{stem}{}", ext_suffix(original));
    if full == original {
        return Ok(None);
    }
    if full.len() > MAX_FILENAME_BYTES {
        return Err(DetailsError("name is longer than 255 bytes"));
    }
    if !valid_display_name(&full, original) {
        return Err(DetailsError("name is not a file name"));
    }
    Ok(Some(full))
}

/// A description as stored: CRLF and a lone CR become LF, a TAB becomes one space, the ends are trimmed. A control character left
/// (C0 other than LF, DEL, C1) is refused, then the length is counted in scalar values. Empty is `None`.
pub fn normalize_description(raw: &str) -> Result<Option<String>, DetailsError> {
    let unified = raw
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\t', " ");
    let text = unified.trim();
    if text.chars().any(|c| c.is_control() && c != '\n') {
        return Err(DetailsError("description has a control character"));
    }
    if text.is_empty() {
        return Ok(None);
    }
    if text.chars().count() > MAX_DESCRIPTION_CHARS || text.len() > MAX_DESCRIPTION_BYTES {
        return Err(DetailsError("description is too long"));
    }
    Ok(Some(text.to_string()))
}

/// Whether a stored display name may be shown: 1 to 255 bytes, a fixed point of the filename rules of slice 7 (no path, control
/// or bidi characters, nothing to trim), and the extension of `original` (bytes, or ASCII-folded bytes; never Unicode lowercasing).
pub fn valid_display_name(name: &str, original: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_FILENAME_BYTES
        && sanitize_filename(name) == name
        && extension(name).eq_ignore_ascii_case(extension(original))
}

/// Whether a stored description may be shown: it is its own normal form, and within the limits.
fn valid_description(text: &str) -> bool {
    matches!(normalize_description(text), Ok(Some(ref normal)) if normal == text)
}

fn is_bidi_override(c: char) -> bool {
    matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
}

/// The canonical value of the `name` group (appendix §C.8): `{"name":<string|null>}`.
pub fn name_group(display_name: Option<&str>) -> Value {
    json!({"name": display_name})
}

/// The canonical value of the `description` group: `{"description":<string|null>}`.
pub fn description_group(description: Option<&str>) -> Value {
    json!({"description": description})
}

/// The canonical value of the `privacy` group: `{"spoiler":<bool>}`.
pub fn privacy_group(spoiler: bool) -> Value {
    json!({"spoiler": spoiler})
}

/// The details of a record from its merged groups: a total function of the bytes, never an error (appendix §C.8).
///
/// The spoiler table fails closed. An `image` whose `privacy` group is there but cannot be understood is covered:
/// - the group is not an object, or its `sv` is unsupported, unknown or missing, or only one of `groups_raw` and `hlc_json` has it;
/// - `spoiler` is `true`, `null`, absent or not a boolean.
///
/// An absent group, or `{"spoiler":false}`, is uncovered. Any value on another kind is ignored (kept verbatim, uncovered). A `name`
/// that is not a valid display name shows the original, a bad `description` shows none, and a group in an unsupported `sv` shows
/// the default. If `groups_raw` itself cannot be read, an image is covered and nothing else is known.
pub fn read_details(
    kind: AttachmentKind,
    original: &str,
    groups_raw: &str,
    hlc_json: &str,
) -> Details {
    let image = kind == AttachmentKind::Image;
    let Ok(groups) = serde_json::from_str::<BTreeMap<String, Box<RawValue>>>(groups_raw) else {
        return Details {
            display_name: None,
            description: None,
            spoiler: image,
        };
    };
    let hlcs: Option<BTreeMap<String, Value>> = serde_json::from_str(hlc_json).ok();
    let group = |name: &str| -> Option<Map<String, Value>> {
        let supported = hlcs
            .as_ref()
            .and_then(|map| map.get(name))
            .and_then(|entry| entry.get("sv"))
            .and_then(Value::as_u64)
            .is_some_and(|sv| sv <= u64::from(SCHEMA_VERSION));
        if !supported {
            return None;
        }
        match serde_json::from_str(groups.get(name)?.get()) {
            Ok(Value::Object(object)) => Some(object),
            _ => None,
        }
    };
    let display_name = group(NAME_GROUP)
        .and_then(|object| {
            object
                .get("name")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .filter(|name| valid_display_name(name, original));
    let description = group(DESCRIPTION_GROUP)
        .and_then(|object| {
            object
                .get("description")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .filter(|text| valid_description(text));
    let spoiler = image && {
        let in_bytes = groups.contains_key(PRIVACY_GROUP);
        let in_hlc = hlcs
            .as_ref()
            .is_none_or(|map| map.contains_key(PRIVACY_GROUP));
        match (in_bytes, in_hlc) {
            (false, false) => false,
            _ => group(PRIVACY_GROUP)
                .is_none_or(|object| object.get("spoiler") != Some(&Value::Bool(false))),
        }
    };
    Details {
        display_name,
        description,
        spoiler,
    }
}
