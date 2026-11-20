//! The `sticker` group (appendix §C.10): the reference bytes, the reader's table case by case, arbitrary input, and the display rule.

use super::sticker::{MAX_GROUP_BYTES, clean_emoji};
use super::*;
use crate::groups::parse_groups_raw;
use serde_json::{Value, json};

pub(super) const ID: &str = "11111111-2222-4333-8444-555555555555";
const MESSAGE: &str = "66666666-7777-4888-9999-aaaaaaaaaaaa";
const SHA: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const KEY: &str = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";
const BLOB: &str = "01939a5e-4c00-7000-8000-0123456789ab";
const HLC: &str = "1793000000000-00000-5f0c3e2a-9b1d-4c7e-8a64-0d2f1b3c4e5f";

fn sticker_input(emoji: Option<&'static str>) -> NewAttachment<'static> {
    NewAttachment {
        id: ID,
        message_id: MESSAGE,
        kind: AttachmentKind::Image,
        sort_index: 0,
        sha256: SHA,
        filename: "sticker.webp",
        mime: "image/webp",
        byte_size: 20_000,
        width: Some(512),
        height: Some(384),
        duration_ms: None,
        remote_blob_id: BLOB,
        blob_key: KEY,
        display_name: None,
        description: None,
        spoiler: false,
        sticker: true,
        sticker_emoji: emoji,
    }
}

fn hlc() -> String {
    HLC.to_string()
}

const CORE: StickerCore<'static> = StickerCore {
    kind: AttachmentKind::Image,
    mime: "image/webp",
    width: Some(512),
    height: Some(384),
    byte_size: 20_000,
};

/// The two maps of a record whose `sticker` group is `raw` under `hlc_entry`; `None` leaves it out of that map.
fn maps(raw: Option<&str>, hlc_entry: Option<Value>) -> (String, String) {
    let row = new_attachment(&sticker_input(None), &hlc()).unwrap();
    let mut groups: serde_json::Map<String, Value> = serde_json::from_str(&row.groups_raw).unwrap();
    let mut hlcs: serde_json::Map<String, Value> = serde_json::from_str(&row.hlc_json).unwrap();
    groups.remove("sticker");
    hlcs.remove("sticker");
    let mut text = serde_json::to_string(&groups).unwrap();
    if let Some(raw) = raw {
        // The group's own bytes verbatim, so a non-canonical form can be written.
        text.pop();
        text.push_str(&format!(r#","sticker":{raw}}}"#));
    }
    if let Some(entry) = hlc_entry {
        hlcs.insert("sticker".into(), entry);
    }
    (text, serde_json::to_string(&hlcs).unwrap())
}

pub(super) fn read_raw(raw: &str) -> Sticker {
    let (groups, hlcs) = maps(Some(raw), Some(json!({"h": hlc(), "sv": 1})));
    read_sticker(&CORE, &groups, &hlcs)
}

fn picture() -> Sticker {
    Sticker::default()
}

fn sticker(emoji: Option<&str>) -> Sticker {
    Sticker {
        sticker: true,
        emoji: emoji.map(str::to_string),
    }
}

#[test]
fn the_reference_bytes_of_appendix_c10() {
    let row = new_attachment(&sticker_input(Some("😺")), &hlc()).unwrap();
    let groups = parse_groups_raw(&row.groups_raw).unwrap();
    assert_eq!(groups["sticker"].get(), r#"{"emoji":"😺","sticker":true}"#);
    let hlcs: Value = serde_json::from_str(&row.hlc_json).unwrap();
    assert_eq!(hlcs["sticker"], json!({"h": hlc(), "sv": 1}));
    assert!(row.sticker);
    assert_eq!(row.sticker_emoji.as_deref(), Some("😺"));
    assert_eq!(row.schema_version, 1);
    // A sticker without an emoji and a picture.
    let bare = new_attachment(&sticker_input(None), &hlc()).unwrap();
    assert_eq!(
        parse_groups_raw(&bare.groups_raw).unwrap()["sticker"].get(),
        r#"{"emoji":null,"sticker":true}"#
    );
}

#[test]
fn a_sticker_and_a_photo_name_the_same_groups() {
    let sticker_row = new_attachment(&sticker_input(Some("😺")), &hlc()).unwrap();
    let photo = NewAttachment {
        sticker: false,
        sticker_emoji: None,
        ..sticker_input(None)
    };
    let photo_row = new_attachment(&photo, &hlc()).unwrap();
    let names = |groups_raw: &str, hlc_json: &str| {
        let groups: Vec<String> = parse_groups_raw(groups_raw).unwrap().into_keys().collect();
        let hlcs: Vec<String> =
            serde_json::from_str::<std::collections::BTreeMap<String, Value>>(hlc_json)
                .unwrap()
                .into_keys()
                .collect();
        (groups, hlcs)
    };
    assert_eq!(
        names(&sticker_row.groups_raw, &sticker_row.hlc_json),
        names(&photo_row.groups_raw, &photo_row.hlc_json)
    );
    assert!(!photo_row.sticker);
    assert_eq!(photo_row.sticker_emoji, None);
}

#[test]
fn the_reader_table_case_by_case() {
    // Absent from both maps: every row written before 5c.
    let (groups, hlcs) = maps(None, None);
    assert_eq!(read_sticker(&CORE, &groups, &hlcs), picture());
    // `false`.
    assert_eq!(read_raw(r#"{"emoji":null,"sticker":false}"#), picture());
    // `true` with a valid emoji, and without one in every way.
    assert_eq!(
        read_raw(r#"{"emoji":"😺","sticker":true}"#),
        sticker(Some("😺"))
    );
    assert_eq!(read_raw(r#"{"emoji":null,"sticker":true}"#), sticker(None));
    assert_eq!(read_raw(r#"{"sticker":true}"#), sticker(None));
    assert_eq!(read_raw(r#"{"emoji":7,"sticker":true}"#), sticker(None));
    assert_eq!(
        read_raw(r#"{"emoji":"hello","sticker":true}"#),
        sticker(None)
    );
    assert_eq!(
        read_raw(r#"{"emoji":"<b>x</b>","sticker":true}"#),
        sticker(None)
    );
    // `sticker` absent or not a boolean; the group is not an object.
    assert_eq!(read_raw(r#"{"emoji":"😺"}"#), picture());
    assert_eq!(read_raw(r#"{"emoji":"😺","sticker":"true"}"#), picture());
    assert_eq!(read_raw(r#"{"emoji":"😺","sticker":1}"#), picture());
    assert_eq!(read_raw(r#"{"emoji":"😺","sticker":null}"#), picture());
    for raw in ["[]", "true", "null", "7", r#""x""#, "[{\"sticker\":true}]"] {
        assert_eq!(read_raw(raw), picture(), "{raw}");
    }
}

#[test]
fn an_unsupported_version_or_a_missing_map_entry_reads_as_a_picture() {
    let raw = r#"{"emoji":"😺","sticker":true}"#;
    for entry in [
        json!({"h": hlc(), "sv": 2}),
        json!({"h": hlc(), "sv": 0}),
        json!({"h": hlc()}),
        json!({"h": hlc(), "sv": "1"}),
        json!({"h": hlc(), "sv": null}),
        json!(null),
        json!([1]),
    ] {
        let (groups, hlcs) = maps(Some(raw), Some(entry.clone()));
        assert_eq!(read_sticker(&CORE, &groups, &hlcs), picture(), "{entry}");
    }
    // In only one of the two maps.
    let (groups, hlcs) = maps(Some(raw), None);
    assert_eq!(read_sticker(&CORE, &groups, &hlcs), picture());
    let (groups, hlcs) = maps(None, Some(json!({"h": hlc(), "sv": 1})));
    assert_eq!(read_sticker(&CORE, &groups, &hlcs), picture());
}

#[test]
fn a_group_that_is_not_canonical_or_too_large_reads_as_a_picture() {
    // The emoji written as an escape is valid JSON but not the canonical bytes (JCS writes the character itself).
    let escaped = format!(r#"{{"emoji":"{0}ud83d{0}ude3a","sticker":true}}"#, '\\');
    for raw in [
        r#"{"sticker":true,"emoji":"😺"}"#,
        r#"{ "emoji":"😺","sticker":true}"#,
        escaped.as_str(),
        r#"{"emoji":"😺","sticker":true,"a":1,"a":2}"#,
    ] {
        assert_eq!(read_raw(raw), picture(), "{raw}");
    }
    // Over 1 KiB, though canonical.
    let big = format!(
        r#"{{"emoji":"😺","pad":"{}","sticker":true}}"#,
        "x".repeat(MAX_GROUP_BYTES)
    );
    assert!(big.len() > MAX_GROUP_BYTES);
    assert_eq!(read_raw(&big), picture());
    // Unknown fields in a canonical group are kept and read.
    assert_eq!(
        read_raw(r#"{"emoji":"😺","extra":[1,2],"sticker":true}"#),
        sticker(Some("😺"))
    );
}

#[test]
fn a_core_outside_the_bounds_is_a_picture_whatever_the_group_says() {
    let raw = r#"{"emoji":"😺","sticker":true}"#;
    let (groups, hlcs) = maps(Some(raw), Some(json!({"h": hlc(), "sv": 1})));
    let read = |core: StickerCore<'_>| read_sticker(&core, &groups, &hlcs);
    assert_eq!(read(CORE), sticker(Some("😺")));
    let tests = [
        StickerCore {
            kind: AttachmentKind::File,
            ..CORE
        },
        StickerCore {
            kind: AttachmentKind::Audio,
            ..CORE
        },
        StickerCore {
            mime: "image/jpeg",
            ..CORE
        },
        StickerCore {
            mime: "image/gif",
            ..CORE
        },
        StickerCore {
            mime: "audio/mp4",
            ..CORE
        },
        StickerCore {
            width: Some(513),
            ..CORE
        },
        StickerCore {
            height: Some(513),
            ..CORE
        },
        StickerCore {
            width: Some(0),
            ..CORE
        },
        StickerCore {
            height: Some(0),
            ..CORE
        },
        StickerCore {
            width: None,
            ..CORE
        },
        StickerCore {
            height: None,
            ..CORE
        },
        StickerCore {
            byte_size: 2 * 1024 * 1024 + 1,
            ..CORE
        },
        StickerCore {
            byte_size: -1,
            ..CORE
        },
    ];
    for core in tests {
        assert_eq!(read(core), picture(), "{core:?}");
    }
    // The edges that pass: a PNG, the smallest and the largest side, exactly 2 MiB.
    let edge = StickerCore {
        mime: "image/png",
        width: Some(1),
        height: Some(512),
        byte_size: 2 * 1024 * 1024,
        ..CORE
    };
    assert_eq!(read(edge), sticker(Some("😺")));
}

#[test]
fn arbitrary_input_never_panics_and_reads_as_a_picture() {
    let garbage = [
        "",
        "{",
        "null",
        "[]",
        r#"{"sticker":"#,
        "\u{0}",
        r#"{"sticker":{"sticker":true}}"#,
    ];
    for text in garbage {
        for other in garbage {
            assert_eq!(read_sticker(&CORE, text, other), picture());
        }
    }
    let deep = format!("{}1{}", "[".repeat(300), "]".repeat(300));
    assert_eq!(read_raw(&deep), picture());
}

#[test]
fn the_emoji_is_a_valid_sequence_of_at_most_forty_bytes() {
    assert_eq!(clean_emoji("😺"), Some("😺"));
    assert_eq!(clean_emoji("👩‍👩‍👧‍👦"), Some("👩‍👩‍👧‍👦"));
    for bad in ["", "a", "😺x", "x😺", "<script>", "\u{0}", "😺😺"] {
        assert_eq!(clean_emoji(bad), None, "{bad:?}");
    }
    // A sequence longer than 40 bytes is refused even when well-formed.
    let long = ["👩"; 8].join("\u{200D}");
    assert!(long.len() > 40);
    assert_eq!(clean_emoji(&long), None);
    // The writer refuses what the reader would not show.
    let mut input = sticker_input(Some("not an emoji"));
    assert!(new_attachment(&input, &hlc()).is_err());
    input.sticker_emoji = None;
    input.sticker = false;
    input.sticker_emoji = Some("😺");
    assert!(
        new_attachment(&input, &hlc()).is_err(),
        "a picture has no emoji"
    );
}

#[test]
fn only_a_core_a_sticker_may_have_is_written_as_one() {
    let refuse = |change: &dyn Fn(&mut NewAttachment<'static>)| {
        let mut input = sticker_input(None);
        change(&mut input);
        assert!(new_attachment(&input, &hlc()).is_err());
    };
    refuse(&|i| i.mime = "image/jpeg");
    refuse(&|i| i.mime = "image/gif");
    refuse(&|i| i.width = Some(513));
    refuse(&|i| i.byte_size = 2 * 1024 * 1024 + 1);
    refuse(&|i| i.spoiler = true);
    refuse(&|i| {
        i.kind = AttachmentKind::File;
        i.width = None;
        i.height = None;
    });
    // A large or JPEG photo is written as a picture.
    let mut photo = sticker_input(None);
    photo.sticker = false;
    photo.mime = "image/jpeg";
    photo.width = Some(4000);
    let row = new_attachment(&photo, &hlc()).unwrap();
    assert!(!row.sticker);
}
