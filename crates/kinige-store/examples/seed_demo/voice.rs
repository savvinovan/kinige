//! Slice 7a demo data: voice notes, only with `KINIGE_SEED_VOICE=1`. A WAV is synthesized here (a quiet two-tone chirp, no third-party
//! audio) and turned into AAC in MP4 by the macOS tool `afconvert` (present on the screens runner); where it is missing, the notes that
//! need a playable file are skipped and the rest is still seeded. Sent through the store API like the app does: a 40 second note, a
//! Telegram-style `.ogg` (synthetic bytes), a note whose file is then deleted (the startup sweep marks it missing), an hour-long note and a
//! note with edited text.

use kinige_domain::groups::INBOX_ID;
use kinige_store::{Now, Store};
use std::path::Path;
use std::process::Command;

const MINUTE: i64 = 60_000;
const RATE: u32 = 8000;

pub fn seed_voice(store: &Store, now: &Now, data: &Path) {
    let at = |ms: i64| Now::at(now.ms + ms, &now.tz);
    let work = data.join("seed-voice");
    std::fs::create_dir_all(&work).expect("seed voice folder");
    let m4a = encode(&work, 40);
    // A Telegram voice message: only the header looks like Ogg; the player shows it at rest and "Can't play here" if pressed.
    let ogg = work.join("audio_1.ogg");
    let mut ogg_bytes = b"OggS\0\x02\0\0\0\0\0\0\0\0".to_vec();
    ogg_bytes.resize(4096, 0);
    std::fs::write(&ogg, ogg_bytes).expect("ogg");
    let id = store
        .attach_path(INBOX_ID, &ogg, &at(-50 * MINUTE))
        .expect("attach ogg")
        .id;
    store
        .send_entry(INBOX_ID, "", None, &[id], &at(-50 * MINUTE))
        .expect("send ogg");

    let Some(bytes) = m4a else {
        println!("afconvert is not available: playable voice notes are skipped");
        return;
    };
    let note = |ago: i64, ms: i64| {
        store
            .send_voice(INBOX_ID, None, &bytes, ms, &at(-ago * MINUTE))
            .expect("send voice")
    };
    note(40, 40_000);
    // Its file is gone: the sweep at startup marks it `missing`.
    let gone = note(35, 40_000);
    let stored = data.join(format!("attachments/{}.m4a", gone.attachments[0].id));
    std::fs::remove_file(stored).expect("remove the file of one note");
    // An hour-long note: the column is set directly (a recorder never makes one; a Telegram audio file can be this long).
    let long = note(30, 40_000);
    rusqlite::Connection::open(data.join("kinige.db"))
        .and_then(|conn| {
            conn.execute(
                "UPDATE attachment SET duration_ms = 3723000 WHERE id = ?1",
                [&long.attachments[0].id],
            )
        })
        .expect("long duration");
    let edited = note(20, 40_000);
    store
        .edit_message(
            &edited.id,
            "Идея для поездки: взять плёнку.",
            &at(-19 * MINUTE),
        )
        .expect("edit");
}

/// Speaks `seconds` of a gentle two-tone chirp as AAC in MP4, or `None` without `afconvert`.
fn encode(work: &Path, seconds: u32) -> Option<Vec<u8>> {
    let wav = work.join("tone.wav");
    let out = work.join("tone.m4a");
    std::fs::write(&wav, synth_wav(seconds)).expect("wav");
    let status = Command::new("afconvert")
        .args(["-f", "m4af", "-d", "aac"])
        .arg(&wav)
        .arg(&out)
        .status()
        .ok()?;
    status.success().then(|| std::fs::read(&out).ok())?
}

/// 16-bit mono PCM at 8 kHz.
fn synth_wav(seconds: u32) -> Vec<u8> {
    let samples = RATE * seconds;
    let mut data = Vec::with_capacity(samples as usize * 2);
    for n in 0..samples {
        let t = f64::from(n) / f64::from(RATE);
        let tone = if ((t * 2.0) as u32).is_multiple_of(2) {
            330.0
        } else {
            440.0
        };
        let sample = (t * tone * std::f64::consts::TAU).sin() * 0.15;
        data.extend_from_slice(&((sample * f64::from(i16::MAX)) as i16).to_le_bytes());
    }
    let mut wav = Vec::with_capacity(data.len() + 44);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&RATE.to_le_bytes());
    wav.extend_from_slice(&(RATE * 2).to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&(data.len() as u32).to_le_bytes());
    wav.extend_from_slice(&data);
    wav
}
