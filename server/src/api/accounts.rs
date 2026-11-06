//! The account endpoints (ADR-0001 §8.3-§8.6, appendix §A.5, §A.6). Each handler checks the shape of the request (a 400 that names the
//! rule), takes the rate limit where there is one, and calls the store. Nothing here decides a credential rule.

use super::{ClientAddr, authed, bearer, parse};
use crate::dto::{
    ChangePasswordRequest, CredVersionResponse, LoginParamsRequest, LoginParamsResponse,
    LoginRequest, LoginResponse, PasswordPart, RecoveryFetchRequest, RecoveryFetchResponse,
    RecoveryRequest, RecoveryResponse, RegisterRequest, ReissueRequest, TokenResponse,
};
use crate::error::{ApiError, json};
use crate::rate::Rule;
use crate::state::AppState;
use crate::store::accounts::{self as store, NewPassword, Registration};
use crate::store::recovery::{self, NewRecovery};
use crate::store::sessions::{self, Token};
use crate::wire::{from_b64, key32, to_b64};
use axum::body::Bytes;
use axum::extract::{Extension, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use kinige_crypto::ids::normalize_login;
use kinige_crypto::keys::SALT_LEN;
use kinige_crypto::secret::{AuthKey, RcAuthKey};
use kinige_crypto::wrap::{WRAP_LEN, WRAP_VERSION};
use kinige_domain::hlc::is_uuid;

/// A login is an e-mail address at most; longer is refused before it reaches the database.
const MAX_LOGIN: usize = 254;

pub async fn register(State(app): State<AppState>, body: Bytes) -> Result<Response, ApiError> {
    let request: RegisterRequest = parse(&body)?;
    if !is_uuid(&request.account_id) {
        return Err(ApiError::BadRequest("account_id"));
    }
    if request.key_id == 0 {
        return Err(ApiError::BadRequest("key_id"));
    }
    let registration = Registration {
        login: login_of(&request.login)?,
        account_id: request.account_id,
        salt: salt(&request.salt)?,
        profile: profile(request.profile)?,
        auth_key: AuthKey::from_bytes(key(&request.auth_key, "auth_key")?),
        rc_auth_key: RcAuthKey::from_bytes(key(&request.rc_auth_key, "rc_auth_key")?),
        wrap_pw: wrap(&request.wrap_pw, "wrap_pw")?,
        wrap_rc: wrap(&request.wrap_rc, "wrap_rc")?,
        key_id: request.key_id,
    };
    let token = store::register(&app.pool, app.clock.now_ms(), &registration).await?;
    Ok(json(
        StatusCode::OK,
        &TokenResponse {
            token: token.encode(),
        },
    ))
}

/// The salt and profile for a login. Any login gets an answer of the same shape (§8.4 step 1); there is no rate limit, because the
/// answer says nothing about whether the account exists.
pub async fn login_params(State(app): State<AppState>, body: Bytes) -> Result<Response, ApiError> {
    let request: LoginParamsRequest = parse(&body)?;
    let login = normalize_login(&request.login).unwrap_or_default();
    let params = store::login_params(&app.pool, &app.secret, &login).await?;
    Ok(json(
        StatusCode::OK,
        &LoginParamsResponse {
            salt: to_b64(&params.salt),
            profile: params.profile,
            cred_version: params.cred_version,
        },
    ))
}

pub async fn login(
    State(app): State<AppState>,
    Extension(address): Extension<ClientAddr>,
    body: Bytes,
) -> Result<Response, ApiError> {
    let request: LoginRequest = parse(&body)?;
    let login = normalize_login(&request.login).unwrap_or_default();
    let auth_key = AuthKey::from_bytes(key(&request.auth_key, "auth_key")?);
    let replaces = match &request.replaces {
        Some(text) => Some(Token::parse(text).ok_or(ApiError::BadRequest("replaces"))?),
        None => None,
    };
    let config = &app.config;
    limit(
        &app,
        "login",
        &login,
        &address,
        config.login_by_login,
        config.login_by_addr,
    )?;
    let ok = store::login(
        &app.pool,
        app.clock.now_ms(),
        &login,
        &auth_key,
        replaces.as_ref(),
    )
    .await?;
    Ok(json(
        StatusCode::OK,
        &LoginResponse {
            token: ok.token.encode(),
            account_id: ok.account_id,
            key_id: ok.key_id,
            wrap_pw: to_b64(&ok.wrap_pw),
            cred_version: ok.cred_version,
        },
    ))
}

pub async fn logout(State(app): State<AppState>, headers: HeaderMap) -> Result<Response, ApiError> {
    let token = bearer(&headers).ok_or(ApiError::Auth)?;
    sessions::close(&app.pool, &token).await?;
    Ok(json(StatusCode::OK, &serde_json::json!({})))
}

pub async fn change_password(
    State(app): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ApiError> {
    let (account, token) = authed(&app, &headers).await?;
    let request: ChangePasswordRequest = parse(&body)?;
    let old = AuthKey::from_bytes(key(&request.auth_key, "auth_key")?);
    let new = new_password(&request.new)?;
    let cred_version = store::change_password(
        &app.pool,
        &account,
        &token,
        &old,
        request.expected_version,
        &new,
        request.revoke_others,
    )
    .await?;
    Ok(json(StatusCode::OK, &CredVersionResponse { cred_version }))
}

/// Step 1 of a recovery: the word check. It shares one budget with the recovery itself (§8.4).
pub async fn recovery_fetch(
    State(app): State<AppState>,
    Extension(address): Extension<ClientAddr>,
    body: Bytes,
) -> Result<Response, ApiError> {
    let request: RecoveryFetchRequest = parse(&body)?;
    let login = normalize_login(&request.login).unwrap_or_default();
    let rc = RcAuthKey::from_bytes(key(&request.rc_auth_key, "rc_auth_key")?);
    let config = &app.config;
    limit(
        &app,
        "recovery",
        &login,
        &address,
        config.recovery_by_login,
        config.recovery_by_addr,
    )?;
    let fetched = recovery::recovery_fetch(&app.pool, &login, &rc).await?;
    Ok(json(
        StatusCode::OK,
        &RecoveryFetchResponse {
            account_id: fetched.account_id,
            key_id: fetched.key_id,
            wrap_rc: to_b64(&fetched.wrap_rc),
            profile: fetched.profile,
        },
    ))
}

pub async fn recover(
    State(app): State<AppState>,
    Extension(address): Extension<ClientAddr>,
    body: Bytes,
) -> Result<Response, ApiError> {
    let request: RecoveryRequest = parse(&body)?;
    let login = normalize_login(&request.login).unwrap_or_default();
    let old = RcAuthKey::from_bytes(key(&request.old_rc_auth_key, "old_rc_auth_key")?);
    let new = NewRecovery {
        password: new_password(&request.new)?,
        rc_auth_key: RcAuthKey::from_bytes(key(&request.rc_auth_key, "rc_auth_key")?),
        wrap_rc: wrap(&request.wrap_rc, "wrap_rc")?,
    };
    let config = &app.config;
    limit(
        &app,
        "recovery",
        &login,
        &address,
        config.recovery_by_login,
        config.recovery_by_addr,
    )?;
    let (token, cred_version) =
        recovery::recover(&app.pool, app.clock.now_ms(), &login, &old, &new).await?;
    Ok(json(
        StatusCode::OK,
        &RecoveryResponse {
            token: token.encode(),
            cred_version,
        },
    ))
}

pub async fn reissue_words(
    State(app): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ApiError> {
    let (account, _) = authed(&app, &headers).await?;
    let request: ReissueRequest = parse(&body)?;
    let auth = AuthKey::from_bytes(key(&request.auth_key, "auth_key")?);
    let new_rc = RcAuthKey::from_bytes(key(&request.new_rc_auth_key, "new_rc_auth_key")?);
    let wrap_rc = wrap(&request.wrap_rc, "wrap_rc")?;
    recovery::reissue_words(&app.pool, &account, &auth, &new_rc, &wrap_rc).await?;
    Ok(json(StatusCode::OK, &serde_json::json!({})))
}

/// Counts the attempt against the login's budget and the address's, and refuses it past either (§8.4).
fn limit(
    app: &AppState,
    kind: &str,
    login: &str,
    address: &ClientAddr,
    by_login: Rule,
    by_addr: Rule,
) -> Result<(), ApiError> {
    let now = app.clock.now_ms();
    app.limiter
        .hit(&format!("{kind}:login:{login}"), by_login, now)
        .and_then(|()| {
            app.limiter
                .hit(&format!("{kind}:addr:{}", address.0), by_addr, now)
        })
        .map_err(ApiError::RateLimited)
}

fn login_of(text: &str) -> Result<String, ApiError> {
    normalize_login(text)
        .filter(|login| login.len() <= MAX_LOGIN)
        .ok_or(ApiError::BadRequest("login"))
}

fn key(text: &str, what: &'static str) -> Result<[u8; 32], ApiError> {
    key32(text).ok_or(ApiError::BadRequest(what))
}

fn salt(text: &str) -> Result<[u8; SALT_LEN], ApiError> {
    from_b64(text)
        .and_then(|bytes| <[u8; SALT_LEN]>::try_from(bytes.as_slice()).ok())
        .ok_or(ApiError::BadRequest("salt"))
}

/// A DEK wrap: the version byte and the length of appendix §A.1. The server cannot look inside.
fn wrap(text: &str, what: &'static str) -> Result<Vec<u8>, ApiError> {
    from_b64(text)
        .filter(|bytes| bytes.len() == WRAP_LEN && bytes.first() == Some(&WRAP_VERSION))
        .ok_or(ApiError::BadRequest(what))
}

/// The closed set of Argon2id profiles (§8.2); anything else is refused.
fn profile(number: u8) -> Result<u8, ApiError> {
    if matches!(number, 1 | 2) {
        Ok(number)
    } else {
        Err(ApiError::BadRequest("profile"))
    }
}

fn new_password(part: &PasswordPart) -> Result<NewPassword, ApiError> {
    Ok(NewPassword {
        salt: salt(&part.salt)?,
        profile: profile(part.profile)?,
        auth_key: AuthKey::from_bytes(key(&part.auth_key, "new_auth_key")?),
        wrap_pw: wrap(&part.wrap_pw, "wrap_pw")?,
    })
}
