//! A scanner for secrets in logs (appendix §B item 10): looks for a secret's bytes in every encoding the code could print, not only
//! in one. A secret is found if its whole value, or its first 6 bytes (a clean prefix in every encoding below), appear.

use libsodium_rs::utils::{
    self, BASE64_VARIANT_ORIGINAL, BASE64_VARIANT_ORIGINAL_NO_PADDING, BASE64_VARIANT_URLSAFE,
    BASE64_VARIANT_URLSAFE_NO_PADDING,
};

/// A named secret to look for.
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

/// The names of the secrets that appear in `log`, in any encoding of [`encodings`].
pub fn find_leaks(log: &str, secrets: &[Secret]) -> Vec<&'static str> {
    secrets
        .iter()
        .filter(|secret| {
            let prefix = secret.bytes.get(..6).unwrap_or(&secret.bytes);
            [secret.bytes.as_slice(), prefix].into_iter().any(|part| {
                !part.is_empty()
                    && encodings(part)
                        .iter()
                        .any(|form| log.contains(form.as_str()))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_encoding_of_a_secret_is_found_and_a_clean_log_is_clean() {
        let bytes: Vec<u8> = (0..32).map(|i| (i * 5 + 100) as u8).collect();
        let secret = [Secret::new("key", &bytes)];
        for form in encodings(&bytes) {
            assert_eq!(
                find_leaks(&format!("event {form} end"), &secret),
                vec!["key"],
                "{form}"
            );
        }
        assert_eq!(
            find_leaks(&format!("{:?}", &bytes[..]), &secret),
            vec!["key"],
            "the Debug of a slice"
        );
        assert_eq!(
            find_leaks(&format!("{:x?}", &bytes[..4]), &secret),
            Vec::<&str>::new(),
            "a short hex dump is not matched"
        );
        assert!(find_leaks("register ok account 1111 login failed", &secret).is_empty());
    }

    #[test]
    fn a_six_byte_prefix_is_enough_to_be_found() {
        let bytes: Vec<u8> = (0..32).map(|i| (i * 3 + 7) as u8).collect();
        let secret = [Secret::new("key", &bytes)];
        let prefix_hex = utils::bin2hex(&bytes[..6]);
        assert_eq!(
            find_leaks(&format!("truncated {prefix_hex}..."), &secret),
            vec!["key"]
        );
    }
}
