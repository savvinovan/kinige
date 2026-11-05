use super::conflict::BODY;
use super::*;
use crate::hlc::Clock;

const A: &str = "5f0c3e2a-9b1d-4c7e-8a64-0d2f1b3c4e5f";
const B: &str = "7a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d";
const C: &str = "11111111-2222-4333-8444-555555555555";

fn hlc(l: u64, device: &str) -> String {
    Clock { l, c: 0 }.format(device)
}

fn body_vv(text: &str, base: Option<&str>, vv: Option<&[String]>) -> String {
    let mut value =
        serde_json::json!({"base": base, "body_md": text, "edited_at": null, "supersedes": []});
    if let Some(vv) = vv {
        value["vv"] = serde_json::json!(vv);
    }
    value.to_string()
}

fn group(h: &str, raw: &str) -> Group {
    Group::new(h, 1, raw).unwrap()
}

#[test]
fn a_version_the_author_never_saw_is_concurrent_however_low_its_hlc() {
    // S4a counterexample: B wrote p (hlc 3), acked, and went away. A, who had not seen it, wrote v (10) and then w (20) from v. The
    // server compacted v. With only `base` p is below w.base (10) and looks like an old ancestor: lost without a trace.
    let p = group(&hlc(3, B), &body_vv("p", Some(&hlc(1, A)), None));
    let v = hlc(10, A);
    let w_hlc = hlc(20, A);
    let old_w = group(&w_hlc, &body_vv("w", Some(&v), None));
    assert!(
        !needs_conflict_revision(BODY, &old_w, &p, false),
        "the old rule cannot tell: this is the hole"
    );

    let vv = next_vv(
        Some(&group(
            &v,
            &body_vv("v", Some(&hlc(1, A)), Some(&[hlc(1, A), v.clone()])),
        )),
        &w_hlc,
    );
    assert_eq!(
        vv,
        std::slice::from_ref(&w_hlc),
        "A's versions only: B is not in the history"
    );
    let new_w = group(&w_hlc, &body_vv("w", Some(&v), Some(&vv)));
    assert!(needs_conflict_revision(BODY, &new_w, &p, false));
    assert!(needs_conflict_revision(BODY, &new_w, &p, true));
}

#[test]
fn a_version_in_the_history_is_not_concurrent() {
    let p = hlc(3, B);
    let w_hlc = hlc(20, A);
    let vv = vec![hlc(5, B), w_hlc.clone()];
    let w = group(&w_hlc, &body_vv("w", Some(&hlc(10, A)), Some(&vv)));
    let ancestor = group(&p, &body_vv("p", None, None));
    assert!(
        !needs_conflict_revision(BODY, &w, &ancestor, false),
        "B's newer version 5 is in the history, so 3 is too"
    );
    assert!(
        !needs_conflict_revision(BODY, &w, &ancestor, true),
        "also for the device's own unconfirmed version"
    );
    let newer = group(&hlc(6, B), &body_vv("q", None, None));
    assert!(
        needs_conflict_revision(BODY, &w, &newer, false),
        "6 is above everything of B's that A saw"
    );
    let stranger = group(&hlc(1, C), &body_vv("s", None, None));
    assert!(
        needs_conflict_revision(BODY, &w, &stranger, false),
        "a device that is not in the history at all"
    );
}

#[test]
fn two_versions_of_one_device_fall_back_to_the_base_rule() {
    // A profile clone (ADR-0001 §1): the same device id wrote both, so `vv` cannot tell them apart.
    let a1 = group(&hlc(10, A), &body_vv("a1", Some(&hlc(1, A)), None));
    let b1_hlc = hlc(20, A);
    let b1 = group(
        &b1_hlc,
        &body_vv("b1", Some(&hlc(5, A)), Some(std::slice::from_ref(&b1_hlc))),
    );
    assert!(
        needs_conflict_revision(BODY, &b1, &a1, false),
        "a1 is above b1.base: concurrent, also under vv"
    );
    let own_old = group(&hlc(2, A), &body_vv("old", None, None));
    assert!(
        !needs_conflict_revision(BODY, &b1, &own_old, false),
        "below the base: an old ancestor"
    );
    assert!(
        needs_conflict_revision(BODY, &b1, &own_old, true),
        "unless it is the device's own unconfirmed version"
    );
}

#[test]
fn a_garbage_vv_is_not_trusted() {
    let w_hlc = hlc(20, A);
    let p = group(&hlc(3, B), &body_vv("p", None, None));
    for vv in [
        r#""x""#,
        r#"[1,2]"#,
        r#"{"a":1}"#,
        "null",
        r#"["not an hlc"]"#,
    ] {
        let raw = format!(r#"{{"base":null,"body_md":"w","vv":{vv}}}"#);
        let w = group(&w_hlc, &raw);
        assert!(
            needs_conflict_revision(BODY, &w, &p, false),
            "{vv}: an unusable vv leaves the loser concurrent"
        );
    }
    assert_eq!(Lineage::parse(r#"{"vv":["a",1]}"#).vv, None);
    assert_eq!(
        Lineage::parse(r#"{"vv":["a","b"]}"#).vv,
        Some(vec!["a".to_string(), "b".to_string()])
    );
}

#[test]
fn next_vv_keeps_the_newest_stamp_per_device() {
    let a1 = hlc(10, A);
    let b1 = hlc(12, B);
    let a2 = hlc(30, A);
    let parent = group(
        &b1,
        &body_vv("x", Some(&a1), Some(&[a1.clone(), b1.clone()])),
    );
    assert_eq!(
        next_vv(Some(&parent), &a2),
        [b1.clone(), a2.clone()]
            .iter()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
    );
    assert_eq!(
        next_vv(None, &a1),
        std::slice::from_ref(&a1),
        "the first version knows only itself"
    );
}

#[test]
fn next_vv_of_an_old_format_parent_is_rebuilt_from_its_bytes() {
    let a1 = hlc(10, A);
    let b1 = hlc(12, B);
    // base b1 (B's), the parent itself is A's a1.
    let parent = group(&a1, &body_vv("x", Some(&b1), None));
    let new = hlc(40, C);
    let vv = next_vv(Some(&parent), &new);
    assert_eq!(
        vv,
        [a1, b1, new]
            .iter()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
    );
}

#[test]
fn next_vv_survives_garbage_and_is_capped() {
    let parent = Group {
        hlc: "garbage".to_string(),
        sv: 1,
        raw: "not json".to_string(),
    };
    assert_eq!(next_vv(Some(&parent), &hlc(1, A)), [hlc(1, A)]);
    let devices: Vec<String> = (0..40u8)
        .map(|i| crate::sync::conflict::uuid_from_bytes([i; 16]))
        .collect();
    let stamps: Vec<String> = devices
        .iter()
        .enumerate()
        .map(|(i, d)| hlc(100 + i as u64, d))
        .collect();
    let parent = group(stamps.first().unwrap(), &body_vv("x", None, Some(&stamps)));
    let newest = hlc(10_000, A);
    let vv = next_vv(Some(&parent), &newest);
    assert_eq!(vv.len(), MAX_VV);
    assert!(vv.contains(&newest), "the new version always stays");
}

#[test]
fn with_vv_keeps_the_rest_of_the_group_canonical() {
    let raw =
        r#"{"base":null,"body_md":"hi","edited_at":null,"future_field":[1,2],"supersedes":[]}"#;
    let out = with_vv(raw, &[hlc(1, A)]).unwrap();
    assert!(out.contains(r#""future_field":[1,2]"#));
    assert!(
        out.ends_with(&format!(r#""vv":["{}"]}}"#, hlc(1, A))),
        "{out}"
    );
    assert_eq!(Lineage::parse(&out).vv, Some(vec![hlc(1, A)]));
    assert!(with_vv("[1]", &[]).is_err());
    assert!(with_vv("", &[]).is_err());
}
