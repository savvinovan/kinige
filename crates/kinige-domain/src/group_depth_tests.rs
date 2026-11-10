//! The depth of a group's value, on the path a payload takes (ADR-0001 appendix A.4, client step 5): `assemble` writes the groups,
//! `parse_groups_raw` reads the outer object with every group as a `RawValue`, and each group's bytes are parsed on their own. The
//! limit of 127 nested arrays and objects is checked by that last parse; `parse_groups_raw` itself bounds nothing, because serde_json
//! skips raw bytes without its recursion counter.

use crate::groups::{assemble, parse_groups_raw};
use crate::jcs::MAX_DEPTH;
use serde::Deserialize;
use serde_json::value::RawValue;
use serde_json::{Value, json};
use std::collections::BTreeMap;

const HLC: &str = "0000000000001-00000-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

#[derive(Deserialize)]
struct Payload {
    groups: BTreeMap<String, Box<RawValue>>,
}

#[test]
fn a_group_of_depth_127_is_written_and_read_back_group_by_group() {
    let deep = nested(MAX_DEPTH - 1);
    let (groups_raw, _) = assemble(&[("core", deep.clone())], HLC).unwrap();
    let groups = parse_groups_raw(&groups_raw).unwrap();
    let bytes = groups["core"].get();
    assert_eq!(serde_json::from_str::<Value>(bytes).unwrap(), deep);
}

#[test]
fn a_group_of_depth_128_is_not_written_and_is_refused_when_it_arrives() {
    assert!(assemble(&[("core", nested(MAX_DEPTH))], HLC).is_err());
    // From outside: the outer parse takes it, the standalone parse of the group refuses it.
    let hostile = format!(r#"{{"core":{}}}"#, nest_text(MAX_DEPTH));
    let groups = parse_groups_raw(&hostile).unwrap();
    assert!(serde_json::from_str::<Value>(groups["core"].get()).is_err());
}

#[test]
fn the_outer_parse_does_not_bound_the_depth_so_the_group_parse_must() {
    let far = nest_text(100_000);
    let groups = parse_groups_raw(&format!(r#"{{"core":{far}}}"#)).unwrap();
    assert_eq!(
        groups["core"].get().len(),
        far.len(),
        "the bytes are kept as they are"
    );
    assert!(serde_json::from_str::<Value>(groups["core"].get()).is_err());
}

#[test]
fn the_limit_counts_from_the_group_not_from_the_whole_payload() {
    let payload = format!(r#"{{"groups":{{"core":{}}}}}"#, nest_text(MAX_DEPTH - 1));
    // Group by group: the group of depth 127 is read.
    let parsed: Payload = serde_json::from_str(&payload).unwrap();
    assert!(serde_json::from_str::<Value>(parsed.groups["core"].get()).is_ok());
    // The whole payload as one `Value` counts the two containers around the group too: an honest group would be refused.
    assert!(serde_json::from_str::<Value>(&payload).is_err());
}

/// `levels` arrays around a number.
fn nested(levels: usize) -> Value {
    let mut value = json!(1);
    for _ in 0..levels {
        value = Value::Array(vec![value]);
    }
    value
}

fn nest_text(levels: usize) -> String {
    format!("{}1{}", "[".repeat(levels), "]".repeat(levels))
}
