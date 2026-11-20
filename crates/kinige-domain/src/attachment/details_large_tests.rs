//! Slice 7b: the read function and a very large stored value (split from `details_tests.rs`, which hit the 400-line cap).

use super::*;
use serde_json::json;

fn hlc(l: u64) -> String {
    format!("{l:013}-00000-5f0c3e2a-9b1d-4c7e-8a64-0d2f1b3c4e5f")
}

#[test]
fn a_900_kib_description_reads_as_none_and_does_not_fail() {
    let huge = "a".repeat(900 * 1024);
    let groups = json!({"description": {"description": huge}, "name": {"name": null}}).to_string();
    let hlcs = json!({"description": {"h": hlc(1), "sv": 1}, "name": {"h": hlc(1), "sv": 1}});
    let details = read_details(AttachmentKind::Image, "a.jpg", &groups, &hlcs.to_string());
    assert_eq!(details.description, None);
    assert!(!details.spoiler);
}
