//! An in-memory account server (ADR-0001 §8.3-§8.6, §9, appendix §A.5). It stores only what §9 says it may: `generichash` of the two
//! auth keys, the wraps, the salt and the profile; never a password, a KEK or a DEK. Idempotency, atomic replacement and the
//! indistinguishable "no such login / wrong key" answers are the rules under test.

use crate::log::Log;
use kinige_crypto::AccountId;
use kinige_crypto::envelope::peek;
use kinige_crypto::ids::{
    AUTH_HASH_LEN, auth_hash, normalize_login, unknown_login_salt, verify_auth,
};
use kinige_crypto::keys::SALT_LEN;
use kinige_crypto::secret::{AuthKey, RcAuthKey, ServerSecret};
use kinige_crypto::wrap::{WRAP_LEN, WRAP_VERSION};
use std::collections::{BTreeMap, BTreeSet};

pub use crate::wire::{
    AuthFailed, LoginOk, Package, Params, PasswordPart, RecoverError, Recovery, RecoveryFetch,
    RegisterError, Session, SessionError, StoredView, Token,
};

/// The profile a login without an account is answered with (§8.4).
const DEFAULT_PROFILE: u8 = 1;

pub struct Server {
    accounts: BTreeMap<[u8; 16], Stored>,
    by_login: BTreeMap<String, [u8; 16]>,
    secret: ServerSecret,
    envelopes: BTreeMap<[u8; 16], Vec<Vec<u8>>>,
    log: Log,
}

struct Stored {
    login: String,
    salt: [u8; SALT_LEN],
    profile: u8,
    auth_hash: [u8; AUTH_HASH_LEN],
    rc_hash: [u8; AUTH_HASH_LEN],
    key_id: u32,
    wrap_pw: Vec<u8>,
    wrap_rc: Vec<u8>,
    sessions: BTreeSet<[u8; AUTH_HASH_LEN]>,
    registered: Package,
    last_recovery: Option<Recovery>,
    last_reissue: Option<(RcAuthKey, Vec<u8>)>,
}

impl Server {
    pub fn new(secret: ServerSecret, log: Log) -> Server {
        Server {
            accounts: BTreeMap::new(),
            by_login: BTreeMap::new(),
            secret,
            envelopes: BTreeMap::new(),
            log,
        }
    }

    /// Registration (§8.3), idempotent by `account_id` (§A.5): the same package again is a success, `account_id` taken by another
    /// package is its own error, and only then is the login looked at, so a repeat after a lost answer never meets "login taken".
    pub fn register(&mut self, package: &Package) -> Result<Session, RegisterError> {
        let login = normalize_login(&package.login).ok_or(RegisterError::Invalid)?;
        if !valid_wrap(&package.wrap_pw)
            || !valid_wrap(&package.wrap_rc)
            || !valid_profile(package.profile)
        {
            return Err(RegisterError::Invalid);
        }
        if let Some(existing) = self.accounts.get(package.account_id.as_bytes()) {
            if !same_package(&existing.registered, package) {
                self.log.event("server register refused: account id taken");
                return Err(RegisterError::AccountIdTaken);
            }
            self.log.event("server register repeated: same package");
            return Ok(self.open_session(package.account_id));
        }
        if self.by_login.contains_key(&login) {
            self.log.event("server register refused: login taken");
            return Err(RegisterError::LoginTaken);
        }
        let stored = Stored {
            login: login.clone(),
            salt: package.salt,
            profile: package.profile,
            auth_hash: auth_hash(package.auth_key.as_bytes())
                .map_err(|_| RegisterError::Invalid)?,
            rc_hash: auth_hash(package.rc_auth_key.as_bytes())
                .map_err(|_| RegisterError::Invalid)?,
            key_id: package.key_id,
            wrap_pw: package.wrap_pw.clone(),
            wrap_rc: package.wrap_rc.clone(),
            sessions: BTreeSet::new(),
            registered: package.clone(),
            last_recovery: None,
            last_reissue: None,
        };
        self.accounts.insert(*package.account_id.as_bytes(), stored);
        self.by_login.insert(login, *package.account_id.as_bytes());
        self.log.event("server register ok");
        Ok(self.open_session(package.account_id))
    }

    /// The salt and profile for a login (§8.4 step 1). For an unknown login: a 16-byte salt from `server_secret` and the default
    /// profile, so the answer has the same shape and repeats. An account with profile 2 differs from that: accepted in §8.4.
    pub fn login_params(&mut self, login: &str) -> Params {
        let login = normalize_login(login).unwrap_or_default();
        match self
            .by_login
            .get(&login)
            .and_then(|id| self.accounts.get(id))
        {
            Some(stored) => Params {
                salt: stored.salt,
                profile: stored.profile,
            },
            None => Params {
                salt: unknown_login_salt(&self.secret, &login).unwrap_or([0; SALT_LEN]),
                profile: DEFAULT_PROFILE,
            },
        }
    }

    /// Login by `auth_key` (§8.4 steps 2-3).
    pub fn login(&mut self, login: &str, auth_key: &AuthKey) -> Result<LoginOk, AuthFailed> {
        let login = normalize_login(login).unwrap_or_default();
        let known = self.by_login.get(&login).copied().filter(|id| {
            self.accounts.get(id).is_some_and(|stored| {
                verify_auth(&stored.auth_hash, auth_key.as_bytes()).unwrap_or(false)
            })
        });
        let Some(id) = known else {
            self.log.event("server login failed");
            return Err(AuthFailed);
        };
        let token = Token::random();
        let stored = self.accounts.get_mut(&id).ok_or(AuthFailed)?;
        stored.sessions.insert(token_hash(&token));
        self.log.event("server login ok");
        Ok(LoginOk {
            token,
            account_id: AccountId::from_bytes(id),
            key_id: stored.key_id,
            wrap_pw: stored.wrap_pw.clone(),
        })
    }

    /// A password change (§8.5): with a session and the old `auth_key`; salt, profile, `auth_key` and `wrap_pw` are replaced
    /// together. `revoke_others` ends every other session.
    pub fn change_password(
        &mut self,
        token: &Token,
        old: &AuthKey,
        new: &PasswordPart,
        revoke_others: bool,
    ) -> Result<(), SessionError> {
        if !valid_wrap(&new.wrap_pw) || !valid_profile(new.profile) {
            return Err(SessionError::Invalid);
        }
        let id = self.account_of(token, old)?;
        let new_hash = auth_hash(new.auth_key.as_bytes()).map_err(|_| SessionError::Invalid)?;
        let stored = self
            .accounts
            .get_mut(&id)
            .ok_or(SessionError::Unauthorized)?;
        stored.salt = new.salt;
        stored.profile = new.profile;
        stored.auth_hash = new_hash;
        stored.wrap_pw = new.wrap_pw.clone();
        if revoke_others {
            stored.sessions.retain(|hash| *hash == token_hash(token));
        }
        self.log.event("server password changed");
        Ok(())
    }

    /// Step 1 of a recovery (§8.6): the account's `wrap_rc`, for a valid `rc_auth_key`, without a session.
    pub fn recovery_fetch(
        &mut self,
        login: &str,
        rc_auth_key: &RcAuthKey,
    ) -> Result<RecoveryFetch, AuthFailed> {
        let id = self.account_of_rc(login, rc_auth_key).ok_or(AuthFailed)?;
        let stored = self.accounts.get(&id).ok_or(AuthFailed)?;
        self.log.event("server recovery fetch ok");
        Ok(RecoveryFetch {
            account_id: AccountId::from_bytes(id),
            key_id: stored.key_id,
            wrap_rc: stored.wrap_rc.clone(),
            profile: stored.profile,
        })
    }

    /// Step 4 of a recovery (§8.6): one atomic replacement authorized by the old `rc_auth_key`, which ends every session. Idempotent by
    /// the new `rc_auth_key` **and** equality of the rest of the package (§A.5): the same package again is a success, another package
    /// behind an `rc_auth_key` already applied is a separate error.
    pub fn recover(
        &mut self,
        login: &str,
        old: &RcAuthKey,
        recovery: &Recovery,
    ) -> Result<Session, RecoverError> {
        if !valid_wrap(&recovery.password.wrap_pw)
            || !valid_wrap(&recovery.wrap_rc)
            || !valid_profile(recovery.password.profile)
        {
            return Err(RecoverError::Invalid);
        }
        let by_login = normalize_login(login).and_then(|login| self.by_login.get(&login).copied());
        let new_hash =
            auth_hash(recovery.rc_auth_key.as_bytes()).map_err(|_| RecoverError::Invalid)?;
        if let Some(stored) = by_login
            .and_then(|id| self.accounts.get(&id))
            .filter(|stored| stored.rc_hash == new_hash)
        {
            return match &stored.last_recovery {
                Some(done) if same_recovery(done, recovery) => {
                    self.log.event("server recovery repeated: same package");
                    Ok(self.open_session(AccountId::from_bytes(by_login.unwrap_or_default())))
                }
                _ => {
                    self.log
                        .event("server recovery refused: key already applied by another package");
                    Err(RecoverError::PackageMismatch)
                }
            };
        }
        let id = self
            .account_of_rc(login, old)
            .ok_or(RecoverError::AuthFailed)?;
        let new_password_hash =
            auth_hash(recovery.password.auth_key.as_bytes()).map_err(|_| RecoverError::Invalid)?;
        let stored = self.accounts.get_mut(&id).ok_or(RecoverError::AuthFailed)?;
        stored.salt = recovery.password.salt;
        stored.profile = recovery.password.profile;
        stored.auth_hash = new_password_hash;
        stored.wrap_pw = recovery.password.wrap_pw.clone();
        stored.rc_hash = new_hash;
        stored.wrap_rc = recovery.wrap_rc.clone();
        stored.sessions.clear();
        stored.last_recovery = Some(recovery.clone());
        self.log.event("server recovery ok: sessions revoked");
        Ok(self.open_session(AccountId::from_bytes(id)))
    }

    /// New words (§8.6, "Reissuing the words"): with a session and the password's `auth_key`; idempotent by the new `rc_auth_key`.
    pub fn reissue_words(
        &mut self,
        token: &Token,
        auth_key: &AuthKey,
        new_rc: &RcAuthKey,
        wrap_rc: &[u8],
    ) -> Result<(), SessionError> {
        if !valid_wrap(wrap_rc) {
            return Err(SessionError::Invalid);
        }
        let id = self.account_of(token, auth_key)?;
        let new_hash = auth_hash(new_rc.as_bytes()).map_err(|_| SessionError::Invalid)?;
        let stored = self
            .accounts
            .get_mut(&id)
            .ok_or(SessionError::Unauthorized)?;
        if stored.rc_hash == new_hash {
            let same = stored
                .last_reissue
                .as_ref()
                .is_some_and(|(key, wrap)| key.ct_eq(new_rc) && wrap == wrap_rc);
            return if same {
                Ok(())
            } else {
                Err(SessionError::Invalid)
            };
        }
        stored.rc_hash = new_hash;
        stored.wrap_rc = wrap_rc.to_vec();
        stored.last_reissue = Some((new_rc.clone(), wrap_rc.to_vec()));
        self.log.event("server words reissued");
        Ok(())
    }

    /// Stores an envelope's bytes for the session's account: the server reads only the lengths and the open header.
    pub fn push_envelope(&mut self, token: &Token, bytes: &[u8]) -> Result<(), SessionError> {
        peek(bytes).map_err(|_| SessionError::Invalid)?;
        let id = self
            .account_of_token(token)
            .ok_or(SessionError::Unauthorized)?;
        self.envelopes.entry(id).or_default().push(bytes.to_vec());
        Ok(())
    }

    pub fn pull_envelopes(&self, token: &Token) -> Result<Vec<Vec<u8>>, SessionError> {
        let id = self
            .account_of_token(token)
            .ok_or(SessionError::Unauthorized)?;
        Ok(self.envelopes.get(&id).cloned().unwrap_or_default())
    }

    /// For tests: how many sessions the account has.
    pub fn session_count(&self, account: &AccountId) -> usize {
        self.accounts
            .get(account.as_bytes())
            .map_or(0, |stored| stored.sessions.len())
    }

    /// For tests: what the server holds of an account, to prove it holds no secret. `(salt, profile, auth hash, rc hash)`.
    pub fn stored_view(&self, account: &AccountId) -> Option<StoredView> {
        self.accounts.get(account.as_bytes()).map(|s| StoredView {
            salt: s.salt,
            profile: s.profile,
            auth_hash: s.auth_hash,
            rc_hash: s.rc_hash,
        })
    }

    /// Test hook: a malicious server sends another profile without any password change (§8.2).
    pub fn tamper_profile(&mut self, account: &AccountId, profile: u8) {
        if let Some(stored) = self.accounts.get_mut(account.as_bytes()) {
            stored.profile = profile;
        }
    }

    pub fn login_of(&self, account: &AccountId) -> Option<String> {
        self.accounts
            .get(account.as_bytes())
            .map(|stored| stored.login.clone())
    }

    /// A new session for an account the caller has just found or made.
    fn open_session(&mut self, account: AccountId) -> Session {
        let token = Token::random();
        if let Some(stored) = self.accounts.get_mut(account.as_bytes()) {
            stored.sessions.insert(token_hash(&token));
        }
        Session { token }
    }

    fn account_of_token(&self, token: &Token) -> Option<[u8; 16]> {
        let hash = token_hash(token);
        self.accounts
            .iter()
            .find(|(_, stored)| stored.sessions.contains(&hash))
            .map(|(id, _)| *id)
    }

    /// A valid session whose account also accepts the `auth_key` (the password is proven again for a change).
    fn account_of(&self, token: &Token, auth_key: &AuthKey) -> Result<[u8; 16], SessionError> {
        let id = self
            .account_of_token(token)
            .ok_or(SessionError::Unauthorized)?;
        let stored = self.accounts.get(&id).ok_or(SessionError::Unauthorized)?;
        if verify_auth(&stored.auth_hash, auth_key.as_bytes()).unwrap_or(false) {
            Ok(id)
        } else {
            Err(SessionError::Unauthorized)
        }
    }

    fn account_of_rc(&self, login: &str, rc: &RcAuthKey) -> Option<[u8; 16]> {
        let id = *self.by_login.get(&normalize_login(login)?)?;
        let stored = self.accounts.get(&id)?;
        verify_auth(&stored.rc_hash, rc.as_bytes())
            .unwrap_or(false)
            .then_some(id)
    }
}

fn valid_wrap(wrap: &[u8]) -> bool {
    wrap.len() == WRAP_LEN && wrap.first() == Some(&WRAP_VERSION)
}

fn valid_profile(profile: u8) -> bool {
    matches!(profile, 1 | 2)
}

fn token_hash(token: &Token) -> [u8; AUTH_HASH_LEN] {
    auth_hash(token.as_bytes()).unwrap_or([0; AUTH_HASH_LEN])
}

fn same_package(a: &Package, b: &Package) -> bool {
    a.account_id == b.account_id
        && a.login == b.login
        && a.salt == b.salt
        && a.profile == b.profile
        && a.auth_key.ct_eq(&b.auth_key)
        && a.rc_auth_key.ct_eq(&b.rc_auth_key)
        && a.wrap_pw == b.wrap_pw
        && a.wrap_rc == b.wrap_rc
        && a.key_id == b.key_id
}

fn same_recovery(a: &Recovery, b: &Recovery) -> bool {
    a.password.salt == b.password.salt
        && a.password.profile == b.password.profile
        && a.password.auth_key.ct_eq(&b.password.auth_key)
        && a.password.wrap_pw == b.password.wrap_pw
        && a.rc_auth_key.ct_eq(&b.rc_auth_key)
        && a.wrap_rc == b.wrap_rc
}
