//! Where blobs live and the values of [`Config`] the operator may change, from the environment (README, "Configuration").

use crate::config::{Config, SettingsError};
use std::path::PathBuf;

/// An S3-compatible store, path-style addressing (the stores of the README all take it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct S3Settings {
    pub bucket: String,
    pub region: String,
    /// `https://host:port`; none means Amazon's own endpoint for the region.
    pub endpoint: Option<String>,
    pub access_key: String,
    pub secret_key: String,
    /// Plain HTTP to the endpoint, for a store on the same private network as the server.
    pub allow_http: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlobSettings {
    /// The blob directory. Objects are in `<dir>/objects` when no S3 store is set; unfinished uploads are always in `<dir>/staging`.
    pub dir: PathBuf,
    pub s3: Option<S3Settings>,
}

impl BlobSettings {
    pub fn objects_dir(&self) -> PathBuf {
        self.dir.join("objects")
    }

    pub fn staging_dir(&self) -> PathBuf {
        self.dir.join("staging")
    }

    /// The blob settings and the `Config` with the operator's overrides applied.
    pub fn from_lookup(
        lookup: &impl Fn(&str) -> Option<String>,
    ) -> Result<(BlobSettings, Config), SettingsError> {
        let dir = lookup("KINIGE_BLOB_DIR").unwrap_or_else(|| "./blobs".to_string());
        let s3 = match lookup("KINIGE_S3_BUCKET").filter(|bucket| !bucket.is_empty()) {
            None => None,
            Some(bucket) => Some(S3Settings {
                bucket,
                region: lookup("KINIGE_S3_REGION").unwrap_or_else(|| "us-east-1".to_string()),
                endpoint: lookup("KINIGE_S3_ENDPOINT").filter(|text| !text.is_empty()),
                access_key: lookup("KINIGE_S3_ACCESS_KEY")
                    .filter(|text| !text.is_empty())
                    .ok_or(SettingsError::Missing("KINIGE_S3_ACCESS_KEY"))?,
                secret_key: lookup("KINIGE_S3_SECRET_KEY")
                    .filter(|text| !text.is_empty())
                    .ok_or(SettingsError::Missing("KINIGE_S3_SECRET_KEY"))?,
                allow_http: flag(lookup("KINIGE_S3_ALLOW_HTTP"), "KINIGE_S3_ALLOW_HTTP")?,
            }),
        };
        let mut config = Config::default();
        if let Some(gib) = number(lookup("KINIGE_BLOB_QUOTA_GIB"), "KINIGE_BLOB_QUOTA_GIB")? {
            config.blob_quota = gib.saturating_mul(1 << 30);
        }
        if let Some(days) = number(
            lookup("KINIGE_BACKUP_RETENTION_DAYS"),
            "KINIGE_BACKUP_RETENTION_DAYS",
        )? {
            config.backup_retention_ms = days.saturating_mul(24 * 60 * 60 * 1000);
        }
        if let Some(hours) = number(
            lookup("KINIGE_BACKUP_INTERVAL_HOURS"),
            "KINIGE_BACKUP_INTERVAL_HOURS",
        )? {
            config.backup_interval_ms = hours.saturating_mul(60 * 60 * 1000);
        }
        Ok((
            BlobSettings {
                dir: PathBuf::from(dir),
                s3,
            },
            config,
        ))
    }
}

/// A positive number, or nothing if the variable is not set.
fn number(text: Option<String>, name: &'static str) -> Result<Option<u64>, SettingsError> {
    match text {
        None => Ok(None),
        Some(text) => text
            .parse::<u64>()
            .ok()
            .filter(|n| *n > 0)
            .map(Some)
            .ok_or(SettingsError::Invalid(name)),
    }
}

fn flag(text: Option<String>, name: &'static str) -> Result<bool, SettingsError> {
    match text.as_deref() {
        None | Some("" | "0" | "false") => Ok(false),
        Some("1" | "true") => Ok(true),
        Some(_) => Err(SettingsError::Invalid(name)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |name| map.get(name).cloned()
    }

    #[test]
    fn the_defaults_are_a_disk_store_and_thirty_days() {
        let (blobs, config) = BlobSettings::from_lookup(&env(&[])).expect("settings");
        assert_eq!(blobs.dir, PathBuf::from("./blobs"));
        assert!(blobs.s3.is_none());
        assert_eq!(config.backup_retention_ms, 30 * 24 * 60 * 60 * 1000);
        assert_eq!(blobs.staging_dir(), PathBuf::from("./blobs/staging"));
    }

    #[test]
    fn an_s3_store_needs_both_keys_and_numbers_must_be_positive() {
        let only_bucket = env(&[("KINIGE_S3_BUCKET", "kinige")]);
        assert_eq!(
            BlobSettings::from_lookup(&only_bucket).err(),
            Some(SettingsError::Missing("KINIGE_S3_ACCESS_KEY"))
        );
        let full = env(&[
            ("KINIGE_S3_BUCKET", "kinige"),
            ("KINIGE_S3_ACCESS_KEY", "fake-access"),
            ("KINIGE_S3_SECRET_KEY", "fake-secret"),
            ("KINIGE_S3_ENDPOINT", "http://s3.example.test:9000"),
            ("KINIGE_S3_ALLOW_HTTP", "1"),
            ("KINIGE_BLOB_QUOTA_GIB", "3"),
        ]);
        let (blobs, config) = BlobSettings::from_lookup(&full).expect("settings");
        assert!(blobs.s3.is_some_and(|s3| s3.allow_http));
        assert_eq!(config.blob_quota, 3 << 30);
        for (name, bad) in [
            ("KINIGE_BLOB_QUOTA_GIB", "0"),
            ("KINIGE_BACKUP_RETENTION_DAYS", "-1"),
            ("KINIGE_BACKUP_INTERVAL_HOURS", "soon"),
            ("KINIGE_S3_ALLOW_HTTP", "maybe"),
        ] {
            let mut pairs = vec![
                ("KINIGE_S3_BUCKET", "kinige"),
                ("KINIGE_S3_ACCESS_KEY", "a"),
                ("KINIGE_S3_SECRET_KEY", "b"),
            ];
            pairs.push((name, bad));
            assert!(BlobSettings::from_lookup(&env(&pairs)).is_err(), "{name}");
        }
    }
}
