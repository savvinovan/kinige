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
    // Two takes with different loudness shapes (phrases and pauses). The notes of the first have no waveform in their rows, so the player
    // decodes the file in the web view (the lazy path); the notes of the second carry peaks computed here from the same samples, the way a
    // recording carries the recorder's.
    let m4a = encode(&work, 40, Take::Decoded);
    let seeded = encode(&work, 40, Take::Seeded);
    let seeded_peaks = peaks_of(&samples(40, Take::Seeded));
    // A Telegram voice message (the newest seeded entry, so that the first player in the feed is a playable note): only the header looks like Ogg; the player shows it at rest and "Can't play here" if pressed.
    let ogg = work.join("audio_1.ogg");
    let mut ogg_bytes = b"OggS\0\x02\0\0\0\0\0\0\0\0".to_vec();
    ogg_bytes.resize(4096, 0);
    std::fs::write(&ogg, ogg_bytes).expect("ogg");
    let id = store
        .attach_path(INBOX_ID, &ogg, &at(-2 * MINUTE))
        .expect("attach ogg")
        .id;
    store
        .send_entry(INBOX_ID, "", None, &[id], &at(-2 * MINUTE))
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
    // A note with peaks from the "recorder": a short one and a longer one.
    if let Some(bytes) = &seeded {
        for (ago, ms) in [(38, 5_000), (37, 2_000)] {
            store
                .send_voice_with_waveform(
                    INBOX_ID,
                    None,
                    bytes,
                    ms,
                    Some(&seeded_peaks),
                    &at(-ago * MINUTE),
                )
                .expect("send voice with peaks");
        }
    }
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

/// Which loudness shape a take has.
#[derive(Clone, Copy)]
enum Take {
    Decoded,
    Seeded,
}

/// Speaks `seconds` of a gentle two-tone chirp as AAC in MP4, or `None` without `afconvert`.
fn encode(work: &Path, seconds: u32, take: Take) -> Option<Vec<u8>> {
    let name = match take {
        Take::Decoded => "decoded",
        Take::Seeded => "seeded",
    };
    let wav = work.join(format!("{name}.wav"));
    let out = work.join(format!("{name}.m4a"));
    std::fs::write(&wav, wav_of(&samples(seconds, take))).expect("wav");
    let status = Command::new("afconvert")
        .args(["-f", "m4af", "-d", "aac"])
        .arg(&wav)
        .arg(&out)
        .status()
        .ok()?;
    status.success().then(|| std::fs::read(&out).ok())?
}

/// 16-bit mono samples at 8 kHz: the two-tone chirp under a loudness that rises and falls in phrases with short pauses, a different
/// shape for each take.
fn samples(seconds: u32, take: Take) -> Vec<i16> {
    let (phrase, syllable) = match take {
        Take::Decoded => (0.9, 5.3),
        Take::Seeded => (0.6, 7.9),
    };
    (0..RATE * seconds)
        .map(|n| {
            let t = f64::from(n) / f64::from(RATE);
            let tone = if ((t * 2.0) as u32).is_multiple_of(2) {
                330.0
            } else {
                440.0
            };
            let loudness =
                (t * phrase).sin().abs().powf(0.7) * (0.45 + 0.55 * (t * syllable).sin().abs());
            let sample = (t * tone * std::f64::consts::TAU).sin() * 0.3 * loudness;
            (sample * f64::from(i16::MAX)) as i16
        })
        .collect()
}

/// The 64 peaks of a set of samples, the way the app makes them (`web/src/voice/waveform.ts`): a bar is the plain RMS of its share of the
/// samples, the loudest bar is 255, and a mild curve (`** 0.85`) is applied after that.
fn peaks_of(samples: &[i16]) -> Vec<u8> {
    let bars = 64;
    let per_bar = samples.len().div_ceil(bars).max(1);
    let levels: Vec<f64> = samples
        .chunks(per_bar)
        .map(|chunk| {
            let sum: f64 = chunk
                .iter()
                .map(|s| (f64::from(*s) / f64::from(i16::MAX)).powi(2))
                .sum();
            (sum / chunk.len() as f64).sqrt()
        })
        .collect();
    let loudest = levels.iter().copied().fold(f64::MIN_POSITIVE, f64::max);
    let mut peaks: Vec<u8> = levels
        .iter()
        .map(|level| ((level / loudest).powf(0.85) * 255.0).round() as u8)
        .collect();
    peaks.resize(bars, 0);
    peaks
}

fn wav_of(samples: &[i16]) -> Vec<u8> {
    let data: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
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
