//! Lenient value types for `result.json` fields (used by `raw.rs`): a number or string id, a key whose presence matters, a value
//! of an unexpected kind that must not fail the import, a number in several spellings.

use serde::Deserialize;
use serde::de::IgnoredAny;

/// An identifier or a number that came as a string or as a number. `untagged` buffers a number as `i64`/`u64`, not `f64`,
/// so a fractional or too large number fits no variant, and an integer does not lose precision.
#[derive(Deserialize)]
#[serde(untagged)]
pub(super) enum Flex {
    Str(String),
    Int(i64),
    Uint(u64),
}

impl Flex {
    pub(super) fn into_string(self) -> String {
        match self {
            Flex::Str(s) => s,
            Flex::Int(n) => n.to_string(),
            Flex::Uint(n) => n.to_string(),
        }
    }
}

/// A field where the presence of the key itself matters: absent is `None`, `null` is `Some(None)`.
pub(super) fn present<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<Option<String>>, D::Error> {
    Option::<String>::deserialize(d).map(Some)
}

/// A value of the expected kind, or anything else (kept as "not stated"): the key being present is still known.
#[derive(Deserialize)]
#[serde(untagged)]
pub(super) enum Lenient<T> {
    Value(T),
    Other(IgnoredAny),
}

impl<T> Lenient<T> {
    pub(super) fn value(&self) -> Option<&T> {
        match self {
            Lenient::Value(value) => Some(value),
            Lenient::Other(_) => None,
        }
    }
}

/// A number that came as an integer, a fraction or a string.
#[derive(Deserialize)]
#[serde(untagged)]
pub(super) enum Num {
    Int(i64),
    Uint(u64),
    Float(f64),
    Str(String),
}

impl Num {
    /// A whole non-negative number, or `None`.
    fn as_u64(&self) -> Option<u64> {
        match self {
            Num::Int(n) => u64::try_from(*n).ok(),
            Num::Uint(n) => Some(*n),
            Num::Float(f) if f.is_finite() && *f >= 0.0 && *f < 9.0e15 => Some(*f as u64),
            Num::Float(_) => None,
            Num::Str(s) => s.trim().parse().ok(),
        }
    }
}

pub(super) fn num(field: &Option<Lenient<Num>>) -> Option<u64> {
    field.as_ref()?.value()?.as_u64()
}
