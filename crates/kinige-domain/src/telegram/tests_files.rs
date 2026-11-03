//! Tests of the files of messages (slice 7): the path, name, MIME type, size and picture size are read from the export without
//! trusting them, "File not included" is "not in the export", and a message whose file is attached loses only its placeholder.

use super::markdown_tests::{msg, plain};
use super::tests::one;
use super::*;
use crate::lang::Language;

#[test]
fn a_photo_and_a_file_carry_their_details() {
    let photo = one(
        r#"{"id":1,"type":"message","photo":"photos/IMG_0001.jpg","photo_file_size":184320,"width":1280,"height":960,"text":""}"#,
    );
    assert_eq!(photo.media, Some(TgMedia::Photo));
    assert_eq!(
        photo.file,
        Some(TgFile {
            path: Some("photos/IMG_0001.jpg".into()),
            name: None,
            mime: None,
            size: Some(184_320),
            width: Some(1280),
            height: Some(960),
            duration_seconds: None,
        })
    );
    let file = one(
        r#"{"id":2,"type":"message","file":"files/menu_spring.pdf","file_name":"menu_spring.pdf","file_size":"2048","mime_type":"application/pdf","text":"меню"}"#,
    );
    let details = file.file.unwrap();
    assert_eq!(details.path.as_deref(), Some("files/menu_spring.pdf"));
    assert_eq!(details.name.as_deref(), Some("menu_spring.pdf"));
    assert_eq!(details.mime.as_deref(), Some("application/pdf"));
    assert_eq!(details.size, Some(2048), "a size that came as a string");
    let audio = one(
        r#"{"id":3,"type":"message","file":"files/t.mp3","media_type":"audio_file","mime_type":"audio/mpeg","duration_seconds":214,"text":""}"#,
    );
    assert_eq!(audio.media, Some(TgMedia::AudioFile));
    assert_eq!(audio.file.unwrap().duration_seconds, Some(214));
    for kind in ["video_file", "video_message", "voice_message"] {
        let m = one(&format!(
            r#"{{"id":4,"type":"message","file":"x/y.bin","media_type":"{kind}","text":""}}"#
        ));
        assert!(m.file.is_some(), "{kind}");
    }
}

#[test]
fn stickers_and_animations_stay_placeholders_without_a_file() {
    for kind in ["sticker", "animation"] {
        let m = one(&format!(
            r#"{{"id":5,"type":"message","file":"stickers/s.webp","media_type":"{kind}","text":""}}"#
        ));
        assert!(m.media.is_some() && m.file.is_none(), "{kind}");
    }
    let location = one(
        r#"{"id":6,"type":"message","location_information":{"latitude":1.0,"longitude":2.0},"text":""}"#,
    );
    assert!(location.file.is_none());
}

#[test]
fn a_note_instead_of_a_path_means_the_file_is_not_in_the_export() {
    for note in [
        "(File not included. Change data exporting settings to download.)",
        "(File exceeds maximum size. Change data exporting settings to download.)",
        "",
    ] {
        let m = one(&format!(
            r#"{{"id":7,"type":"message","photo":"{note}","text":"подпись"}}"#
        ));
        assert_eq!(m.media, Some(TgMedia::Photo), "{note:?}");
        assert_eq!(m.file.unwrap().path, None, "{note:?}");
    }
}

#[test]
fn odd_values_never_fail_the_parse() {
    let cases = [
        r#""photo": 12"#,
        r#""photo": {"a": 1}"#,
        r#""photo": null"#,
        r#""photo": ["x"]"#,
        r#""photo": "p.jpg", "width": "wide", "height": -5, "photo_file_size": 1.5e300"#,
        r#""file": "f", "file_size": {"x": 1}, "mime_type": 5, "duration_seconds": [1]"#,
        r#""file": "f", "file_size": 3.9, "duration_seconds": 12.7"#,
        r#""file": "f", "file_size": 18446744073709551615, "width": 99999999999"#,
    ];
    for fields in cases {
        let m = one(&format!(
            r#"{{"id":8,"type":"message",{fields},"text":""}}"#
        ));
        let _ = m.file;
    }
    let fractions = one(
        r#"{"id":9,"type":"message","file":"f","file_size":3.9,"duration_seconds":12.7,"text":""}"#,
    )
    .file
    .unwrap();
    assert_eq!(
        (fractions.size, fractions.duration_seconds),
        (Some(3), Some(12))
    );
    let huge = one(r#"{"id":9,"type":"message","file":"f","width":99999999999,"text":""}"#)
        .file
        .unwrap();
    assert_eq!(huge.width, None, "does not fit a u32");
    let hostile_path = one(r#"{"id":10,"type":"message","photo":"../../etc/passwd","text":""}"#);
    assert_eq!(
        hostile_path.file.unwrap().path.as_deref(),
        Some("../../etc/passwd"),
        "the parser keeps it as written; the importer refuses it"
    );
}

fn photo_message(caption: &str) -> TgMessage {
    let mut message = msg(if caption.is_empty() {
        vec![]
    } else {
        vec![plain(caption)]
    });
    message.media = Some(TgMedia::Photo);
    message.file = Some(TgFile {
        path: Some("photos/a.jpg".into()),
        name: None,
        mime: None,
        size: None,
        width: None,
        height: None,
        duration_seconds: None,
    });
    message
}

#[test]
fn an_attached_file_takes_its_placeholder_line_out_and_nothing_else() {
    let message = photo_message("вечер");
    assert_eq!(
        render_body(&message, None, Language::En).unwrap(),
        "_[Photo not imported]_\nвечер"
    );
    assert_eq!(
        render_body_attached(&message, None, Language::En).unwrap(),
        "вечер"
    );
    assert_eq!(
        render_body_attached(&message, Some("Аня"), Language::Ru).unwrap(),
        "**Аня**\nвечер"
    );
    // A photo with no caption: the body is empty, which is valid because the entry has the attachment.
    let bare = photo_message("");
    assert_eq!(
        render_body(&bare, None, Language::Ru).unwrap(),
        "_[Фото не импортировано]_"
    );
    assert_eq!(render_body_attached(&bare, None, Language::Ru).unwrap(), "");
    // Without a file in the export the placeholder stays.
    let mut absent = photo_message("");
    absent.file = None;
    assert_eq!(
        render_body_attached(&absent, None, Language::En).unwrap(),
        "_[Photo not imported]_"
    );
    // A sticker's placeholder is not touched even if a file field exists.
    let mut sticker = msg(vec![]);
    sticker.media = Some(TgMedia::Animation);
    assert_eq!(
        render_body_attached(&sticker, None, Language::En).unwrap(),
        "_[Animation not imported]_"
    );
}
