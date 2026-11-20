//! The `sticker` group (appendix §C.10), the rest: the display rule, the summary line, and the fingerprint that pins `RULES_VERSION`.

use super::sticker::{
    MAX_BYTES, MAX_EMOJI_BYTES, MAX_GROUP_BYTES, MAX_SIDE, RULES_VERSION, clean_emoji,
};
use super::sticker_tests::{ID, read_raw};
use super::*;

fn view(sticker: bool, spoiler: bool) -> AttachmentView {
    AttachmentView {
        id: ID.to_string(),
        kind: AttachmentKind::Image,
        filename: "sticker.webp".to_string(),
        name: "sticker.webp".to_string(),
        description: None,
        spoiler,
        sticker,
        sticker_emoji: Some("😺".to_string()),
        mime: "image/webp".to_string(),
        byte_size: 1,
        width: Some(512),
        height: Some(512),
        duration_ms: None,
        state: AttachmentState::Local,
        waveform: None,
    }
}

#[test]
fn an_entry_is_drawn_as_a_sticker_when_it_has_no_text_one_attachment_and_no_cover() {
    let one = [view(true, false)];
    assert!(sticker_of("", &one).is_some());
    assert!(
        sticker_of("text", &one).is_none(),
        "an edited entry keeps its text and shows a picture"
    );
    assert!(
        sticker_of("", &[view(true, true)]).is_none(),
        "a covered one is drawn as the cover"
    );
    assert!(sticker_of("", &[view(false, false)]).is_none());
    assert!(sticker_of("", &[view(true, false), view(true, false)]).is_none());
    assert!(sticker_of("", &[]).is_none());
}

#[test]
fn the_summary_says_sticker_only_for_one_uncovered_sticker() {
    let summary = summary_of(&[view(true, false)]);
    assert!(summary.sticker);
    assert_eq!(summary.sticker_emoji.as_deref(), Some("😺"));
    assert_eq!(summary.photos, 1);
    assert!(!summary_of(&[view(true, true)]).sticker);
    assert!(!summary_of(&[view(true, false), view(false, false)]).sticker);
    assert!(!summary_of(&[view(false, false)]).sticker);
}

/// Everything the derived columns are built from, as one number: the emoji table (shared with reactions), what `clean_emoji` lets through,
/// the bounds of the core, and the reader's verdict on a few records. A change to any of them changes the columns, so `RULES_VERSION`
/// must change with it.
fn rules_fingerprint() -> u64 {
    fn mix(hash: &mut u64, byte: u8) {
        *hash = (*hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    fn mix_u64(hash: &mut u64, value: u64) {
        value.to_le_bytes().iter().for_each(|byte| mix(hash, *byte));
    }
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for code in 0..=0x0010_FFFF_u32 {
        if char::from_u32(code).is_some_and(|c| clean_emoji(&c.to_string()).is_some()) {
            mix_u64(&mut hash, u64::from(code));
        }
    }
    for text in ["😺", "😺\u{FE0F}", "👩\u{200D}👩\u{200D}👧", "🇷🇺", "a", ""] {
        mix(&mut hash, u8::from(clean_emoji(text).is_some()));
    }
    mix_u64(&mut hash, u64::from(MAX_SIDE));
    mix_u64(&mut hash, MAX_BYTES as u64);
    mix_u64(&mut hash, MAX_GROUP_BYTES as u64);
    mix_u64(&mut hash, MAX_EMOJI_BYTES as u64);
    for raw in [
        r#"{"emoji":"😺","sticker":true}"#,
        r#"{"emoji":null,"sticker":true}"#,
        r#"{"emoji":"😺","sticker":false}"#,
        r#"{"sticker":true,"emoji":"😺"}"#,
    ] {
        let got = read_raw(raw);
        mix(&mut hash, u8::from(got.sticker));
        mix(&mut hash, u8::from(got.emoji.is_some()));
    }
    hash
}

#[test]
fn the_rules_version_changes_with_the_rules() {
    // If this fails, what the reader and the emoji check let through has changed. Bump `RULES_VERSION`, so that every device rebuilds the
    // derived columns on open, and then update both numbers here.
    assert_eq!(
        (RULES_VERSION, rules_fingerprint()),
        (1, 5_717_971_833_230_877_496)
    );
}
