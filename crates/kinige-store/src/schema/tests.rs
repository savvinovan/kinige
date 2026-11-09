//! A shipped migration never changes (KINIG-103). A user's database that went through migration `n` has exactly the text of
//! `MIGRATION_n` as it was on that day; an edit made later would give a fresh database a different schema from an upgraded one,
//! and nothing would say so. The SHA-256 of every entry of `MIGRATIONS` is pinned here.
//!
//! The pins were taken from the tagged releases: migrations 1-6 are byte-identical in v0.5.0 to v0.9.0, 7 appeared in v0.8.0
//! and 8 in v0.9.0, and none of them changed afterwards. (Migration 5 was edited once, on 29.10.2026 in the slice 4a branch,
//! before the first release; the pinned text is the edited one, which is what every release has.)
//!
//! To add a migration, append it to `MIGRATIONS` and append its hash to `PINNED` below; this is the only line that changes.
//! Never edit a pinned one: add a new migration that changes the schema instead.

use super::{LATEST_VERSION, MIGRATIONS};
use kinige_domain::attachment::hex_lower;
use sha2::{Digest, Sha256};

/// `(version, SHA-256 of the migration text as a hex string)`.
const PINNED: [(i64, &str); 8] = [
    (
        1,
        "7a14d665df877c9fd984385e23f1af9541fe940cf6fe250bbe55beb46f1300b2",
    ),
    (
        2,
        "6c22ab497093964233ad5894db8e46a57f6556b352b767f5c73210e0c47400ec",
    ),
    (
        3,
        "0a62ac8b6a7fb87f7db04ec06abb40d157fb32f0dc3293a988efe0e43831bacc",
    ),
    (
        4,
        "6fa95af888a2d500e43aa31abf73636b7d5cbe9240457f31cd55eec4ceacfe69",
    ),
    (
        5,
        "043588d28a22021005b0237fd0c2f19dde4dab5d477a414ecfdaff805e4e02da",
    ),
    (
        6,
        "c0a5decba1ba71a2068a4c43cf65b19a2c985f0302ad683efe410965446a682f",
    ),
    (
        7,
        "819f5491067b8c74f6191a409510ad62a1df24d44692a6bb7612a511b8ef03f2",
    ),
    (
        8,
        "91172be56104e2ea82ad3a8de3f05582c8f7b1753e25c4956f3982883f8d91f8",
    ),
];

#[test]
fn every_migration_has_its_pinned_hash() {
    assert_eq!(
        MIGRATIONS.len(),
        PINNED.len(),
        "a migration was added or removed: append its SHA-256 to `PINNED` (and never change a pinned one)"
    );
    assert_eq!(LATEST_VERSION, PINNED.len() as i64);
    for (index, (sql, (version, pinned))) in MIGRATIONS.iter().zip(PINNED).enumerate() {
        assert_eq!(version, index as i64 + 1, "PINNED is in order");
        let actual = hex_lower(Sha256::digest(sql.as_bytes()).as_slice());
        assert_eq!(
            actual, pinned,
            "migration {version} changed after it shipped: a database that went through the old text no longer matches. \
             Add a new migration instead of editing this one."
        );
    }
}
