//! Slice 7b: counting and normalizing, the bytes of the three groups, the total read function and its spoiler table.

use super::*;
use crate::groups::parse_groups_raw;
use serde_json::{Value, json};

const ID: &str = "9d3e4f50-6a7b-4c8d-9e0f-1a2b3c4d5e6f";
const MESSAGE: &str = "5b1c0d9e-3f2a-4a7b-9c8d-1e2f3a4b5c6d";
const BLOB: &str = "01934f2e-8a7b-7c3d-9e1f-2a3b4c5d6e7f";
const KEY: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
const SHA: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

/// The reference bytes of appendix §C.8 (ADR-0001) for a renamed, described and hidden photo: changing them changes the sync format.
const NAME_REFERENCE: &str = r#"{"name":"beach.jpg"}"#;
const DESCRIPTION_REFERENCE: &str = r#"{"description":"Beach, July"}"#;
const PRIVACY_REFERENCE: &str = r#"{"spoiler":true}"#;

fn hlc(l: u64) -> String {
    format!("{l:013}-00000-5f0c3e2a-9b1d-4c7e-8a64-0d2f1b3c4e5f")
}

fn input() -> NewAttachment<'static> {
    NewAttachment {
        id: ID,
        message_id: MESSAGE,
        kind: AttachmentKind::Image,
        sort_index: 0,
        sha256: SHA,
        filename: "IMG_0001.jpg",
        mime: "image/jpeg",
        byte_size: 245_760,
        width: Some(1280),
        height: Some(960),
        duration_ms: None,
        remote_blob_id: BLOB,
        blob_key: KEY,
        display_name: Some("beach.jpg"),
        description: Some("Beach, July"),
        spoiler: true,
    }
}

#[test]
fn the_three_groups_match_the_reference_bytes_of_appendix_c8() {
    let row = new_attachment(&input(), &hlc(1_793_000_000_000)).unwrap();
    let groups = parse_groups_raw(&row.groups_raw).unwrap();
    assert_eq!(groups["name"].get(), NAME_REFERENCE);
    assert_eq!(groups["description"].get(), DESCRIPTION_REFERENCE);
    assert_eq!(groups["privacy"].get(), PRIVACY_REFERENCE);
    assert_eq!(
        (
            row.display_name.as_deref(),
            row.description.as_deref(),
            row.spoiler
        ),
        (Some("beach.jpg"), Some("Beach, July"), true)
    );
}

#[test]
fn details_outside_the_rules_never_reach_the_bytes() {
    let long = "x".repeat(MAX_DESCRIPTION_CHARS + 1);
    for bad in [
        NewAttachment {
            display_name: Some("a/b.jpg"),
            ..input()
        },
        NewAttachment {
            display_name: Some("beach.png"),
            ..input()
        },
        NewAttachment {
            display_name: Some("IMG_0001.jpg"),
            ..input()
        },
        NewAttachment {
            display_name: Some(""),
            ..input()
        },
        NewAttachment {
            description: Some(&long),
            ..input()
        },
        NewAttachment {
            description: Some("a\rb"),
            ..input()
        },
        NewAttachment {
            description: Some(" padded "),
            ..input()
        },
        NewAttachment {
            description: Some(""),
            ..input()
        },
        NewAttachment {
            kind: AttachmentKind::File,
            width: None,
            height: None,
            ..input()
        },
    ] {
        assert!(new_attachment(&bad, &hlc(1)).is_err(), "{bad:?}");
    }
}

#[test]
fn a_name_is_a_stem_with_the_fixed_original_extension() {
    let name = |stem: &str, original: &str| normalize_name(stem, original);
    assert_eq!(name("beach", "IMG_0001.jpg"), Ok(Some("beach.jpg".into())));
    assert_eq!(
        name("  beach\u{a0} ", "IMG_0001.jpg"),
        Ok(Some("beach.jpg".into()))
    );
    assert_eq!(name("my.photo", "a.JPG"), Ok(Some("my.photo.JPG".into())));
    // Empty, or the original again: stored as the original.
    assert_eq!(name("", "IMG_0001.jpg"), Ok(None));
    assert_eq!(name("   ", "IMG_0001.jpg"), Ok(None));
    assert_eq!(name("IMG_0001", "IMG_0001.jpg"), Ok(None));
    // A file with no extension, or a dot file, has an empty suffix.
    assert_eq!(name("notes", "README"), Ok(Some("notes".into())));
    assert_eq!(name("x", ".bashrc"), Ok(Some("x".into())));
    // A dot in the stem of an extension-less file would change the extension: `README` must never become `README.doc`.
    assert!(name("README.doc", "README").is_err());
    assert!(name(".", "README").is_err());
}

#[test]
fn a_name_refuses_paths_controls_and_bidi_overrides() {
    for stem in [
        "a/b",
        "a\\b",
        "a\0b",
        "a\u{1b}b",
        "a\u{7f}b",
        "a\u{85}b",
        "a\u{202e}b",
        "a\u{2066}b",
        "a\nb",
    ] {
        assert!(normalize_name(stem, "x.jpg").is_err(), "{stem:?}");
    }
}

#[test]
fn a_name_counts_255_bytes_with_its_extension() {
    // `é` is two bytes: 125 of them and the 4 bytes of ".jpg" make 254; one more byte makes 255.
    let stem: String = "é".repeat(125);
    let full = normalize_name(&stem, "x.jpg").unwrap().unwrap();
    assert_eq!(full.len(), 254);
    let at_limit = format!("{stem}a");
    assert_eq!(
        normalize_name(&at_limit, "x.jpg").unwrap().unwrap().len(),
        255
    );
    assert!(normalize_name(&format!("{at_limit}a"), "x.jpg").is_err());
    assert!(normalize_name(&format!("{stem}é"), "x.jpg").is_err());
}

#[test]
fn a_description_is_normalized_then_counted_in_scalar_values() {
    let text = |raw: &str| normalize_description(raw);
    assert_eq!(text("a\r\nb\rc\nd"), Ok(Some("a\nb\nc\nd".into())));
    assert_eq!(text("a\tb"), Ok(Some("a b".into())));
    assert_eq!(text("  \n hi \n "), Ok(Some("hi".into())));
    assert_eq!(text(""), Ok(None));
    assert_eq!(text(" \t\r\n "), Ok(None));
    // Astral scalars count once: 1,024 fit, 1,025 do not, and 1,024 of them are 4,096 bytes.
    let astral = "😀".repeat(MAX_DESCRIPTION_CHARS);
    assert_eq!(astral.len(), MAX_DESCRIPTION_BYTES);
    assert_eq!(text(&astral), Ok(Some(astral.clone())));
    assert!(text(&format!("{astral}a")).is_err());
    // The count is taken after the CRLF pair became one character.
    let crlf = "a\r\n".repeat(511) + "a";
    assert_eq!(text(&crlf).unwrap().unwrap().chars().count(), 1023);
    for bad in ["a\0b", "a\u{1b}b", "a\u{7f}b", "a\u{85}b", "a\u{8}b"] {
        assert!(text(bad).is_err(), "{bad:?}");
    }
}

#[test]
fn the_extension_is_compared_as_bytes_or_ascii_folded_bytes() {
    assert!(valid_display_name("beach.JPG", "IMG.jpg"));
    assert!(!valid_display_name("beach.jpeg", "IMG.jpg"));
    // Unicode lowercasing would equate these; ASCII folding does not.
    assert!(!valid_display_name("a.\u{212a}", "a.k"));
    assert!(!valid_display_name("a.SS", "a.ß"));
    assert!(valid_display_name("b.ß", "a.ß"));
}

fn read(kind: AttachmentKind, groups: &[(&str, &str)], sv: &[(&str, u64)]) -> Details {
    let raw: Vec<String> = groups.iter().map(|(g, v)| format!("{g:?}:{v}")).collect();
    let hlcs: Vec<String> = sv
        .iter()
        .map(|(g, sv)| format!("{g:?}:{{\"h\":\"{}\",\"sv\":{sv}}}", hlc(1)))
        .collect();
    read_details(
        kind,
        "IMG.jpg",
        &format!("{{{}}}", raw.join(",")),
        &format!("{{{}}}", hlcs.join(",")),
    )
}

#[test]
fn the_spoiler_table_fails_closed() {
    let image = AttachmentKind::Image;
    let covered =
        |privacy: &str, sv: u64| read(image, &[("privacy", privacy)], &[("privacy", sv)]).spoiler;
    // Uncovered: the group is absent, or says `false`.
    assert!(!read(image, &[], &[]).spoiler);
    assert!(!covered(r#"{"spoiler":false}"#, 1));
    assert!(!covered(r#"{"spoiler":false,"future":1}"#, 1));
    // Covered: `true`, `null`, absent, mistyped, not an object.
    for privacy in [
        r#"{"spoiler":true}"#,
        r#"{"spoiler":null}"#,
        r#"{}"#,
        r#"{"spoiler":"false"}"#,
        r#"{"spoiler":0}"#,
        r#"{"spoiler":[false]}"#,
        r#"[false]"#,
        r#"false"#,
        r#""spoiler""#,
        r#"null"#,
    ] {
        assert!(covered(privacy, 1), "{privacy}");
    }
    // Covered: an unsupported `sv`, even when the value says `false`.
    assert!(covered(r#"{"spoiler":false}"#, 2));
    // Covered: the group is in only one of the two maps, or the maps do not read.
    assert!(read(image, &[("privacy", r#"{"spoiler":false}"#)], &[]).spoiler);
    assert!(read(image, &[], &[("privacy", 1)]).spoiler);
    assert!(read_details(image, "a.jpg", "not json", "{}").spoiler);
    assert!(read_details(image, "a.jpg", r#"{"privacy":{"spoiler":false}}"#, "x").spoiler);
    // A value on another kind is ignored.
    for kind in [
        AttachmentKind::File,
        AttachmentKind::Audio,
        AttachmentKind::Video,
    ] {
        for privacy in [r#"{"spoiler":true}"#, r#"null"#, r#"7"#] {
            let details = read(kind, &[("privacy", privacy)], &[("privacy", 1)]);
            assert!(!details.spoiler, "{kind:?} {privacy}");
        }
        assert!(!read_details(kind, "a.pdf", "not json", "{}").spoiler);
    }
}

#[test]
fn a_bad_name_shows_the_original_and_a_bad_description_shows_none() {
    let both = |name: &str, description: &str| {
        read(
            AttachmentKind::Image,
            &[("name", name), ("description", description)],
            &[("name", 1), ("description", 1)],
        )
    };
    let good = both(r#"{"name":"beach.jpg"}"#, r#"{"description":"Beach"}"#);
    assert_eq!(good.display_name.as_deref(), Some("beach.jpg"));
    assert_eq!(good.description.as_deref(), Some("Beach"));
    for name in [
        r#"{"name":"beach.png"}"#,
        r#"{"name":"a/b.jpg"}"#,
        r#"{"name":""}"#,
        r#"{"name":7}"#,
        r#"{"name":null}"#,
        r#"{}"#,
        r#"[]"#,
        r#"null"#,
    ] {
        assert_eq!(both(name, "{}").display_name, None, "{name}");
    }
    for description in [
        r#"{"description":" padded "}"#,
        r#"{"description":"a\r\nb"}"#,
        r#"{"description":"a\u0000b"}"#,
        r#"{"description":""}"#,
        r#"{"description":false}"#,
        r#"{"description":[]}"#,
        r#"null"#,
    ] {
        assert_eq!(both("{}", description).description, None, "{description}");
    }
    // A group in a newer `sv` is not understood: the default is shown.
    let future = read(
        AttachmentKind::Image,
        &[
            ("name", r#"{"name":"beach.jpg"}"#),
            ("description", r#"{"description":"x"}"#),
        ],
        &[("name", 2), ("description", 2)],
    );
    assert_eq!((future.display_name, future.description), (None, None));
}

/// A small deterministic generator: the same inputs on every run.
struct Rng(u64);

impl Rng {
    fn next(&mut self, below: u64) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 % below
    }
}

fn arbitrary(rng: &mut Rng, depth: u32) -> Value {
    const WORDS: &[&str] = &[
        "",
        "beach.jpg",
        "a/b",
        " x ",
        "a\u{0}b",
        "ok",
        "IMG.jpg",
        "😀",
        "x\r\ny",
        "\u{85}",
        "IMG.JPG",
    ];
    match rng.next(if depth > 2 { 5 } else { 7 }) {
        0 => Value::Null,
        1 => Value::Bool(rng.next(2) == 0),
        2 => json!(rng.next(5)),
        3 | 4 => Value::String(WORDS[rng.next(WORDS.len() as u64) as usize].to_string()),
        5 => Value::Array(
            (0..rng.next(3))
                .map(|_| arbitrary(rng, depth + 1))
                .collect(),
        ),
        _ => {
            let keys = ["name", "description", "spoiler", "sv", "x"];
            Value::Object(
                (0..rng.next(4))
                    .map(|_| {
                        (
                            keys[rng.next(5) as usize].to_string(),
                            arbitrary(rng, depth + 1),
                        )
                    })
                    .collect(),
            )
        }
    }
}

#[test]
fn reading_is_total_and_yields_only_valid_values() {
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let kinds = [
        AttachmentKind::Image,
        AttachmentKind::File,
        AttachmentKind::Audio,
    ];
    for round in 0..4000 {
        let mut groups = serde_json::Map::new();
        let mut hlcs = serde_json::Map::new();
        for group in ["name", "description", "privacy"] {
            if rng.next(5) != 0 {
                groups.insert(group.into(), arbitrary(&mut rng, 0));
            }
            if rng.next(5) != 0 {
                hlcs.insert(group.into(), arbitrary(&mut rng, 1));
            }
            if rng.next(3) != 0 {
                hlcs.insert(group.into(), json!({"h": hlc(1), "sv": rng.next(3)}));
            }
        }
        let kind = kinds[(round % 3) as usize];
        let details = read_details(
            kind,
            "IMG.jpg",
            &Value::Object(groups).to_string(),
            &Value::Object(hlcs).to_string(),
        );
        if let Some(name) = &details.display_name {
            assert!(valid_display_name(name, "IMG.jpg"), "{name:?}");
        }
        if let Some(text) = &details.description {
            assert_eq!(
                normalize_description(text).as_ref(),
                Ok(&Some(text.clone()))
            );
        }
        assert!(kind == AttachmentKind::Image || !details.spoiler);
    }
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
