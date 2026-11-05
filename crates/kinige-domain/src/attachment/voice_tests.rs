use super::*;
use crate::groups::parse_groups_raw;

const ID: &str = "9d3e4f50-6a7b-4c8d-9e0f-1a2b3c4d5e6f";
const MESSAGE: &str = "5b1c0d9e-3f2a-4a7b-9c8d-1e2f3a4b5c6d";
const BLOB: &str = "01934f2e-8a7b-7c3d-9e1f-2a3b4c5d6e7f";
const KEY: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
const SHA: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

/// The reference bytes of an `audio` row with a non-null `duration_ms` (appendix §C.6): changing them changes the sync format.
const AUDIO_CORE_REFERENCE: &str = r#"{"blob_key":"000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f","byte_size":321536,"duration_ms":40000,"filename":"voice-2026-11-04-213012.m4a","height":null,"kind":"audio","link_site":null,"link_title":null,"message_id":"5b1c0d9e-3f2a-4a7b-9c8d-1e2f3a4b5c6d","mime":"audio/mp4","remote_blob_id":"01934f2e-8a7b-7c3d-9e1f-2a3b4c5d6e7f","sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","url":null,"width":null}"#;

fn audio(duration_ms: Option<i64>) -> NewAttachment<'static> {
    NewAttachment {
        id: ID,
        message_id: MESSAGE,
        kind: AttachmentKind::Audio,
        sort_index: 0,
        sha256: SHA,
        filename: "voice-2026-11-04-213012.m4a",
        mime: "audio/mp4",
        byte_size: 321_536,
        width: None,
        height: None,
        duration_ms,
        remote_blob_id: BLOB,
        blob_key: KEY,
    }
}

fn hlc() -> String {
    format!(
        "{:013}-00000-5f0c3e2a-9b1d-4c7e-8a64-0d2f1b3c4e5f",
        1_793_000_000_000u64
    )
}

#[test]
fn an_audio_core_with_a_duration_matches_the_reference_bytes() {
    let row = new_attachment(&audio(Some(40_000)), &hlc()).unwrap();
    let groups = parse_groups_raw(&row.groups_raw).unwrap();
    assert_eq!(groups["core"].get(), AUDIO_CORE_REFERENCE);
}

#[test]
fn every_existing_writer_satisfies_the_duration_rule() {
    // The Telegram import writes a `u32` of seconds times 1000: up to 4_294_967_295_000, for audio and video alike.
    for ms in [0, 1, 40_000, 90_000_000, MAX_DURATION_MS] {
        for kind in [AttachmentKind::Audio, AttachmentKind::Video] {
            let input = NewAttachment {
                kind,
                ..audio(Some(ms))
            };
            assert!(new_attachment(&input, &hlc()).is_ok(), "{kind:?} {ms}");
        }
    }
    assert!(new_attachment(&audio(None), &hlc()).is_ok());
}

#[test]
fn a_duration_outside_the_rule_is_refused() {
    for ms in [-1, MAX_DURATION_MS + 1, i64::MAX, i64::MIN] {
        assert!(new_attachment(&audio(Some(ms)), &hlc()).is_err(), "{ms}");
    }
    // Only audio and video have a length.
    for (kind, name, mime) in [
        (AttachmentKind::Image, "a.jpg", "image/jpeg"),
        (AttachmentKind::File, "a.pdf", "application/pdf"),
    ] {
        let input = NewAttachment {
            kind,
            filename: name,
            mime,
            ..audio(Some(1_000))
        };
        assert!(new_attachment(&input, &hlc()).is_err(), "{kind:?}");
    }
}

#[test]
fn the_summary_counts_audio_as_voice_notes_with_their_total_length() {
    let view = |kind, name: &str, duration_ms| AttachmentView {
        id: ID.to_string(),
        kind,
        filename: name.to_string(),
        mime: "x/y".to_string(),
        byte_size: 1,
        width: None,
        height: None,
        duration_ms,
        state: AttachmentState::Local,
        waveform: None,
    };
    let one = summary_of(&[view(AttachmentKind::Audio, "v.m4a", Some(40_000))]);
    assert_eq!(
        (one.voices, one.voice_ms, one.files, one.name.as_deref()),
        (1, Some(40_000), 0, Some("v.m4a"))
    );
    let two = summary_of(&[
        view(AttachmentKind::Audio, "a.ogg", Some(10_000)),
        view(AttachmentKind::Audio, "b.ogg", None),
    ]);
    assert_eq!(
        (two.voices, two.voice_ms, two.name),
        (2, Some(10_000), None)
    );
    let unknown = summary_of(&[view(AttachmentKind::Audio, "a.ogg", None)]);
    assert_eq!(unknown.voice_ms, None);
    let mixed = summary_of(&[
        view(AttachmentKind::Image, "a.jpg", None),
        view(AttachmentKind::Audio, "a.ogg", Some(5_000)),
        view(AttachmentKind::Video, "a.mp4", Some(9_000)),
    ]);
    assert_eq!((mixed.photos, mixed.voices, mixed.files), (1, 1, 1));
}

#[test]
fn the_container_comes_from_the_first_bytes() {
    let mp4 = [
        0, 0, 0, 0x20, b'f', b't', b'y', b'p', b'M', b'4', b'A', b' ',
    ];
    assert_eq!(VoiceContainer::detect(&mp4), Some(VoiceContainer::Mp4));
    assert_eq!(
        VoiceContainer::detect(&[0x1A, 0x45, 0xDF, 0xA3, 0x9F]),
        Some(VoiceContainer::WebM)
    );
    // Anything else, including every length that is too short to tell.
    assert_eq!(VoiceContainer::detect(&[]), None);
    assert_eq!(VoiceContainer::detect(&[0x1A, 0x45, 0xDF]), None);
    assert_eq!(VoiceContainer::detect(b"ftyp"), None);
    assert_eq!(VoiceContainer::detect(b"OggS\0\x02\0\0\0\0\0\0"), None);
    assert_eq!(VoiceContainer::detect(b"\x89PNG\r\n\x1a\n\0\0\0\r"), None);
    assert_eq!(VoiceContainer::Mp4.ext(), "m4a");
    assert_eq!(VoiceContainer::WebM.mime(), "audio/webm");
}

#[test]
fn the_recorder_names_a_file_in_local_time_and_recognises_its_own_names() {
    // 2026-11-04 18:30:12 UTC is 21:30:12 in Moscow and 13:30:12 in New York.
    let utc_ms = 1_793_817_012_000;
    assert_eq!(
        voice_file_name(utc_ms, 3 * 3600, VoiceContainer::Mp4).as_deref(),
        Some("voice-2026-11-04-213012.m4a")
    );
    assert_eq!(
        voice_file_name(utc_ms, -5 * 3600, VoiceContainer::WebM).as_deref(),
        Some("voice-2026-11-04-133012.webm")
    );
    // Across midnight the date moves with the zone.
    assert_eq!(
        voice_file_name(utc_ms, 6 * 3600, VoiceContainer::Mp4).as_deref(),
        Some("voice-2026-11-05-003012.m4a")
    );
    assert_eq!(voice_file_name(i64::MAX, 0, VoiceContainer::Mp4), None);
    for good in [
        "voice-2026-11-04-213012.m4a",
        "voice-2026-11-04-213012.webm",
    ] {
        assert!(is_voice_file_name(good), "{good}");
    }
    for bad in [
        "voice.m4a",
        "voice-2026-11-04-213012.mp3",
        "voice-2026-11-04-21301.m4a",
        "voice-2026-11-04-2130123.m4a",
        "voice-2026x11-04-213012.m4a",
        "Voice-2026-11-04-213012.m4a",
        "audio_file_1.ogg",
        "",
    ] {
        assert!(!is_voice_file_name(bad), "{bad}");
    }
    // The name passes the file name rules of the `core` group as it is.
    assert_eq!(
        sanitize_filename("voice-2026-11-04-213012.m4a"),
        "voice-2026-11-04-213012.m4a"
    );
    assert_eq!(stored_ext("voice-2026-11-04-213012.webm", None), "webm");
}

#[test]
fn durations_read_as_minutes_and_seconds_and_hours_past_an_hour() {
    for (ms, text) in [
        (-5, "0:00"),
        (0, "0:00"),
        (999, "0:00"),
        (1_000, "0:01"),
        (40_000, "0:40"),
        (59_999, "0:59"),
        (60_000, "1:00"),
        (3_599_000, "59:59"),
        (3_600_000, "1:00:00"),
        (3_723_000, "1:02:03"),
        (MAX_DURATION_MS, "1193046:28:15"),
    ] {
        assert_eq!(format_duration(ms), text, "{ms}");
    }
}

#[test]
fn only_audio_types_on_the_allowlist_are_served() {
    for allowed in [
        "audio/mp4",
        "audio/aac",
        "audio/mpeg",
        "audio/ogg",
        "audio/opus",
        "audio/wav",
        "audio/flac",
        "audio/webm",
    ] {
        assert_eq!(playable_audio_mime(allowed), Some(allowed));
    }
    for other in [
        "",
        "audio/",
        "audio/x-wav",
        "audio/AAC",
        "audio/mp4; codecs=mp4a",
        "text/html",
        "image/svg+xml",
        "video/webm",
        "application/octet-stream",
        "audio/mp4\r\nX-Evil: 1",
    ] {
        assert_eq!(playable_audio_mime(other), None, "{other:?}");
    }
}

#[test]
fn the_first_bytes_must_fit_the_claimed_container() {
    let mp4 = b"\0\0\0\x20ftypM4A \0\0\0\0";
    let ogg = b"OggS\0\x02\0\0\0\0\0\0";
    let wav = b"RIFF\x24\x08\0\0WAVEfmt ";
    let flac = b"fLaC\0\0\0\x22\x10\0\x10\0";
    let id3 = b"ID3\x04\0\0\0\0\0\x21\0\0";
    let frame = [0xFF, 0xFB, 0x90, 0x64, 0, 0, 0, 0, 0, 0, 0, 0];
    let ebml = [0x1A, 0x45, 0xDF, 0xA3, 0x9F, 0x42, 0x86, 0x81, 1, 0, 0, 0];
    assert!(audio_head_matches("audio/mp4", mp4));
    assert!(audio_head_matches("audio/aac", mp4));
    assert!(audio_head_matches(
        "audio/aac",
        &[0xFF, 0xF1, 0x50, 0x80, 0, 0]
    ));
    assert!(audio_head_matches("audio/mpeg", id3));
    assert!(audio_head_matches("audio/mpeg", &frame));
    assert!(audio_head_matches("audio/ogg", ogg));
    assert!(audio_head_matches("audio/opus", ogg));
    assert!(audio_head_matches("audio/wav", wav));
    assert!(audio_head_matches("audio/flac", flac));
    assert!(audio_head_matches("audio/webm", &ebml));
    // A row that says one thing about a file that is another.
    assert!(!audio_head_matches("audio/mp4", b"<html><script>"));
    assert!(!audio_head_matches("audio/mp4", ogg));
    assert!(!audio_head_matches("audio/ogg", mp4));
    assert!(!audio_head_matches("audio/wav", b"RIFF\0\0\0\0AVI LIST"));
    assert!(!audio_head_matches("audio/mpeg", &[0xFF, 0x00, 0, 0]));
    assert!(!audio_head_matches("audio/webm", mp4));
    // Short or empty input and an unknown type never match.
    for short in [&b""[..], &b"O"[..], &b"RIFF"[..], &b"\0\0\0\x20ftyp"[..7]] {
        for kind in [
            "audio/mp4",
            "audio/ogg",
            "audio/wav",
            "audio/webm",
            "audio/mpeg",
        ] {
            assert!(!audio_head_matches(kind, short), "{kind} {short:?}");
        }
    }
    assert!(!audio_head_matches("text/html", b"<html>"));
}
