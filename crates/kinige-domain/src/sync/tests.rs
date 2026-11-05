use super::conflict::{BODY, PLACEMENT, content_differs, uuid_bytes, uuid_from_bytes};
use super::*;
use crate::groups::{NewMessage, new_message};
use crate::hlc::Clock;

const A: &str = "5f0c3e2a-9b1d-4c7e-8a64-0d2f1b3c4e5f";
const B: &str = "7a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d";
const MESSAGE: &str = "11111111-2222-4333-8444-555555555555";

fn hlc(l: u64, c: u32, device: &str) -> String {
    Clock { l, c }.format(device)
}

fn body(text: &str, base: Option<&str>, supersedes: &[&str]) -> String {
    serde_json::json!({"base": base, "body_md": text, "edited_at": null, "supersedes": supersedes})
        .to_string()
}

fn group(h: &str, raw: &str) -> Group {
    Group::new(h, 1, raw).unwrap()
}

fn record(groups: &[(&str, Group)]) -> Versioned {
    let mut v = Versioned::new();
    for (name, g) in groups {
        v.set(name, g.clone());
    }
    v
}

#[test]
fn columns_round_trip_through_the_app_format() {
    let row = new_message(
        &NewMessage {
            id: MESSAGE,
            chat_id: "00000000-0000-4000-8000-000000000001",
            body_md: "hello",
            created_at: 1,
            created_tz: "UTC",
            journal_date: "2026-03-14",
        },
        &hlc(1_000, 0, A),
    )
    .unwrap();
    let parsed = Versioned::from_columns(&row.groups_raw, &row.hlc_json).unwrap();
    assert_eq!(parsed.snapshot().len(), 5);
    let (groups_raw, hlc_json) = parsed.to_columns().unwrap();
    assert_eq!(groups_raw, row.groups_raw);
    assert_eq!(hlc_json, row.hlc_json);
}

#[test]
fn corrupt_columns_are_errors_not_panics() {
    let good_hlc = hlc(1, 0, A);
    let hlc_json = format!(r#"{{"body":{{"h":"{good_hlc}","sv":1}}}}"#);
    let cases = [
        ("", "{}"),
        ("[]", "{}"),
        (r#"{"body":{}}"#, "not json"),
        (r#"{"body":{}}"#, "{}"),
        (r#"{"body":{}}"#, r#"{"body":{"h":"x","sv":1}}"#),
        (r#"{"body":{}}"#, r#"{"body":{"h":"1","sv":-1}}"#),
        (r#"{"body":{}}"#, r#"{"body":{"sv":1}}"#),
        (
            r#"{"body":{}}"#,
            &format!(
                r#"{{"body":{{"h":"{good_hlc}","sv":1}},"flags":{{"h":"{good_hlc}","sv":1}}}}"#
            ),
        ),
    ];
    for (raw, hlcs) in cases {
        assert!(Versioned::from_columns(raw, hlcs).is_err(), "{raw} {hlcs}");
    }
    assert!(Versioned::from_columns(r#"{"body":{}}"#, &hlc_json).is_ok());
}

#[test]
fn a_group_needs_a_strict_stamp_and_json_bytes() {
    let good = hlc(5, 1, A);
    assert!(Group::new(&good, 1, "{}").is_ok());
    for bad in [
        "",
        &good.replace(A, "NOT-A-DEVICE"),
        good.get(..55).unwrap(),
        &format!("{good}x"),
        &good.replace("0000000000005", "00000000005"),
    ] {
        assert!(Group::new(bad, 1, "{}").is_err(), "{bad}");
    }
    assert!(Group::new(&good, 1, "{not json").is_err());
    assert!(Group::new(&good, 1, "").is_err());
}

#[test]
fn merge_takes_the_larger_hlc_per_group_and_is_order_independent() {
    let old = hlc(10, 0, A);
    let new = hlc(20, 0, B);
    let x = record(&[
        ("body", group(&old, &body("a", None, &[]))),
        ("flags", group(&new, r#"{"is_task":true}"#)),
    ]);
    let y = record(&[
        ("body", group(&new, &body("b", Some(&old), &[]))),
        ("flags", group(&old, r#"{"is_task":false}"#)),
        ("deleted", group(&old, r#"{"deleted_at":null}"#)),
    ]);
    let xy = merge(&x, &y);
    let yx = merge(&y, &x);
    assert_eq!(xy.record, yx.record, "commutative");
    assert_eq!(xy.record.get("body").unwrap().hlc, new, "body from y");
    assert_eq!(
        xy.record.get("flags").unwrap().hlc,
        new,
        "flags from x: different groups are both kept"
    );
    assert!(
        xy.record.get("deleted").is_some(),
        "a group missing on one side is taken from the other"
    );
    assert_eq!(merge(&xy.record, &y).record, xy.record, "idempotent");
    assert!(!merge(&xy.record, &y).changed);
}

#[test]
fn equal_hlc_with_different_bytes_is_resolved_by_the_larger_bytes() {
    let h = hlc(10, 0, A);
    let x = record(&[("flags", group(&h, r#"{"is_task":false}"#))]);
    let y = record(&[("flags", group(&h, r#"{"is_task":true}"#))]);
    let xy = merge(&x, &y);
    assert_eq!(xy.equal_hlc_different_bytes, ["flags"]);
    assert_eq!(
        xy.record,
        merge(&y, &x).record,
        "the choice does not depend on the side"
    );
    assert_eq!(xy.record.get("flags").unwrap().raw, r#"{"is_task":true}"#);
}

#[test]
fn unknown_groups_and_a_newer_sv_are_kept_as_they_arrived() {
    let h = hlc(10, 0, A);
    let future = Group::new(&h, 7, r#"{"z":[1,2,3],"new_field":"x"}"#).unwrap();
    let x = record(&[("body", group(&h, &body("a", None, &[])))]);
    let y = record(&[("shiny_new_group", future.clone())]);
    let merged = merge(&x, &y).record;
    assert_eq!(merged.get("shiny_new_group"), Some(&future));
    assert_eq!(merged.schema_version(), 7);
    assert!(
        merged.is_read_only(),
        "a group from the future makes the record read-only (§4 item 4)"
    );
    assert!(!x.is_read_only());
    let (raw, hlcs) = merged.to_columns().unwrap();
    assert_eq!(
        Versioned::from_columns(&raw, &hlcs).unwrap(),
        merged,
        "raw bytes and sv survive the columns"
    );
}

#[test]
fn a_losing_group_is_reported_with_the_side_it_came_from() {
    let old = hlc(10, 0, A);
    let new = hlc(20, 0, B);
    let local = record(&[("body", group(&old, &body("mine", None, &[])))]);
    let incoming = record(&[("body", group(&new, &body("theirs", None, &[])))]);
    let merged = merge(&local, &incoming);
    let contest = merged.contests.first().unwrap();
    assert!(contest.loser_is_local);
    assert_eq!(contest.loser.hlc, old);
    let merged = merge(&incoming, &local);
    assert!(!merged.contests.first().unwrap().loser_is_local);
}

#[test]
fn a_concurrent_pair_makes_a_revision_and_a_causal_ancestor_does_not() {
    let v1 = hlc(10, 0, A);
    let v2_a = hlc(20, 0, A);
    let v2_b = hlc(21, 0, B);
    let w = group(&v2_b, &body("b", Some(&v1), &[]));
    let p = group(&v2_a, &body("a", Some(&v1), &[]));
    assert!(
        needs_conflict_revision(BODY, &w, &p, false),
        "both edited v1"
    );
    let ancestor = group(&v1, &body("v1", None, &[]));
    assert!(
        !needs_conflict_revision(BODY, &w, &ancestor, false),
        "v1 == W.base: W was made from it"
    );
    let older = group(&hlc(5, 0, A), &body("v0", None, &[]));
    assert!(
        !needs_conflict_revision(BODY, &w, &older, false),
        "a stale version is below W.base"
    );
    assert!(
        needs_conflict_revision(BODY, &w, &older, true),
        "the device's own unconfirmed version always makes a revision, below the base too"
    );
    let same_text = group(&v2_a, &body("b", Some(&v1), &[]));
    assert!(
        !needs_conflict_revision(BODY, &w, &same_text, false),
        "equal text loses nothing"
    );
}

#[test]
fn the_base_of_the_winner_is_never_a_conflict_even_when_its_ack_was_lost() {
    // S4a counterexample (spikes/S4a-sync, seed 1, property 9): the device pushed v1, the response was lost, another device edited v1
    // into v2. On pull v1 is "the device's own unconfirmed version" and lost to v2, but v2 was made from it.
    let v1 = hlc(10, 0, A);
    let w = group(&hlc(20, 0, B), &body("b", Some(&v1), &[]));
    let p = group(&v1, &body("a", None, &[]));
    assert!(!needs_conflict_revision(BODY, &w, &p, true));
    assert!(!needs_conflict_revision(BODY, &w, &p, false));
}

#[test]
fn supersedes_hides_an_intermediate_version_even_for_the_own_unconfirmed_one() {
    let v1 = hlc(10, 0, A);
    let mid = hlc(20, 0, A);
    let last = hlc(30, 0, A);
    let w = group(&last, &body("c", Some(&v1), &[&mid]));
    let p = group(&mid, &body("b", Some(&v1), &[]));
    assert!(!needs_conflict_revision(BODY, &w, &p, false));
    assert!(!needs_conflict_revision(BODY, &w, &p, true));
}

#[test]
fn no_base_counts_as_minus_infinity() {
    let w = group(&hlc(20, 0, B), &body("b", None, &[]));
    let p = group(&hlc(10, 0, A), &body("a", None, &[]));
    assert!(needs_conflict_revision(BODY, &w, &p, false));
}

#[test]
fn a_version_from_the_future_makes_no_revision_on_this_client() {
    // ADR-0001 §3.4: "if it supports the `sv` of both versions (otherwise another device will create the revision)".
    let ok = group(&hlc(10, 0, A), &body("a", None, &[]));
    let future = |h: String| Group::new(&h, 2, &body("b", None, &[])).unwrap();
    let newer = future(hlc(20, 0, B));
    let older = future(hlc(5, 0, B));
    assert!(
        !needs_conflict_revision(BODY, &newer, &ok, true),
        "the winner is from the future"
    );
    assert!(
        !needs_conflict_revision(BODY, &ok, &older, true),
        "the loser is from the future"
    );
    assert!(!needs_conflict_revision(BODY, &newer, &older, true));
    assert_eq!(
        conflict_revision(MESSAGE, BODY, &older),
        Err(SyncError::UnsupportedVersion),
        "and the revision itself cannot be written"
    );
    let same_version = Group::new(&hlc(20, 0, B), 1, &body("b", None, &[])).unwrap();
    assert!(
        needs_conflict_revision(BODY, &same_version, &ok, true),
        "the same pair at a supported sv does"
    );
}

#[test]
fn flags_and_other_groups_never_make_revisions() {
    let w = group(&hlc(20, 0, B), r#"{"is_task":true}"#);
    let p = group(&hlc(10, 0, A), r#"{"is_task":false}"#);
    for name in ["flags", "deleted", "meta", "core", "unknown"] {
        assert!(!needs_conflict_revision(name, &w, &p, true), "{name}");
    }
}

#[test]
fn placement_compares_chat_and_day_not_the_service_fields() {
    let a = r#"{"base":null,"chat_id":"c1","journal_date":"2026-01-01","supersedes":[]}"#;
    let b = r#"{"base":"x","chat_id":"c1","journal_date":"2026-01-01","supersedes":["y"]}"#;
    let c = r#"{"base":null,"chat_id":"c2","journal_date":"2026-01-01","supersedes":[]}"#;
    assert!(!content_differs(PLACEMENT, a, b));
    assert!(content_differs(PLACEMENT, a, c));
}

#[test]
fn garbage_in_a_group_is_treated_as_a_difference_and_an_empty_lineage() {
    for garbage in ["", "null", "[1]", "{", "\u{0}", "{\"body_md\":5}"] {
        assert!(
            content_differs(BODY, garbage, &body("x", None, &[])),
            "{garbage:?}"
        );
        assert_eq!(Lineage::parse(garbage), Lineage::default(), "{garbage:?}");
    }
    let lineage = Lineage::parse(r#"{"base":5,"supersedes":"nope"}"#);
    assert_eq!(lineage, Lineage::default());
    let lineage = Lineage::parse(r#"{"base":"b","supersedes":["a",1,null,"c"]}"#);
    assert_eq!(lineage.base.as_deref(), Some("b"));
    assert_eq!(lineage.supersedes, ["a", "c"]);
}

#[test]
fn a_conflict_revision_is_identical_wherever_it_is_made() {
    let loser = group(
        &hlc(1_800_000_000_123, 3, A),
        &body("lost", Some(&hlc(5, 0, A)), &[]),
    );
    let one = conflict_revision(MESSAGE, BODY, &loser).unwrap();
    let two = conflict_revision(MESSAGE, BODY, &loser).unwrap();
    assert_eq!(one, two);
    assert_eq!(one.id_input.len(), 16 + 1 + 56);
    assert_eq!(one.id_input.get(16), Some(&1));
    assert_eq!(
        one.core.hlc, loser.hlc,
        "the revision's core carries hlc(P)"
    );
    let core: serde_json::Value = serde_json::from_str(&one.core.raw).unwrap();
    assert_eq!(core["reason"], "conflict");
    assert_eq!(
        core["saved_at"], 1_800_000_000_123u64,
        "saved_at is wall_ms of hlc(P)"
    );
    let snapshot: serde_json::Value =
        serde_json::from_str(core["snapshot_json"].as_str().unwrap()).unwrap();
    assert_eq!(
        snapshot["raw"], loser.raw,
        "raw is the exact group bytes, as a string"
    );
    assert_eq!(snapshot["g"], "body");
    let other = conflict_revision(MESSAGE, PLACEMENT, &loser).unwrap();
    assert_ne!(other.id_input, one.id_input, "the group is part of the id");
    assert_eq!(other.id_input.get(16), Some(&2));
}

#[test]
fn a_conflict_revision_refuses_garbage() {
    let loser = group(&hlc(1, 0, A), &body("x", None, &[]));
    assert!(conflict_revision("not-a-uuid", BODY, &loser).is_err());
    assert!(conflict_revision(MESSAGE, "flags", &loser).is_err());
    let bad = Group {
        hlc: "garbage".into(),
        sv: 1,
        raw: "{}".into(),
    };
    assert!(conflict_revision(MESSAGE, BODY, &bad).is_err());
}

#[test]
fn uuids_convert_to_bytes_and_back_with_the_v4_bits() {
    assert_eq!(uuid_bytes(MESSAGE).unwrap().first(), Some(&0x11));
    assert!(uuid_bytes("").is_none());
    assert!(uuid_bytes(&A.to_uppercase()).is_none());
    assert!(uuid_bytes(&MESSAGE.replace('1', "g")).is_none());
    let id = uuid_from_bytes([0xff; 16]);
    assert!(crate::hlc::is_uuid_v4(&id), "{id}");
    assert!(crate::hlc::is_uuid_v4(&uuid_from_bytes([0; 16])));
}
