//! Putting and removing reactions (slice 5a): the first put adds the group, later ones rewrite it, the caps are input rules, nothing
//! else changes.

use super::tests::{DEVICE, hlc, item};
use super::write::{ReactionError, apply, plan};
use super::*;
use crate::groups::{NewMessage, new_message};
use crate::rewrite::rewrite_group;
use serde_json::json;

pub(super) const AT: i64 = 1_700_000_000_000;
pub(super) const HEART: &str = "\u{2764}\u{FE0F}";

/// A new entry's maps (five groups under one HLC), and the maps after each write the test makes.
pub(super) struct Record {
    raw: String,
    hlcs: String,
    clock: u64,
}

impl Record {
    pub(super) fn new() -> Record {
        let input = NewMessage {
            id: "5b1c0d9e-3f2a-4a7b-9c8d-1e2f3a4b5c6d",
            chat_id: crate::groups::INBOX_ID,
            body_md: "Passed the exam!",
            created_at: AT,
            created_tz: "UTC",
            journal_date: "2026-11-18",
        };
        let row = new_message(&input, &hlc(1_000)).unwrap();
        Record {
            raw: row.groups_raw,
            hlcs: row.hlc_json,
            clock: 1_000,
        }
    }

    /// A new entry that already has the `reactions` group `group` (written by another device) under the HLC `at`.
    pub(super) fn with_group(group: &str, at: u64) -> Record {
        let mut record = Record::new();
        let mut groups: serde_json::Map<String, Value> = serde_json::from_str(&record.raw).unwrap();
        groups.insert(GROUP.into(), serde_json::from_str(group).unwrap());
        let mut hlcs: serde_json::Map<String, Value> = serde_json::from_str(&record.hlcs).unwrap();
        hlcs.insert(GROUP.into(), json!({"h": hlc(at), "sv": 1}));
        record.raw = serde_json::to_string(&groups).unwrap();
        record.hlcs = serde_json::to_string(&hlcs).unwrap();
        record.clock = at;
        record
    }

    pub(super) fn bytes(&self) -> (String, String) {
        (self.raw.clone(), self.hlcs.clone())
    }

    pub(super) fn groups(&self) -> Groups<'_> {
        Groups {
            groups_raw: &self.raw,
            hlc_json: &self.hlcs,
        }
    }

    /// Puts or removes; `Ok(true)` when something was written.
    pub(super) fn change(&mut self, emoji: &str, on: bool, at: i64) -> Result<bool, ReactionError> {
        let Some(planned) = plan(&self.groups(), emoji, on)? else {
            return Ok(false);
        };
        self.clock += 1_000;
        let done = apply(
            &self.groups(),
            &planned,
            (at, "Europe/Moscow"),
            &hlc(self.clock),
        )?;
        self.raw = done.groups_raw;
        self.hlcs = done.hlc_json;
        Ok(true)
    }

    pub(super) fn shown(&self) -> Shown {
        shown(&self.raw, &self.hlcs)
    }

    fn group_bytes(&self) -> String {
        crate::groups::parse_groups_raw(&self.raw).unwrap()[GROUP]
            .get()
            .to_string()
    }
}

#[test]
fn the_first_put_adds_the_group_and_leaves_the_other_groups_byte_identical() {
    let mut record = Record::new();
    let before = crate::groups::parse_groups_raw(&record.raw).unwrap();
    assert!(record.change(HEART, true, AT).unwrap());
    let after = crate::groups::parse_groups_raw(&record.raw).unwrap();
    for (name, bytes) in &before {
        assert_eq!(after[name].get(), bytes.get(), "group {name}");
    }
    assert_eq!(after.len(), before.len() + 1);
    let hlcs: serde_json::Map<String, Value> = serde_json::from_str(&record.hlcs).unwrap();
    assert_eq!(
        hlcs[GROUP],
        json!({"h": hlc(2_000), "sv": 1}),
        "above every HLC of the record"
    );
    assert_eq!(hlcs["body"], json!({"h": hlc(1_000), "sv": 1}));
}

/// The reference bytes of the first reaction (ADR-0001 appendix §C.9): the fixture and the Rust reader both pin them.
#[test]
fn the_reference_bytes_of_a_group() {
    let mut record = Record::new();
    record.change(HEART, true, AT).unwrap();
    assert_eq!(
        record.group_bytes(),
        format!(
            r#"{{"items":[{{"at":1700000000000,"e":"{HEART}","h":"0000000002000-00000-{DEVICE}","tz":"Europe/Moscow"}}]}}"#
        )
    );
    record.change("🔥", true, AT + 1_000).unwrap();
    record.change(HEART, false, AT + 2_000).unwrap();
    assert_eq!(
        record.group_bytes(),
        format!(
            r#"{{"items":[{{"at":null,"e":"{HEART}","h":"0000000004000-00000-{DEVICE}","tz":null}},{{"at":1700000001000,"e":"🔥","h":"0000000003000-00000-{DEVICE}","tz":"Europe/Moscow"}}]}}"#
        )
    );
}

#[test]
fn a_put_of_a_live_emoji_writes_nothing_and_keeps_its_time_and_a_removal_of_an_absent_one_too() {
    let mut record = Record::new();
    assert!(
        !record.change(HEART, false, AT).unwrap(),
        "nothing to remove"
    );
    assert!(record.change(HEART, true, AT).unwrap());
    let bytes = (record.raw.clone(), record.hlcs.clone());
    // Another form of the same emoji is the same reaction.
    assert!(!record.change("\u{2764}", true, AT + 99_000).unwrap());
    assert!(!record.change(HEART, true, AT + 99_000).unwrap());
    assert_eq!((record.raw.clone(), record.hlcs.clone()), bytes);
    assert_eq!(record.shown().reactions[0].put_at, AT);
}

#[test]
fn a_removal_leaves_a_marker_and_a_second_put_gets_a_new_time() {
    let mut record = Record::new();
    record.change(HEART, true, AT).unwrap();
    assert!(
        record.change("\u{2764}", false, AT + 1_000).unwrap(),
        "removed by another form of the emoji"
    );
    let got = record.shown();
    assert!(got.reactions.is_empty());
    assert_eq!(got.keys.len(), 1, "the marker stays");
    assert!(
        !record.change(HEART, false, AT).unwrap(),
        "removing twice writes nothing"
    );
    assert!(record.change(HEART, true, AT + 5_000).unwrap());
    let got = record.shown();
    assert_eq!(got.reactions[0].put_at, AT + 5_000);
    assert_eq!(got.reactions[0].emoji, HEART);
    assert_eq!(got.keys.len(), 1);
}

#[test]
fn a_write_changes_one_item_and_every_other_item_stays_byte_identical() {
    let mut record = Record::new();
    record.change("😂", true, AT).unwrap();
    record.change("🔥", true, AT + 1).unwrap();
    record.change("😢", true, AT + 2).unwrap();
    let items = |record: &Record| -> Vec<(String, String)> {
        let value: Value = serde_json::from_str(&record.group_bytes()).unwrap();
        value["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| {
                (
                    item["e"].as_str().unwrap().to_string(),
                    jcs::canonical_string(item).unwrap(),
                )
            })
            .collect()
    };
    let before = items(&record);
    record.change("🔥", false, AT + 3).unwrap();
    let after = items(&record);
    assert_eq!(before.len(), after.len());
    for (old, new) in before.iter().zip(&after) {
        if old.0 == "🔥" {
            assert_ne!(old.1, new.1);
        } else {
            assert_eq!(old, new, "an untouched item is byte-identical");
        }
    }
}

#[test]
fn unknown_fields_of_the_group_and_of_the_items_survive_a_write() {
    let mut extra = item("😂", Some(AT), Some("UTC"), 1_500);
    extra["sticker"] = json!({"id": 7});
    let group = jcs::canonical_string(&json!({"items": [extra.clone()], "zz": true})).unwrap();
    let mut record = Record::with_group(&group, 1_500);
    record.change("🔥", true, AT + 1).unwrap();
    let value: Value = serde_json::from_str(&record.group_bytes()).unwrap();
    assert_eq!(value["zz"], json!(true));
    assert!(
        value["items"].as_array().unwrap().contains(&extra),
        "the item with an unknown field is unchanged"
    );
}

#[test]
fn input_that_is_not_one_emoji_is_refused_before_anything_is_read() {
    let record = Record::new();
    let long = "😂".repeat(100);
    let big = "x".repeat(1 << 20);
    for text in [
        "",
        " ",
        "a",
        "ab",
        "😂😂",
        "😂 ",
        "\u{FE0F}",
        "\u{200D}",
        "😂\u{200D}",
        "\u{7}",
        "<b>",
        &long,
        &big,
    ] {
        for on in [true, false] {
            assert_eq!(
                plan(&record.groups(), text, on).unwrap_err(),
                ReactionError::Invalid,
                "{text:.10?}"
            );
        }
    }
}

#[test]
fn a_group_that_is_not_well_formed_refuses_a_write_as_corrupt() {
    let record = Record::with_group(r#"{"items":"no"}"#, 1_500);
    for on in [true, false] {
        assert!(matches!(
            plan(&record.groups(), HEART, on),
            Err(ReactionError::Group(GroupError::Corrupt(_)))
        ));
    }
}

/// 7b's whole-record refusal covers the add path: a message whose two maps name different groups is `Corrupt`, whatever group is added.
#[test]
fn a_record_whose_maps_name_different_groups_is_corrupt_on_the_add_path() {
    let record = Record::new();
    let mut hlcs: serde_json::Map<String, Value> = serde_json::from_str(&record.hlcs).unwrap();
    hlcs.remove("flags");
    let hlcs = serde_json::to_string(&hlcs).unwrap();
    let broken = Groups {
        groups_raw: &record.raw,
        hlc_json: &hlcs,
    };
    let planned = plan(&broken, HEART, true).unwrap().unwrap();
    assert!(matches!(
        apply(&broken, &planned, (AT, "UTC"), &hlc(9_000)),
        Err(GroupError::Corrupt(_))
    ));
    // The other way round: a group in the bytes with no HLC entry.
    let mut groups: serde_json::Map<String, Value> = serde_json::from_str(&record.raw).unwrap();
    groups.insert("zz_future".into(), json!({}));
    let raw = serde_json::to_string(&groups).unwrap();
    let broken = Groups {
        groups_raw: &raw,
        hlc_json: &record.hlcs,
    };
    let planned = plan(&broken, HEART, true).unwrap().unwrap();
    assert!(matches!(
        apply(&broken, &planned, (AT, "UTC"), &hlc(9_000)),
        Err(GroupError::Corrupt(_))
    ));
}

#[test]
fn an_hlc_that_is_not_above_the_record_is_refused() {
    let mut record = Record::new();
    record.change(HEART, true, AT).unwrap();
    let planned = plan(&record.groups(), "🔥", true).unwrap().unwrap();
    for stale in [1_000, 2_000] {
        assert_eq!(
            apply(&record.groups(), &planned, (AT, "UTC"), &hlc(stale)).unwrap_err(),
            GroupError::HlcNotNewer
        );
    }
}

#[test]
fn an_older_client_rewriting_flags_keeps_the_reactions_bytes_verbatim() {
    let mut record = Record::new();
    record.change(HEART, true, AT).unwrap();
    let before = record.group_bytes();
    let rewritten = rewrite_group(&record.groups(), "flags", &hlc(7_000), None, |flags, _| {
        flags.insert("favorite_at".into(), json!(AT));
        Ok(())
    })
    .unwrap();
    let after = crate::groups::parse_groups_raw(&rewritten.groups_raw).unwrap();
    assert_eq!(after[GROUP].get(), before);
    assert!(
        rewritten
            .hlc_json
            .contains(&format!(r#""reactions":{{"h":"{}","sv":1}}"#, hlc(2_000)))
    );
}

#[test]
fn the_time_and_the_zone_of_a_put_are_sanitized() {
    let record = Record::new();
    let planned = plan(&record.groups(), HEART, true).unwrap().unwrap();
    let done = apply(
        &record.groups(),
        &planned,
        (i64::MAX, &"z".repeat(100)),
        &hlc(2_000),
    )
    .unwrap();
    let got = shown(&done.groups_raw, &done.hlc_json);
    assert_eq!(got.reactions[0].put_at, MAX_AT);
    assert_eq!(got.reactions[0].put_tz, "UTC");
    let done = apply(&record.groups(), &planned, (-5, "Asia/Tokyo"), &hlc(2_000)).unwrap();
    assert_eq!(
        shown(&done.groups_raw, &done.hlc_json).reactions[0].put_at,
        0
    );
}
