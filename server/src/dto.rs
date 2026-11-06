//! The JSON bodies of the HTTP API (ADR-0001 appendix §A.6). Keys, salts, wraps and tokens are base64url without padding in strings;
//! a generation is a decimal string (it is a random 64-bit number, and JSON numbers are not safe above 2^53); `seq`, `epoch` and
//! versions are numbers. Both sides of the wire use these types: the server parses them, the tests and the S4a adapter build them.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginParamsRequest {
    pub login: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginParamsResponse {
    pub salt: String,
    pub profile: u8,
    pub cred_version: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub account_id: String,
    pub login: String,
    pub salt: String,
    pub profile: u8,
    pub auth_key: String,
    pub rc_auth_key: String,
    pub wrap_pw: String,
    pub wrap_rc: String,
    pub key_id: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenResponse {
    pub token: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginRequest {
    pub login: String,
    pub auth_key: String,
    /// The token this login replaces: it is revoked if it belongs to the same account (ADR-0018, "S4c, sessions").
    #[serde(default)]
    pub replaces: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginResponse {
    pub token: String,
    pub account_id: String,
    pub key_id: u32,
    pub wrap_pw: String,
    pub cred_version: u64,
}

/// A new password: salt, profile, `auth_key` and `wrap_pw` (§8.5).
#[derive(Debug, Serialize, Deserialize)]
pub struct PasswordPart {
    pub salt: String,
    pub profile: u8,
    pub auth_key: String,
    pub wrap_pw: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChangePasswordRequest {
    /// The `auth_key` of the old password.
    pub auth_key: String,
    /// The credential version the old key was derived against.
    pub expected_version: u64,
    pub new: PasswordPart,
    #[serde(default)]
    pub revoke_others: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CredVersionResponse {
    pub cred_version: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RecoveryFetchRequest {
    pub login: String,
    pub rc_auth_key: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RecoveryFetchResponse {
    pub account_id: String,
    pub key_id: u32,
    pub wrap_rc: String,
    pub profile: u8,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RecoveryRequest {
    pub login: String,
    /// The `rc_auth_key` of the words being replaced.
    pub old_rc_auth_key: String,
    pub new: PasswordPart,
    pub rc_auth_key: String,
    pub wrap_rc: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RecoveryResponse {
    pub token: String,
    pub cred_version: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReissueRequest {
    pub auth_key: String,
    pub new_rc_auth_key: String,
    pub wrap_rc: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PullRequest {
    pub generation: String,
    pub after: u64,
    pub known_epoch: u64,
    #[serde(default)]
    pub limit: Option<usize>,
}

/// The server's answer to a pull (§6.1, §6.3).
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum PullResponse {
    Page {
        generation: String,
        /// The cursor this page was asked from, echoed so that a delayed answer can be recognised.
        after: u64,
        /// Where the cursor goes after this page: the last `seq` of a truncated page, otherwise the head.
        next: u64,
        head: u64,
        epoch: u64,
        items: Vec<PulledEnvelope>,
    },
    /// Something that matters was dropped above the cursor (§6.3).
    Stale { generation: String, epoch: u64 },
    GenerationChanged {
        generation: String,
        stale: bool,
        epoch: u64,
    },
}

/// An envelope with the `entity`, `id` and `op` the server declares for it (ADR-0018 finding 2): the client checks them against the
/// authentic header.
#[derive(Debug, Serialize, Deserialize)]
pub struct PulledEnvelope {
    pub seq: u64,
    pub entity: String,
    pub id: String,
    pub op: String,
    pub envelope: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PushResponse {
    pub results: Vec<PushResult>,
}

/// One answer per `op_id`, in the order of the request (appendix §A.3).
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PushResult {
    pub op: String,
    pub result: PushOutcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seq: Option<u64>,
    /// With `ok` for a repeated `purged`: the snapshot of the `purged` the server accepted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub purged_of: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PushOutcome {
    Ok,
    Duplicate,
    OpConflict,
    Purged,
    StalePurge,
    ClockSkew,
    TooOld,
    Stale,
}
