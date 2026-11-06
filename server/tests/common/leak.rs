//! A scanner for secrets and plain text in a database dump and a log (ADR-0001 §8.8, PRD §14 S4c): it looks for a value's bytes in every
//! encoding the code could print, not only one. Ported from `spikes/S4b-keys/src/leak.rs`. A value is found if its whole self, or its
//! first 6 bytes (a clean prefix in every encoding below), appear.

use libsodium_rs::utils::{
    self, BASE64_VARIANT_ORIGINAL, BASE64_VARIANT_ORIGINAL_NO_PADDING, BASE64_VARIANT_URLSAFE,
    BASE64_VARIANT_URLSAFE_NO_PADDING,
};

/// A named value to look for.
pub struct Secret {
    pub name: &'static str,
    pub bytes: Vec<u8>,
}

impl Secret {
    pub fn new(name: &'static str, bytes: &[u8]) -> Secret {
        Secret {
            name,
            bytes: bytes.to_vec(),
        }
    }
}

/// The names of the secrets that appear in `text`, in any encoding of [`encodings`].
pub fn find_leaks(text: &str, secrets: &[Secret]) -> Vec<&'static str> {
    secrets
        .iter()
        .filter(|secret| {
            let prefix = secret.bytes.get(..6).unwrap_or(&secret.bytes);
            [secret.bytes.as_slice(), prefix].into_iter().any(|part| {
                !part.is_empty()
                    && encodings(part)
                        .iter()
                        .any(|form| text.contains(form.as_str()))
            })
        })
        .map(|secret| secret.name)
        .collect()
}

/// Every text form of `bytes` that code might print: hex in both cases, four base64 variants, the decimal list as `{:?}` prints it,
/// the same without spaces and with spaces only, and the text itself when it is UTF-8.
pub fn encodings(bytes: &[u8]) -> Vec<String> {
    let decimals: Vec<String> = bytes.iter().map(|byte| byte.to_string()).collect();
    let mut forms = vec![
        utils::bin2hex(bytes),
        utils::bin2hex(bytes).to_uppercase(),
        format!("[{}]", decimals.join(", ")),
        decimals.join(","),
        decimals.join(" "),
    ];
    for variant in [
        BASE64_VARIANT_ORIGINAL,
        BASE64_VARIANT_ORIGINAL_NO_PADDING,
        BASE64_VARIANT_URLSAFE,
        BASE64_VARIANT_URLSAFE_NO_PADDING,
    ] {
        let form = utils::bin2base64(bytes, variant);
        // Padding and a partial last group make the end of a prefix differ; the leading full groups are what matters.
        let stable = form.trim_end_matches('=');
        forms.push(
            stable
                .get(..stable.len() / 4 * 4)
                .unwrap_or(stable)
                .to_string(),
        );
    }
    if let Ok(text) = std::str::from_utf8(bytes) {
        forms.push(text.to_string());
    }
    forms.retain(|form| form.len() >= 6);
    forms
}
