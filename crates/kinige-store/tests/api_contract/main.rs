//! The API contract (KINIG-104): the rules that the web mock repeats from Rust, pinned by the shared fixtures in
//! `fixtures/api-contract/`. The web side (`web/src/api/apiContract.test.ts`) reads the same files, so a rule cannot change on one
//! side only. `rules` runs the pure functions of `kinige-domain`; `flow` runs the scenarios on a real `Store`.

// A whole integration-test root is test code: `cfg(test)` gives helpers outside #[test] the same clippy.toml allowances
// (unwrap/expect/panic/indexing) as the tests in src/.
#![cfg(test)]

#[path = "../common/mod.rs"]
mod common;
mod flow;
mod rules;

use serde_json::Value;
use std::path::PathBuf;

/// The JSON of a fixture file.
fn fixture(name: &str) -> Value {
    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "fixtures",
        "api-contract",
        &format!("{name}.json"),
    ]
    .iter()
    .collect();
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The list under `key`, which every fixture has.
fn list<'a>(file: &'a Value, key: &str) -> &'a [Value] {
    file[key]
        .as_array()
        .unwrap_or_else(|| panic!("`{key}` is not a list"))
}

fn text<'a>(case: &'a Value, key: &str) -> &'a str {
    case[key]
        .as_str()
        .unwrap_or_else(|| panic!("`{key}` is not a string in {case}"))
}

/// A text written as a string or as a list of parts, where `[unit, n]` is the unit repeated `n` times.
fn spec(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Array(parts) => parts
            .iter()
            .map(|part| match part {
                Value::Array(repeat) => {
                    let times = usize::try_from(repeat[1].as_u64().unwrap()).unwrap();
                    repeat[0].as_str().unwrap().repeat(times)
                }
                other => spec(other),
            })
            .collect(),
        other => panic!("not a text: {other}"),
    }
}

fn opt_text<'a>(case: &'a Value, key: &str) -> Option<&'a str> {
    case[key].as_str()
}

fn int(case: &Value, key: &str) -> i64 {
    case[key]
        .as_i64()
        .unwrap_or_else(|| panic!("`{key}` is not a number in {case}"))
}
