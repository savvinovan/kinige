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
fn a_record_whose_two_maps_name_different_groups_takes_no_group_at_all() {
    // `privacy` is only in `groups_raw`: adding `name` (a group that is absent from both) must not hide that behind a new group.
    let (groups, hlcs) = legacy();
    let mut only_bytes: Map<String, serde_json::Value> = serde_json::from_str(&groups).unwrap();
    only_bytes.insert("privacy".into(), json!({"spoiler": false}));
    let mut only_hlc: Map<String, serde_json::Value> = serde_json::from_str(&hlcs).unwrap();
    only_hlc.insert("zz_other".into(), json!({"h": hlc(1_000), "sv": 1}));
    let cases = [
        (serde_json::to_string(&only_bytes).unwrap(), hlcs.clone()),
        (groups.clone(), serde_json::to_string(&only_hlc).unwrap()),
    ];
    for case in &cases {
        for (group, value) in [
            ("name", name_group(Some("a.jpg"))),
            ("description", description_group(Some("x"))),
        ] {
            let result = add_group(
                &groups_of(case),
                "attachment",
                group,
                &value,
                &hlc(9_000),
                None,
            );
            assert!(
                matches!(result, Err(GroupError::Corrupt(_))),
                "{group}: {result:?}"
            );
        }
    }
}

#[test]
fn rewrite_field_keeps_the_unknown_fields_of_a_group_and_takes_only_the_details_groups() {
    let mut row = legacy();
    let mut groups: Map<String, serde_json::Value> = serde_json::from_str(&row.0).unwrap();
    let mut hlcs: Map<String, serde_json::Value> = serde_json::from_str(&row.1).unwrap();
    groups.insert(
        "privacy".into(),
        json!({"future": {"a": [1, 2]}, "spoiler": true}),
    );
    hlcs.insert("privacy".into(), json!({"h": hlc(1_000), "sv": 1}));
    row = (
        serde_json::to_string(&groups).unwrap(),
        serde_json::to_string(&hlcs).unwrap(),
    );
    let done = rewrite_field(
        &groups_of(&row),
        "privacy",
        &hlc(9_000),
        None,
        ("spoiler", json!(false)),
    )
    .unwrap();
    let after: Map<String, serde_json::Value> = serde_json::from_str(&done.groups_raw).unwrap();
    assert_eq!(
        after["privacy"].to_string(),
        r#"{"future":{"a":[1,2]},"spoiler":false}"#
    );
    // Any other group is refused, a corrupt `core` included: it is not replaced by an object.
    for group in ["core", "body", "order", "deleted", "zz_future"] {
        let result = rewrite_field(
            &groups_of(&row),
            group,
            &hlc(9_000),
            None,
            ("spoiler", json!(false)),
        );
        assert_eq!(
            result,
            Err(GroupError::Invalid(
                "this group may not be rewritten field by field"
            )),
            "{group}"
        );
    }
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

/// The allowlist decides for the rewrite paths as well as for adding: a group that is on the record but may not be added is not edited by
/// `rewrite_or_add_*` either (removing the check left every other test green).
#[test]
fn the_allowlist_decides_on_the_rewrite_paths_too() {
    let old = legacy();
    for (entity, group) in [
        ("message", "core"),
        ("attachment", "core"),
        ("attachment", "order"),
        ("attachment", "zz_future"),
    ] {
        let by_group =
            rewrite_or_add_group(&groups_of(&old), entity, group, &hlc(9_000), None, |_| {
                Ok(())
            });
        assert_eq!(
            by_group,
            Err(GroupError::Invalid("this group may not be added")),
            "group: {entity} {group}"
        );
        let by_field = rewrite_or_add_field(
            &groups_of(&old),
            entity,
            group,
            &hlc(9_000),
            None,
            ("x", json!(1)),
        );
        assert_eq!(
            by_field,
            Err(GroupError::Invalid("this group may not be added")),
            "field: {entity} {group}"
        );
    }
    // The group of the allowlist that the record has is rewritten.
    let with_name = add_group(
        &groups_of(&old),
        "attachment",
        "name",
        &name_group(Some("a.jpg")),
        &hlc(2_000),
        None,
    )
    .unwrap();
    let with_name = (with_name.groups_raw, with_name.hlc_json);
    let again = rewrite_or_add_field(
        &groups_of(&with_name),
        "attachment",
        "name",
        &hlc(3_000),
        None,
        ("name", json!("b.jpg")),
    );
    assert!(again.is_ok(), "{again:?}");
}

mod convergence;
