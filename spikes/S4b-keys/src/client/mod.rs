//! The client's side of the account flows (ADR-0001 §8): what it keeps across a restart (`DeviceStore`, standing for the OS secret
//! store and the database), what it keeps only in memory (the password, the words, the old `rc_auth_key`), and the flows in
//! `account.rs` and `recovery.rs`. Every Argon2id run is counted, so a test can prove that a refusal happened before deriving.

mod account;
mod recovery;

use crate::link::Lost;
use crate::log::Log;
use crate::server::{Package, PasswordPart, Token};
use kinige_crypto::keys::{self, Dek, SALT_LEN};
use kinige_crypto::secret::{Password, PwRoot, RcAuthKey};
use kinige_crypto::words::Words;
use kinige_crypto::wrap::wrap_pw;
use kinige_crypto::{AccountId, CryptoError, Profile};

pub use recovery::{PendingRecovery, Resumed};

pub struct Client {
    pub store: DeviceStore,
    log: Log,
    memory: Memory,
    derive_runs: usize,
    /// The profile this client version gives a new account and a new password at least (§8.2). A newer version may raise it.
    default_profile: Profile,
}

impl Client {
    pub fn new(log: Log) -> Client {
        Client::with_default_profile(log, Profile::DEFAULT)
    }

    /// A client of a version whose default profile is `default_profile`: the test of "a newer client raises the profile".
    pub fn with_default_profile(log: Log, default_profile: Profile) -> Client {
        Client {
            store: DeviceStore::default(),
            log,
            memory: Memory::default(),
            derive_runs: 0,
            default_profile,
        }
    }

    /// The process died and started again: the persisted store is all there is; memory and the Argon2id counter start from zero.
    pub fn restart(self) -> Client {
        self.log.event("client restarted");
        Client {
            store: self.store,
            log: self.log,
            memory: Memory::default(),
            derive_runs: 0,
            default_profile: self.default_profile,
        }
    }

    /// How many times Argon2id ran in this process.
    pub fn derive_runs(&self) -> usize {
        self.derive_runs
    }

    /// The words just made, while the process that made them lives (what the screen shows).
    pub fn shown_words(&self) -> Option<&Words> {
        self.memory.words.as_ref()
    }

    /// The user pressed "Save" and the file was written (§A.5: a cancelled dialog does not count).
    pub fn confirm_saved(&mut self) -> Result<(), ClientError> {
        match self
            .store
            .pending
            .as_mut()
            .ok_or(ClientError::NothingPending)?
        {
            Pending::Registration(pending) => pending.saved = true,
            Pending::Recovery(pending) => pending.saved = true,
            Pending::Reissue(pending) => pending.saved = true,
            Pending::PasswordChange => return Err(ClientError::NothingPending),
        }
        self.log.event("client words saved");
        Ok(())
    }

    pub fn account(&self) -> Option<&AccountState> {
        self.store.account.as_ref()
    }

    pub fn dek(&self) -> Option<&Dek> {
        self.store.account.as_ref().map(|account| &account.dek)
    }

    /// `crypto_pwhash` for `password`, counted and logged by profile number only.
    fn derive_root(
        &mut self,
        password: &Password,
        salt: &[u8; SALT_LEN],
        profile: Profile,
    ) -> Result<PwRoot, ClientError> {
        self.derive_runs += 1;
        self.log
            .event(format!("client argon2id profile {}", profile.number()));
        Ok(keys::derive_pw_root(password, salt, profile)?)
    }

    /// A new password's part: a new `salt`, the keys derived from it and `wrap_pw` under them (§8.5).
    fn password_part(
        &mut self,
        password: &Password,
        dek: &Dek,
        account: &AccountId,
        profile: Profile,
    ) -> Result<PasswordPart, ClientError> {
        let salt = random_array::<SALT_LEN>();
        let root = self.derive_root(password, &salt, profile)?;
        let auth_key = keys::pw_auth_key(&root)?;
        let kek = keys::pw_kek(&root)?;
        let wrap = wrap_pw(dek, &kek, account, &salt, profile)?;
        Ok(PasswordPart {
            salt,
            profile: profile.number(),
            auth_key,
            wrap_pw: wrap,
        })
    }

    fn set_account(
        &mut self,
        account_id: AccountId,
        login: String,
        dek: Dek,
        token: Token,
        profile: Profile,
    ) {
        self.store.kdf_profile_seen = Some(
            self.store
                .kdf_profile_seen
                .map_or(profile, |seen| seen.max(profile)),
        );
        self.store.account = Some(AccountState {
            account_id,
            login,
            dek,
            token: Some(token),
        });
        self.store.pending = None;
        self.memory = Memory::default();
    }
}

/// Lost when the process dies.
#[derive(Default)]
struct Memory {
    /// The words just made, for the one screen that shows them.
    words: Option<Words>,
    /// The old `rc_auth_key` of a recovery, kept until step 4 (§8.8).
    old_rc_auth_key: Option<RcAuthKey>,
}

/// What survives killing the process: the OS secret store (DEK, token, prepared package) and the device's database
/// (`kdf_profile_seen`, §A.5 "sync_state"). Cloning it is a restart's persisted state.
#[derive(Clone, Default)]
pub struct DeviceStore {
    pub kdf_profile_seen: Option<Profile>,
    pub account: Option<AccountState>,
    pub pending: Option<Pending>,
}

#[derive(Clone)]
pub struct AccountState {
    pub account_id: AccountId,
    pub login: String,
    pub dek: Dek,
    pub token: Option<Token>,
}

/// A prepared package saved **before** its words are shown and repeated until a definite answer (§A.5), or the mark of a password
/// change whose answer was lost (§8.5).
#[derive(Clone)]
pub enum Pending {
    Registration(PendingRegistration),
    Recovery(PendingRecovery),
    Reissue(PendingReissue),
    /// A password change was sent and no answer came. The new password is not kept; it settles the question by logging in (§8.5).
    PasswordChange,
}

#[derive(Clone)]
pub struct PendingRegistration {
    pub package: Package,
    pub dek: Dek,
    pub saved: bool,
}

#[derive(Clone)]
pub struct PendingReissue {
    pub new_rc_auth_key: RcAuthKey,
    pub wrap_rc: Vec<u8>,
    pub saved: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientError {
    /// No answer: the request or its reply was lost. The prepared package stays, and the same one is sent again.
    Lost,
    /// A request would have gone out before "Save" was confirmed (§8.3, §8.6 item 3).
    SaveNotConfirmed,
    /// A refusal by the profile rule (§8.2), before anything was derived.
    Profile(CryptoError),
    Crypto(CryptoError),
    /// "No such login" or "wrong key": the server does not say which (§8.6).
    Auth,
    LoginTaken,
    AccountIdTaken,
    /// A different package already behind the same new `rc_auth_key` (§A.5).
    PackageMismatch,
    NothingPending,
    NotLoggedIn,
    Invalid,
}

impl From<Lost> for ClientError {
    fn from(_: Lost) -> ClientError {
        ClientError::Lost
    }
}

impl From<CryptoError> for ClientError {
    fn from(error: CryptoError) -> ClientError {
        ClientError::Crypto(error)
    }
}

pub(crate) fn random_array<const N: usize>() -> [u8; N] {
    let mut bytes = [0u8; N];
    libsodium_rs::random::fill_bytes(&mut bytes);
    bytes
}

/// 16 random bytes with the version and variant bits of a v4 UUID.
pub(crate) fn random_uuid_v4() -> AccountId {
    let mut bytes = random_array::<16>();
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    AccountId::from_bytes(bytes)
}
