//! Configuration. [`Config`] is the behaviour of the server (limits, rate-limit numbers) with fixed defaults that the README records;
//! [`Settings`] is what the process is told by its environment (database, address, secret file). The real numbers are slice 11's.

use crate::rate::Rule;
use kinige_crypto::secret::ServerSecret;
use std::fmt;

/// 16 MiB: a batch of envelopes, each at most a little over 1 MiB (`MAX_ENVELOPE`).
pub const MAX_PUSH_BODY: usize = 16 * 1024 * 1024;
/// Envelopes per push request (ADR-0001 §6.2 step 4 measured 1000 per request).
pub const MAX_BATCH: usize = 1000;
pub const PAGE_DEFAULT: usize = 200;
pub const PAGE_MAX: usize = 1000;
/// A pull page stops adding envelopes once this many bytes are in it (the first one is always sent).
pub const PAGE_BYTES: usize = 8 * 1024 * 1024;
/// The HTTP protocol version the server speaks (§4); a push from a client below `min_protocol` gets `too_old`.
pub const PROTOCOL_VERSION: u32 = 1;
/// The smallest body of an account request that is judged: 16 KiB covers the largest (a recovery with two wraps) many times over.
pub const MAX_JSON_BODY: usize = 16 * 1024;
const MINUTE_MS: u64 = 60_000;

/// What the server does, with the defaults of the README. Tests change single fields.
#[derive(Debug, Clone)]
pub struct Config {
    pub max_push_body: usize,
    pub max_batch: usize,
    pub page_default: usize,
    pub page_max: usize,
    pub page_bytes: usize,
    /// A push below this protocol version is answered `too_old` per `op_id` (§A.3).
    pub min_protocol: u32,
    /// Login attempts: per normalized login and per client address, in a fixed window.
    pub login_by_login: Rule,
    pub login_by_addr: Rule,
    /// The word checks: the recovery fetch and the recovery itself share one budget.
    pub recovery_by_login: Rule,
    pub recovery_by_addr: Rule,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            max_push_body: MAX_PUSH_BODY,
            max_batch: MAX_BATCH,
            page_default: PAGE_DEFAULT,
            page_max: PAGE_MAX,
            page_bytes: PAGE_BYTES,
            min_protocol: PROTOCOL_VERSION,
            login_by_login: Rule::new(10, MINUTE_MS),
            login_by_addr: Rule::new(60, MINUTE_MS),
            recovery_by_login: Rule::new(5, MINUTE_MS),
            recovery_by_addr: Rule::new(30, MINUTE_MS),
        }
    }
}

/// What the process reads from its environment (README, "Configuration").
pub struct Settings {
    pub database_url: String,
    pub listen: String,
    pub secret: ServerSecret,
    /// How often compaction runs, seconds.
    pub compact_every_secs: u64,
    /// The name of a request header a trusted reverse proxy sets to the client's address; none by default.
    pub client_addr_header: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum SettingsError {
    Missing(&'static str),
    Invalid(&'static str),
}

impl fmt::Display for SettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SettingsError::Missing(name) => write!(f, "{name} is not set"),
            SettingsError::Invalid(name) => write!(f, "{name} is not valid"),
        }
    }
}

impl Settings {
    /// `lookup` reads an environment variable; `read_file` reads a file's text (the secret). Injected so the parsing is testable.
    pub fn from_lookup(
        lookup: impl Fn(&str) -> Option<String>,
        read_file: impl Fn(&str) -> Option<String>,
    ) -> Result<Settings, SettingsError> {
        let database_url =
            lookup("KINIGE_DATABASE_URL").ok_or(SettingsError::Missing("KINIGE_DATABASE_URL"))?;
        let listen = lookup("KINIGE_LISTEN").unwrap_or_else(|| "127.0.0.1:8080".to_string());
        let path = lookup("KINIGE_SERVER_SECRET_FILE")
            .ok_or(SettingsError::Missing("KINIGE_SERVER_SECRET_FILE"))?;
        let secret = read_file(&path)
            .and_then(|text| parse_secret(&text))
            .ok_or(SettingsError::Invalid("KINIGE_SERVER_SECRET_FILE"))?;
        let compact_every_secs = match lookup("KINIGE_COMPACT_EVERY_SECS") {
            None => 3600,
            Some(text) => text
                .parse::<u64>()
                .ok()
                .filter(|secs| *secs > 0)
                .ok_or(SettingsError::Invalid("KINIGE_COMPACT_EVERY_SECS"))?,
        };
        let client_addr_header =
            lookup("KINIGE_CLIENT_ADDR_HEADER").filter(|name| !name.is_empty());
        Ok(Settings {
            database_url,
            listen,
            secret,
            compact_every_secs,
            client_addr_header,
        })
    }
}

/// The secret file holds 64 hex characters (32 bytes, `openssl rand -hex 32`), optionally followed by a newline.
fn parse_secret(text: &str) -> Option<ServerSecret> {
    let hex = text.trim();
    // `from_str_radix` accepts a leading `+`, so the characters are checked first.
    if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let mut bytes = [0u8; 32];
    for (slot, pair) in bytes.iter_mut().zip(hex.as_bytes().chunks(2)) {
        let pair = std::str::from_utf8(pair).ok()?;
        *slot = u8::from_str_radix(pair, 16).ok()?;
    }
    Some(ServerSecret::from_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEX: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";

    fn env(name: &str) -> Option<String> {
        match name {
            "KINIGE_DATABASE_URL" => Some("postgres://fake@localhost/fake".to_string()),
            "KINIGE_SERVER_SECRET_FILE" => Some("/fake/secret".to_string()),
            _ => None,
        }
    }

    #[test]
    fn defaults_apply_and_the_secret_is_read_from_the_file() {
        let settings = Settings::from_lookup(env, |_| Some(format!("{HEX}\n"))).expect("settings");
        assert_eq!(settings.listen, "127.0.0.1:8080");
        assert_eq!(settings.compact_every_secs, 3600);
        assert!(settings.client_addr_header.is_none());
    }

    #[test]
    fn a_missing_variable_or_a_bad_secret_is_refused_by_name() {
        let no_url = |name: &str| env(name).filter(|_| name != "KINIGE_DATABASE_URL");
        assert_eq!(
            Settings::from_lookup(no_url, |_| Some(HEX.to_string())).err(),
            Some(SettingsError::Missing("KINIGE_DATABASE_URL"))
        );
        for bad in ["", "abc", &"zz".repeat(32), &format!("{HEX}00")] {
            assert_eq!(
                Settings::from_lookup(env, |_| Some(bad.to_string())).err(),
                Some(SettingsError::Invalid("KINIGE_SERVER_SECRET_FILE")),
                "{bad:?}"
            );
        }
        assert!(Settings::from_lookup(env, |_| None).is_err());
    }

    #[test]
    fn the_compaction_period_must_be_a_positive_number() {
        for bad in ["0", "-1", "soon", ""] {
            let lookup = |name: &str| match name {
                "KINIGE_COMPACT_EVERY_SECS" => Some(bad.to_string()),
                other => env(other),
            };
            assert!(Settings::from_lookup(lookup, |_| Some(HEX.to_string())).is_err());
        }
    }
}
