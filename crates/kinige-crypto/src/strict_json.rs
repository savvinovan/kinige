//! JSON without duplicate keys (ADR-0001 §5.3, §A.4 step 5). `serde_json` keeps the last of two equal keys silently, and a hostile
//! or faulty writer could use that to show two readers two different values; this walks the whole text and refuses the first
//! repeated key at any depth. Keys are compared after unescaping, so `"a"` and `"a"` are the same key.
//!
//! The nesting limit is `serde_json`'s: 128 levels (appendix §A.4 step 5), so garbage like `[[[[...` is an error and not a stack
//! overflow.

use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use std::collections::HashSet;
use std::fmt;

/// `Ok` when `bytes` is exactly one JSON value, valid and without a repeated object key at any depth.
pub(crate) fn check_no_duplicate_keys(bytes: &[u8]) -> Result<(), &'static str> {
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    NoDuplicates
        .deserialize(&mut deserializer)
        .map_err(|error| {
            if error.to_string().starts_with("duplicate key") {
                "duplicate key"
            } else {
                "not json"
            }
        })?;
    deserializer.end().map_err(|_| "not json")
}

struct NoDuplicates;

impl<'de> DeserializeSeed<'de> for NoDuplicates {
    type Value = ();

    fn deserialize<D: de::Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for NoDuplicates {
    type Value = ();

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a json value")
    }

    fn visit_bool<E>(self, _: bool) -> Result<(), E> {
        Ok(())
    }

    fn visit_i64<E>(self, _: i64) -> Result<(), E> {
        Ok(())
    }

    fn visit_u64<E>(self, _: u64) -> Result<(), E> {
        Ok(())
    }

    fn visit_f64<E>(self, _: f64) -> Result<(), E> {
        Ok(())
    }

    fn visit_str<E>(self, _: &str) -> Result<(), E> {
        Ok(())
    }

    fn visit_unit<E>(self) -> Result<(), E> {
        Ok(())
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        while seq.next_element_seed(NoDuplicates)?.is_some() {}
        Ok(())
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        let mut seen: HashSet<String> = HashSet::new();
        while let Some(key) = map.next_key::<String>()? {
            if !seen.insert(key) {
                return Err(de::Error::custom("duplicate key"));
            }
            map.next_value_seed(NoDuplicates)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_json_passes() {
        for text in [
            r#"{}"#,
            r#"{"a":1,"b":[1,2,{"c":null}],"d":"x"}"#,
            r#"[{"a":1},{"a":2}]"#,
            r#"{"a":{"a":1}}"#,
            "  true ",
        ] {
            assert_eq!(check_no_duplicate_keys(text.as_bytes()), Ok(()), "{text}");
        }
    }

    #[test]
    fn a_repeated_key_is_refused_at_any_depth_and_after_unescaping() {
        // `"a"` is `"a"` after unescaping; the backslash is put in at run time.
        let escaped = format!("{{\"a\":1,\"{}u0061\":2}}", '\\');
        for text in [
            r#"{"a":1,"a":2}"#,
            escaped.as_str(),
            r#"{"x":{"y":[{"k":1,"k":1}]}}"#,
            r#"{"groups":{"body":{"body_md":"a","body_md":"b"}}}"#,
        ] {
            assert_eq!(
                check_no_duplicate_keys(text.as_bytes()),
                Err("duplicate key"),
                "{text}"
            );
        }
    }

    #[test]
    fn garbage_is_an_error_and_never_a_panic() {
        let deep = "[".repeat(10_000);
        let long_key_chain = r#"{"a":"#.repeat(1_000);
        let samples: [&[u8]; 9] = [
            b"",
            b"{",
            b"{\"a\":",
            b"\xff\xfe\x00",
            b"{} {}",
            b"{\"a\":1,}",
            b"nul",
            deep.as_bytes(),
            long_key_chain.as_bytes(),
        ];
        for sample in samples {
            assert!(check_no_duplicate_keys(sample).is_err());
        }
    }
}
