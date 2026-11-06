//! Helpers for tests and for the S4a simulation (feature `testing`): a throwaway database, an in-process client of the router, and
//! synthetic credentials. Not part of the server binary. Everything here is synthetic data.

use crate::blobs::Blobs;
use crate::blobs::objects::Objects;
use crate::blobs::settings::S3Settings;
use crate::blobs::staging::Staging;
use crate::clock::ManualClock;
use crate::config::Config;
use crate::dto::{PasswordPart, RegisterRequest};
use crate::state::AppState;
use crate::store;
use crate::wire::to_b64;
use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use kinige_crypto::secret::ServerSecret;
use kinige_crypto::wrap::{WRAP_LEN, WRAP_VERSION};
use libsodium_rs::random;
use serde_json::Value;
use std::str::FromStr;
use std::sync::Arc;
use tokio_postgres::NoTls;
use tower::ServiceExt;

/// The environment variable that points at a Postgres server where databases may be created and dropped.
pub const TEST_DATABASE_URL: &str = "KINIGE_TEST_DATABASE_URL";

/// The throwaway S3-compatible store of the blob suites (README, "Tests"): `KINIGE_TEST_S3_URL` (`http://host:port`) with
/// `KINIGE_TEST_S3_BUCKET`, `KINIGE_TEST_S3_ACCESS_KEY` and `KINIGE_TEST_S3_SECRET_KEY`. Each database gets its own key prefix.
pub const TEST_S3_URL: &str = "KINIGE_TEST_S3_URL";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlobBackend {
    Disk,
    S3,
}

/// The backends a blob suite runs against: the disk always, S3 when `KINIGE_TEST_S3_URL` is set. With `KINIGE_TEST_REQUIRE_S3=1` (CI)
/// a missing store is a failure and not a quiet skip.
pub fn blob_backends() -> Result<Vec<BlobBackend>, String> {
    let configured = std::env::var(TEST_S3_URL).is_ok();
    if !configured && std::env::var("KINIGE_TEST_REQUIRE_S3").is_ok_and(|v| v == "1") {
        return Err(format!(
            "{TEST_S3_URL} is not set and the S3 suites are required"
        ));
    }
    Ok(if configured {
        vec![BlobBackend::Disk, BlobBackend::S3]
    } else {
        vec![BlobBackend::Disk]
    })
}

/// A database created for one test and dropped with it (also on a panic). Runs never share state. The blob directories the test
/// made go with it.
pub struct TestDb {
    admin: tokio_postgres::Config,
    name: String,
    config: tokio_postgres::Config,
    dirs: std::sync::Mutex<Vec<std::path::PathBuf>>,
}

impl TestDb {
    pub async fn create() -> Result<TestDb, String> {
        let url = std::env::var(TEST_DATABASE_URL)
            .map_err(|_| format!("{TEST_DATABASE_URL} is not set"))?;
        let admin = tokio_postgres::Config::from_str(&url)
            .map_err(|_| "the test database URL does not parse".to_string())?;
        let name = format!("kinige_t_{}", hex(&random::bytes(8)));
        let (client, connection) = admin
            .connect(NoTls)
            .await
            .map_err(|error| format!("cannot connect to the test server: {error}"))?;
        let driver = tokio::spawn(connection);
        client
            .batch_execute(&format!("CREATE DATABASE {name}"))
            .await
            .map_err(|error| format!("cannot create a database: {error}"))?;
        drop(client);
        let _ = driver.await;
        let mut config = admin.clone();
        config.dbname(&name);
        Ok(TestDb {
            admin,
            name,
            config,
            dirs: std::sync::Mutex::default(),
        })
    }

    /// A server state on this database with the migrations applied, on the given clock, with blobs on a throwaway directory.
    pub async fn state(&self, clock: Arc<ManualClock>, config: Config) -> Result<AppState, String> {
        self.state_on(clock, config, BlobBackend::Disk).await
    }

    pub async fn state_on(
        &self,
        clock: Arc<ManualClock>,
        config: Config,
        backend: BlobBackend,
    ) -> Result<AppState, String> {
        let pool = store::pool_for(self.config.clone()).map_err(|e| e.code().to_string())?;
        store::migrate(&pool)
            .await
            .map_err(|e| e.code().to_string())?;
        let secret = ServerSecret::random();
        let blobs = self.blobs(backend)?;
        Ok(AppState::new(pool, clock, secret, config, None, blobs))
    }

    /// A blob store and staging area of their own: a new directory, and for S3 a new key prefix in the shared bucket.
    pub fn blobs(&self, backend: BlobBackend) -> Result<Blobs, String> {
        let dir = self.scratch_dir("blobs")?;
        let objects = match backend {
            BlobBackend::Disk => Objects::disk(&dir.join("objects"))?,
            BlobBackend::S3 => {
                let var =
                    |name: &str| std::env::var(name).map_err(|_| format!("{name} is not set"));
                let settings = S3Settings {
                    bucket: var("KINIGE_TEST_S3_BUCKET")?,
                    region: "us-east-1".to_string(),
                    endpoint: Some(var(TEST_S3_URL)?),
                    access_key: var("KINIGE_TEST_S3_ACCESS_KEY")?,
                    secret_key: var("KINIGE_TEST_S3_SECRET_KEY")?,
                    allow_http: true,
                };
                Objects::s3(&settings)?.with_prefix(&self.name)
            }
        };
        Ok(Blobs::new(objects, Staging::new(dir.join("staging"))))
    }

    /// A new empty directory that is removed with the database.
    pub fn scratch_dir(&self, label: &str) -> Result<std::path::PathBuf, String> {
        let dir = std::env::temp_dir().join(format!(
            "kinige-test-{}-{label}-{}",
            self.name,
            hex(&random::bytes(4))
        ));
        std::fs::create_dir_all(&dir).map_err(|_| "cannot make a test directory".to_string())?;
        self.dirs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(dir.clone());
        Ok(dir)
    }

    /// A pool on this database for raw SQL in a test (the dump, the negative control).
    pub fn pool(&self) -> Result<deadpool_postgres::Pool, String> {
        store::pool_for(self.config.clone()).map_err(|e| e.code().to_string())
    }
}

impl Drop for TestDb {
    fn drop(&mut self) {
        let dirs = std::mem::take(
            &mut *self
                .dirs
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
        for dir in dirs {
            let _ = std::fs::remove_dir_all(dir);
        }
        let admin = self.admin.clone();
        let name = self.name.clone();
        let cleanup = move || {
            let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                return;
            };
            runtime.block_on(async {
                if let Ok((client, connection)) = admin.connect(NoTls).await {
                    let driver = tokio::spawn(connection);
                    let _ = client
                        .batch_execute(&format!("DROP DATABASE IF EXISTS {name} WITH (FORCE)"))
                        .await;
                    drop(client);
                    let _ = driver.await;
                }
            });
        };
        // A thread of its own: `drop` may run inside a runtime, where blocking on another one is refused.
        if let Ok(handle) = std::thread::Builder::new().spawn(cleanup) {
            let _ = handle.join();
        }
    }
}

/// The router as a client sees it, without a socket.
#[derive(Clone)]
pub struct Http {
    router: Router,
}

impl Http {
    pub fn new(router: Router) -> Http {
        Http { router }
    }

    /// Sends a request; the body of the answer is returned as bytes.
    pub async fn send(&self, request: Request<Body>) -> (StatusCode, Vec<u8>) {
        let Ok(response) = self.router.clone().oneshot(request).await;
        let status = response.status();
        let bytes = response
            .into_body()
            .collect()
            .await
            .map(|collected| collected.to_bytes().to_vec())
            .unwrap_or_default();
        (status, bytes)
    }

    /// Any request, with headers; the answer's status, headers and body.
    pub async fn request(
        &self,
        method: &str,
        path: &str,
        token: Option<&str>,
        headers: &[(&str, String)],
        body: Body,
    ) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
        let mut builder = Request::builder().method(method).uri(path);
        if let Some(token) = token {
            builder = builder.header("authorization", format!("Bearer {token}"));
        }
        for (name, value) in headers {
            builder = builder.header(*name, value);
        }
        let Ok(request) = builder.body(body) else {
            return (
                StatusCode::BAD_REQUEST,
                axum::http::HeaderMap::new(),
                Vec::new(),
            );
        };
        let Ok(response) = self.router.clone().oneshot(request).await;
        let (parts, body) = response.into_parts();
        let bytes = body
            .collect()
            .await
            .map(|collected| collected.to_bytes().to_vec())
            .unwrap_or_default();
        (parts.status, parts.headers, bytes)
    }

    /// `POST path` with a JSON body and, if given, a bearer token. The answer is parsed as JSON (`Null` if it is not).
    pub async fn post(&self, path: &str, token: Option<&str>, body: &Value) -> (StatusCode, Value) {
        let raw = serde_json::to_vec(body).unwrap_or_default();
        self.post_raw(path, token, raw).await
    }

    pub async fn post_raw(
        &self,
        path: &str,
        token: Option<&str>,
        body: Vec<u8>,
    ) -> (StatusCode, Value) {
        let mut builder = Request::builder().method("POST").uri(path);
        if let Some(token) = token {
            builder = builder.header("authorization", format!("Bearer {token}"));
        }
        match builder.body(Body::from(body)) {
            Ok(request) => parsed(self.send(request).await),
            Err(_) => (StatusCode::BAD_REQUEST, Value::Null),
        }
    }

    /// A push: `cursor` is `<generation>.<seq>.<epoch>`, `protocol` the protocol version header, `body` the frames.
    pub async fn push(
        &self,
        token: Option<&str>,
        cursor: &str,
        protocol: Option<&str>,
        body: Vec<u8>,
    ) -> (StatusCode, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/v1/push")
            .header("x-kinige-cursor", cursor);
        if let Some(protocol) = protocol {
            builder = builder.header("x-kinige-protocol", protocol);
        }
        if let Some(token) = token {
            builder = builder.header("authorization", format!("Bearer {token}"));
        }
        match builder.body(Body::from(body)) {
            Ok(request) => parsed(self.send(request).await),
            Err(_) => (StatusCode::BAD_REQUEST, Value::Null),
        }
    }
}

fn parsed((status, bytes): (StatusCode, Vec<u8>)) -> (StatusCode, Value) {
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

/// Random synthetic credentials: 32-byte keys and wraps of the right shape. No Argon2id runs, so a test registers in milliseconds.
#[derive(Clone)]
pub struct Creds {
    pub account_id: String,
    pub login: String,
    pub salt: Vec<u8>,
    pub profile: u8,
    pub auth_key: [u8; 32],
    pub rc_auth_key: [u8; 32],
    pub wrap_pw: Vec<u8>,
    pub wrap_rc: Vec<u8>,
}

impl Creds {
    pub fn random() -> Creds {
        let id = random::bytes(16);
        Creds {
            account_id: crate::wire::uuid_text(&id),
            login: format!("fake.{}@example.test", hex(&random::bytes(4))),
            salt: random::bytes(16),
            profile: 1,
            auth_key: array32(),
            rc_auth_key: array32(),
            wrap_pw: random_wrap(),
            wrap_rc: random_wrap(),
        }
    }

    pub fn register_request(&self) -> RegisterRequest {
        RegisterRequest {
            account_id: self.account_id.clone(),
            login: self.login.clone(),
            salt: to_b64(&self.salt),
            profile: self.profile,
            auth_key: to_b64(&self.auth_key),
            rc_auth_key: to_b64(&self.rc_auth_key),
            wrap_pw: to_b64(&self.wrap_pw),
            wrap_rc: to_b64(&self.wrap_rc),
            key_id: 1,
        }
    }

    /// A new password for the same account: a new salt, key and wrap.
    pub fn new_password(&self, profile: u8) -> (PasswordPart, [u8; 32], Vec<u8>, Vec<u8>) {
        let (key, wrap, salt) = (array32(), random_wrap(), random::bytes(16));
        let part = PasswordPart {
            salt: to_b64(&salt),
            profile,
            auth_key: to_b64(&key),
            wrap_pw: to_b64(&wrap),
        };
        (part, key, wrap, salt)
    }

    /// Registers over HTTP and returns the session token.
    pub async fn register(&self, http: &Http) -> Result<String, String> {
        let body = serde_json::to_value(self.register_request()).map_err(|e| e.to_string())?;
        let (status, answer) = http.post("/v1/register", None, &body).await;
        if status != StatusCode::OK {
            return Err(format!("registration answered {status}"));
        }
        answer
            .get("token")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| "no token".to_string())
    }
}

pub fn array32() -> [u8; 32] {
    let mut bytes = [0u8; 32];
    random::fill_bytes(&mut bytes);
    bytes
}

/// 73 bytes that start with the wrap version: what the server can check of a wrap.
pub fn random_wrap() -> Vec<u8> {
    let mut wrap = random::bytes(WRAP_LEN);
    if let Some(first) = wrap.first_mut() {
        *first = WRAP_VERSION;
    }
    wrap
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
