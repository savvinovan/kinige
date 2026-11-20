use super::*;
use serde_json::json;

fn sticker(id: &str) -> Value {
    json!({"file_id": id, "file_unique_id": "u", "width": 512, "height": 384, "is_animated": false, "is_video": false,
           "emoji": "😺", "file_size": 20_000, "type": "regular"})
}

fn set(stickers: Vec<Value>) -> Vec<u8> {
    serde_json::to_vec(&json!({"ok": true, "result": {"name": "TestPack", "title": "Test Pack", "stickers": stickers}})).unwrap()
}

#[test]
fn a_set_is_read_into_the_validated_structure() {
    let body = set(vec![sticker("A1"), sticker("A2")]);
    let got = read_sticker_set(&body).unwrap();
    assert_eq!(got.title, "Test Pack");
    assert_eq!(got.skipped, 0);
    assert_eq!(
        got.stickers,
        vec![
            SetSticker {
                file_id: "A1".into(),
                emoji: Some("😺".into()),
                file_size: Some(20_000)
            },
            SetSticker {
                file_id: "A2".into(),
                emoji: Some("😺".into()),
                file_size: Some(20_000)
            },
        ]
    );
}

#[test]
fn animated_video_and_unusable_stickers_are_counted_and_never_requested() {
    let mut animated = sticker("B1");
    animated["is_animated"] = json!(true);
    let mut video = sticker("B2");
    video["is_video"] = json!(true);
    let mut bad_id = sticker("x");
    bad_id["file_id"] = json!("a&b=c#d");
    let mut wide = sticker("B3");
    wide["width"] = json!(513);
    let mut huge = sticker("B4");
    huge["file_size"] = json!(MAX_STICKER_BYTES + 1);
    let mut flag = sticker("B5");
    flag["is_video"] = json!("no");
    let mut bad_size = sticker("B6");
    bad_size["file_size"] = json!("big");
    let body = set(vec![
        sticker("A1"),
        animated,
        video,
        bad_id,
        wide,
        huge,
        flag,
        bad_size,
        json!(7),
        json!(null),
    ]);
    let got = read_sticker_set(&body).unwrap();
    assert_eq!(got.stickers.len(), 1);
    assert_eq!(got.skipped, 9);
}

#[test]
fn a_file_id_twice_is_requested_once() {
    let got = read_sticker_set(&set(vec![sticker("A1"), sticker("A1"), sticker("A2")])).unwrap();
    assert_eq!(got.stickers.len(), 2);
    assert_eq!(got.skipped, 0);
}

#[test]
fn an_emoji_that_is_not_an_emoji_becomes_none() {
    let mut a = sticker("A1");
    a["emoji"] = json!("<script>alert(1)</script>");
    let mut b = sticker("A2");
    b["emoji"] = json!(5);
    let mut c = sticker("A3");
    c.as_object_mut().unwrap().remove("emoji");
    let got = read_sticker_set(&set(vec![a, b, c])).unwrap();
    assert!(got.stickers.iter().all(|s| s.emoji.is_none()));
}

#[test]
fn a_pack_of_only_animated_stickers_says_so() {
    let mut animated = sticker("B1");
    animated["is_animated"] = json!(true);
    assert_eq!(
        read_sticker_set(&set(vec![animated])),
        Err(ReadError::OnlyAnimated)
    );
    assert_eq!(read_sticker_set(&set(vec![])), Err(ReadError::BadResponse));
    assert_eq!(
        read_sticker_set(&set(vec![json!({})])),
        Err(ReadError::BadResponse)
    );
}

#[test]
fn a_201st_sticker_and_a_pack_over_64_mib_are_too_large() {
    let many: Vec<Value> = (0..=MAX_PACK_STICKERS)
        .map(|i| sticker(&format!("S{i}")))
        .collect();
    assert_eq!(many.len(), 201);
    assert_eq!(read_sticker_set(&set(many)), Err(ReadError::TooLarge));
    let exactly: Vec<Value> = (0..MAX_PACK_STICKERS)
        .map(|i| sticker(&format!("S{i}")))
        .collect();
    assert!(read_sticker_set(&set(exactly)).is_ok());
    let heavy: Vec<Value> = (0..40)
        .map(|i| {
            let mut s = sticker(&format!("H{i}"));
            s["file_size"] = json!(MAX_STICKER_BYTES);
            s
        })
        .collect();
    assert_eq!(read_sticker_set(&set(heavy)), Err(ReadError::TooLarge));
}

#[test]
fn a_title_that_is_hostile_is_cleaned() {
    let body = serde_json::to_vec(
        &json!({"ok": true, "result": {"title": "../../etc\u{202E}", "stickers": [sticker("A1")]}}),
    )
    .unwrap();
    let got = read_sticker_set(&body).unwrap();
    assert_eq!(got.title, "etc");
    let body = serde_json::to_vec(
        &json!({"ok": true, "result": {"title": 5, "stickers": [sticker("A1")]}}),
    )
    .unwrap();
    assert_eq!(
        read_sticker_set(&body).unwrap().title,
        crate::stickers::DEFAULT_TITLE
    );
}

#[test]
fn garbage_is_a_bad_response_and_never_a_panic() {
    let deep = format!("{}1{}", "[".repeat(5_000), "]".repeat(5_000));
    let garbage: [&[u8]; 12] = [
        b"",
        b"not json",
        b"{",
        b"null",
        b"[]",
        b"{\"ok\":false,\"result\":{}}",
        b"{\"ok\":\"true\",\"result\":{}}",
        b"{\"ok\":true}",
        b"{\"ok\":true,\"result\":[]}",
        b"{\"ok\":true,\"result\":{\"stickers\":\"x\"}}",
        b"\xff\xfe\x00",
        deep.as_bytes(),
    ];
    for body in garbage {
        assert_eq!(
            read_sticker_set(body),
            Err(ReadError::BadResponse),
            "{body:?}"
        );
        assert_eq!(
            read_file_path(body),
            Err(ReadError::BadResponse),
            "{body:?}"
        );
    }
}

#[test]
fn a_file_path_is_read_only_when_it_is_safe() {
    let body = |path: Value| {
        serde_json::to_vec(&json!({"ok": true, "result": {"file_id": "A1", "file_path": path}}))
            .unwrap()
    };
    assert_eq!(
        read_file_path(&body(json!("stickers/file_1.webp"))).unwrap(),
        "stickers/file_1.webp"
    );
    for bad in [
        json!("../etc/passwd"),
        json!("/abs"),
        json!("a b"),
        json!("a?b"),
        json!(""),
        json!(7),
        json!(null),
    ] {
        assert_eq!(
            read_file_path(&body(bad.clone())),
            Err(ReadError::BadResponse),
            "{bad}"
        );
    }
    assert_eq!(
        read_file_path(br#"{"ok":true,"result":{"file_id":"A1"}}"#),
        Err(ReadError::BadResponse)
    );
}
