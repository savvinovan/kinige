//! The add-group function (slice 7b): absent, half-present, not allowed, byte identity of everything else, and the convergence of
//! details edits from two devices on a row that has none of the groups (every merge order).

use super::*;
use crate::attachment::{
    AttachmentKind, Details, NewAttachment, description_group, name_group, new_attachment,
    privacy_group, read_details,
};
use crate::rewrite::rewrite_field;
use crate::sync::{Versioned, merge};
use serde_json::{Map, json};

const ID: &str = "9d3e4f50-6a7b-4c8d-9e0f-1a2b3c4d5e6f";
const MESSAGE: &str = "5b1c0d9e-3f2a-4a7b-9c8d-1e2f3a4b5c6d";
const BLOB: &str = "01934f2e-8a7b-7c3d-9e1f-2a3b4c5d6e7f";
const KEY: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
const SHA: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

fn hlc(l: u64) -> String {
    format!("{l:013}-00000-5f0c3e2a-9b1d-4c7e-8a64-0d2f1b3c4e5f")
}

/// A row written before 7b: `core`, `order`, `deleted` only, with an extra field in one `hlc_json` entry and an unknown group.
fn legacy() -> (String, String) {
    let input = NewAttachment {
        id: ID,
        message_id: MESSAGE,
        kind: AttachmentKind::Image,
        sort_index: 0,
        sha256: SHA,
        filename: "IMG_0001.jpg",
        mime: "image/jpeg",
        byte_size: 1,
        width: Some(2),
        height: Some(2),
        duration_ms: None,
        remote_blob_id: BLOB,
        blob_key: KEY,
        display_name: None,
        description: None,
        spoiler: false,
    };
    let row = new_attachment(&input, &hlc(1_000)).unwrap();
    let mut groups: Map<String, serde_json::Value> = serde_json::from_str(&row.groups_raw).unwrap();
    let mut hlcs: Map<String, serde_json::Value> = serde_json::from_str(&row.hlc_json).unwrap();
    for group in ["name", "description", "privacy"] {
        groups.remove(group);
        hlcs.remove(group);
    }
    groups.insert("zz_future".into(), json!({"a": 1}));
    hlcs.insert(
        "zz_future".into(),
        json!({"h": hlc(1_000), "sv": 1, "extra": [1, 2]}),
    );
    (
        serde_json::to_string(&groups).unwrap(),
        serde_json::to_string(&hlcs).unwrap(),
    )
}

fn groups_of(raw: &(String, String)) -> Groups<'_> {
    Groups {
        groups_raw: &raw.0,
        hlc_json: &raw.1,
    }
}

#[test]
fn an_absent_group_is_added_and_everything_else_stays_byte_identical() {
    let old = legacy();
    let added = add_group(
        &groups_of(&old),
        "attachment",
        "privacy",
        &privacy_group(true),
        &hlc(2_000),
        None,
    )
    .unwrap();
    let before = crate::groups::parse_groups_raw(&old.0).unwrap();
    let after = crate::groups::parse_groups_raw(&added.groups_raw).unwrap();
    assert_eq!(after["privacy"].get(), r#"{"spoiler":true}"#);
    for (name, bytes) in &before {
        assert_eq!(after[name].get(), bytes.get(), "group {name}");
    }
    assert_eq!(after.len(), before.len() + 1);
    // The `hlc_json` entries of the other groups, extra fields included, are untouched; the new one is `{h, sv}`.
    let hlcs_before: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(&old.1).unwrap();
    let hlcs_after: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(&added.hlc_json).unwrap();
    for (name, entry) in &hlcs_before {
        assert_eq!(&hlcs_after[name], entry, "hlc of {name}");
    }
    assert_eq!(hlcs_after["privacy"], json!({"h": hlc(2_000), "sv": 1}));
    assert!(
        added.hlc_json.contains(r#""extra":[1,2]"#),
        "{}",
        added.hlc_json
    );
    assert_eq!(added.schema_version, 1);
}

#[test]
fn the_new_hlc_must_be_above_every_hlc_of_the_record() {
    let old = legacy();
    // Equal to, or below, the newest HLC of any group (here the unknown one) is refused; nothing is written.
    for l in [1_000, 999, 1] {
        let result = add_group(
            &groups_of(&old),
            "attachment",
            "name",
            &name_group(Some("a.jpg")),
            &hlc(l),
            None,
        );
        assert_eq!(result, Err(GroupError::HlcNotNewer), "{l}");
    }
    let garbage = add_group(
        &groups_of(&old),
        "attachment",
        "name",
        &name_group(None),
        "x",
        None,
    );
    assert!(matches!(garbage, Err(GroupError::Corrupt(_))));
}

#[test]
fn a_group_in_only_one_of_the_two_maps_is_corrupt() {
    let (groups, hlcs) = legacy();
    let mut only_bytes: Map<String, serde_json::Value> = serde_json::from_str(&groups).unwrap();
    only_bytes.insert("privacy".into(), json!({"spoiler": false}));
    let mut only_hlc: Map<String, serde_json::Value> = serde_json::from_str(&hlcs).unwrap();
    only_hlc.insert("privacy".into(), json!({"h": hlc(1_000), "sv": 1}));
    let cases = [
        (serde_json::to_string(&only_bytes).unwrap(), hlcs.clone()),
        (groups.clone(), serde_json::to_string(&only_hlc).unwrap()),
    ];
    for case in &cases {
        let result = add_group(
            &groups_of(case),
            "attachment",
            "privacy",
            &privacy_group(true),
            &hlc(9_000),
            None,
        );
        assert!(matches!(result, Err(GroupError::Corrupt(_))), "{result:?}");
    }
    // A group that is there in both is not added again: the caller rewrites it.
    let both = (
        serde_json::to_string(&only_bytes).unwrap(),
        serde_json::to_string(&only_hlc).unwrap(),
    );
    let result = add_group(
        &groups_of(&both),
        "attachment",
        "privacy",
        &privacy_group(true),
        &hlc(9_000),
        None,
    );
    assert_eq!(
        result,
        Err(GroupError::Invalid("the group is already there"))
    );
}

#[test]
fn only_the_allowlisted_groups_of_the_entity_may_be_added() {
    let old = legacy();
    for (entity, group) in [
        ("attachment", "core"),
        ("attachment", "order"),
        ("attachment", "deleted"),
        ("attachment", "zz_future"),
        ("message", "body"),
        ("message", "name"),
        ("chat", "privacy"),
        ("", "name"),
    ] {
        let result = add_group(
            &groups_of(&old),
            entity,
            group,
            &json!({}),
            &hlc(9_000),
            None,
        );
        assert_eq!(
            result,
            Err(GroupError::Invalid("this group may not be added")),
            "{entity} {group}"
        );
    }
    // The limit of the group's bytes is checked too.
    let result = add_group(
        &groups_of(&old),
        "attachment",
        "description",
        &description_group(Some(&"a".repeat(100))),
        &hlc(9_000),
        Some(50),
    );
    assert_eq!(result, Err(GroupError::TooLarge));
}

// ---- convergence ----

#[derive(Clone, Copy, Debug)]
enum Op {
    Name(&'static str),
    Describe(&'static str),
    Hide,
    Show,
}

const OPS: [Op; 5] = [
    Op::Name("a.jpg"),
    Op::Name("b.jpg"),
    Op::Describe("x"),
    Op::Hide,
    Op::Show,
];

fn field(op: Op) -> (&'static str, serde_json::Value) {
    match op {
        Op::Name(name) => ("name", name_group(Some(name))),
        Op::Describe(text) => ("description", description_group(Some(text))),
        Op::Hide => ("privacy", privacy_group(true)),
        Op::Show => ("privacy", privacy_group(false)),
    }
}

/// One device's edit, as `set_attachment_details` does it: rewrite the group when it is there, add it when it is not.
fn apply(row: &(String, String), op: Op, at: u64) -> (String, String) {
    let (group, value) = field(op);
    let present = serde_json::from_str::<Map<String, serde_json::Value>>(&row.1)
        .unwrap()
        .contains_key(group);
    let done = if present {
        let (key, field_value) = value.as_object().unwrap().iter().next().unwrap();
        rewrite_field(
            &groups_of(row),
            group,
            &hlc(at),
            None,
            (key, field_value.clone()),
        )
    } else {
        add_group(&groups_of(row), "attachment", group, &value, &hlc(at), None)
    }
    .unwrap();
    (done.groups_raw, done.hlc_json)
}

fn details(row: &(String, String)) -> Details {
    read_details(AttachmentKind::Image, "IMG_0001.jpg", &row.0, &row.1)
}

fn sequences() -> Vec<Vec<Op>> {
    let mut all = vec![Vec::new()];
    for first in OPS {
        all.push(vec![first]);
        for second in OPS {
            all.push(vec![first, second]);
        }
    }
    all
}

/// What the last writer of each field leaves: the op with the highest HLC per group, over both devices.
fn expected(edits: &[(Op, u64)]) -> Details {
    let last = |group: &str| {
        edits
            .iter()
            .filter(|(op, _)| field(*op).0 == group)
            .max_by_key(|(_, at)| *at)
            .map(|(op, _)| *op)
    };
    Details {
        display_name: match last("name") {
            Some(Op::Name(name)) => Some(name.to_string()),
            _ => None,
        },
        description: match last("description") {
            Some(Op::Describe(text)) => Some(text.to_string()),
            _ => None,
        },
        spoiler: matches!(last("privacy"), Some(Op::Hide)),
    }
}

#[test]
fn details_edits_from_two_devices_converge_in_every_merge_order() {
    let base = legacy();
    for (a_clock, b_clock) in [(1_000u64, 2_000u64), (2_000, 1_000)] {
        for a_ops in sequences() {
            for b_ops in sequences() {
                let (mut a, mut b) = (base.clone(), base.clone());
                let mut edits = Vec::new();
                for (i, op) in a_ops.iter().enumerate() {
                    let at = 10_000 + a_clock * (i as u64 + 1) * 2;
                    a = apply(&a, *op, at);
                    edits.push((*op, at));
                }
                for (i, op) in b_ops.iter().enumerate() {
                    let at = 10_000 + b_clock * (i as u64 + 1) * 2 + 1;
                    b = apply(&b, *op, at);
                    edits.push((*op, at));
                }
                let (va, vb) = (
                    Versioned::from_columns(&a.0, &a.1).unwrap(),
                    Versioned::from_columns(&b.0, &b.1).unwrap(),
                );
                let ab = merge(&va, &vb).record;
                let ba = merge(&vb, &va).record;
                assert_eq!(ab, ba, "{a_ops:?} against {b_ops:?}");
                let merged = ab.to_columns().unwrap();
                assert_eq!(
                    details(&merged),
                    expected(&edits),
                    "{a_ops:?} against {b_ops:?}: a rename keeps the spoiler, a hide keeps the name and the text"
                );
                // The unknown group survives every merge.
                assert!(merged.0.contains("zz_future"));
            }
        }
    }
}
