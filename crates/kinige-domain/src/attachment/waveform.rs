//! The waveform of a voice note (slice 7a, ADR-0015): a fixed number of peaks, local to this device. It is cosmetic data about a file
//! that is already stored: it is never part of the synced `core` group of an attachment (ADR-0001 appendix §C.6), so a damaged or
//! missing waveform can never lose or change a note. No IO.

/// How many peaks a waveform has. The player draws exactly this many bars.
pub const WAVEFORM_PEAKS: usize = 64;

/// The most a waveform travels as text (a header, a command argument): two hex digits per peak.
pub const WAVEFORM_HEX_LEN: usize = WAVEFORM_PEAKS * 2;

/// What can be stored as a waveform: exactly [`WAVEFORM_PEAKS`] bytes (0 is silence, 255 the loudest part of the note), or nothing at
/// all, the explicit "the decoder could not read this file, do not try again" marker.
pub fn waveform_is_storable(peaks: &[u8]) -> bool {
    peaks.is_empty() || peaks.len() == WAVEFORM_PEAKS
}

/// Peaks from hex text (either case): [`WAVEFORM_HEX_LEN`] digits, or the empty text for the "failed" marker. Anything else, an odd
/// length, a character that is not a hex digit, the wrong number of peaks, is `None`.
pub fn waveform_from_hex(text: &str) -> Option<Vec<u8>> {
    if text.len() != WAVEFORM_HEX_LEN && !text.is_empty() {
        return None;
    }
    let digit = |byte: u8| {
        char::from(byte)
            .to_digit(16)
            .and_then(|d| u8::try_from(d).ok())
    };
    text.as_bytes()
        .chunks(2)
        .map(|pair| match pair {
            [high, low] => Some((digit(*high)? << 4) | digit(*low)?),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_fixed_count_or_the_empty_marker_is_storable() {
        assert!(waveform_is_storable(&[]));
        assert!(waveform_is_storable(&[7; WAVEFORM_PEAKS]));
        for bad in [1, 2, WAVEFORM_PEAKS - 1, WAVEFORM_PEAKS + 1, 4096] {
            assert!(!waveform_is_storable(&vec![0; bad]), "{bad}");
        }
    }

    #[test]
    fn hex_text_becomes_peaks_in_either_case() {
        let text = "0aFf".repeat(WAVEFORM_PEAKS / 2);
        let peaks = waveform_from_hex(&text).unwrap();
        assert_eq!(peaks.len(), WAVEFORM_PEAKS);
        assert_eq!(peaks.get(..2), Some([0x0a, 0xff].as_slice()));
        assert_eq!(waveform_from_hex(""), Some(Vec::new()));
    }

    #[test]
    fn garbage_hex_is_none_and_never_a_panic() {
        let good = "ab".repeat(WAVEFORM_PEAKS);
        assert!(waveform_from_hex(&good).is_some());
        for bad in [
            "a".to_string(),
            "zz".repeat(WAVEFORM_PEAKS),
            format!("{good}00"),
            good.get(..WAVEFORM_HEX_LEN - 1).unwrap().to_string(),
            "９".repeat(WAVEFORM_PEAKS),
            " ".repeat(WAVEFORM_HEX_LEN),
            "+1".repeat(WAVEFORM_PEAKS),
            "\u{0}".repeat(WAVEFORM_HEX_LEN),
        ] {
            assert_eq!(waveform_from_hex(&bad), None, "{bad:?}");
        }
    }
}
