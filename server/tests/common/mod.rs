//! Shared by the suites that need a real Postgres (`--features pg-tests`, `KINIGE_TEST_DATABASE_URL`). Every test makes its own database
//! and drops it, so runs never share state. All data here is synthetic.
#![allow(dead_code)]

use axum::http::StatusCode;
use kinige_crypto::header::{GroupStamp, Header, Kind};
use kinige_crypto::{AccountId, Dek, Operation, seal_operation};
use kinige_server::config::Config;
use kinige_server::dto::PushResponse;
use kinige_server::testing::{Creds, Http, TestDb};
use kinige_server::wire::encode_frames;
use kinige_server::{AppState, ManualClock, router};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::thread::ThreadId;

pub mod leak;

/// The virtual "now" of the tests, ms (2027-01-15).
pub const T0: u64 = 1_800_000_000_000;
pub const DAY: u64 = 24 * 60 * 60 * 1000;
pub const DEVICE: &str = "66666666-6666-4666-8666-666666666666";

pub struct Rig {
    pub db: TestDb,
    pub clock: Arc<ManualClock>,
    pub state: AppState,
    pub http: Http,
}

pub async fn rig() -> Rig {
    rig_with(Config::default()).await
}

pub async fn rig_with(config: Config) -> Rig {
    capture_logs();
    let db = TestDb::create().await.expect("a test database");
    let clock = Arc::new(ManualClock::new(T0));
    let state = db.state(clock.clone(), config).await.expect("a state");
    let http = Http::new(router(state.clone()));
    Rig {
        db,
        clock,
        state,
        http,
    }
}

/// A registered account and its session.
pub struct Acct {
    pub creds: Creds,
    pub token: String,
}

pub async fn account(rig: &Rig) -> Acct {
    let creds = Creds::random();
    let token = creds.register(&rig.http).await.expect("registration");
    Acct { creds, token }
}

pub fn b64(bytes: &[u8]) -> String {
    kinige_server::wire::to_b64(bytes)
}

pub fn uuid(n: u8) -> String {
    format!("{n:08x}-0000-4000-8000-{n:012x}")
}

/// The id of record `n` (and, in the tests, of an op).
pub fn id(n: u8) -> String {
    uuid(n)
}

/// A one-group upsert of message `n` at `wall`, under op `op_n`.
pub fn msg(maker: &Maker, n: u8, op_n: u8, wall: u64) -> Vec<u8> {
    maker.upsert(
        "message",
        &id(n),
        &id(op_n),
        &[("body", hlc(wall, 0))],
        "fake entry",
    )
}

/// Pushes `bytes` as message `n` under op `op_n` and returns the single result.
pub async fn put(rig: &Rig, acct: &Acct, n: u8, op_n: u8, bytes: &[u8]) -> Value {
    let (id, op) = (id(n), id(op_n));
    push(
        rig,
        acct,
        &Item {
            entity: "message",
            id: &id,
            op: &op,
            bytes,
        },
    )
    .await
}

/// Compacts the account at the rig's clock.
pub async fn compact_now(rig: &Rig, acct: &Acct) -> kinige_server::store::compact::Stats {
    kinige_server::store::compact::account(
        &rig.state.pool,
        &acct.creds.account_id,
        kinige_server::Clock::now_ms(&*rig.clock),
    )
    .await
    .expect("compaction")
}

pub fn hlc(wall: u64, counter: u32) -> String {
    format!("{wall:013}-{counter:05}-{DEVICE}")
}

/// Builds sealed envelopes under one synthetic account and DEK (the server never opens them).
pub struct Maker {
    account: AccountId,
    dek: Dek,
}

impl Maker {
    pub fn new() -> Maker {
        Maker {
            account: AccountId::from_bytes([0x11; 16]),
            dek: Dek::generate(1).expect("dek"),
        }
    }

    /// An `upsert` of `entity`/`id` with `groups` as `(name, hlc)`, each carrying `text` as its encrypted content.
    pub fn upsert(
        &self,
        entity: &str,
        id: &str,
        op: &str,
        groups: &[(&str, String)],
        text: &str,
    ) -> Vec<u8> {
        let header = Header {
            entity: entity.to_string(),
            id: id.to_string(),
            op: op.to_string(),
            kind: Kind::Upsert,
            groups: groups
                .iter()
                .map(|(name, h)| {
                    (
                        name.to_string(),
                        GroupStamp {
                            h: h.clone(),
                            sv: 1,
                        },
                    )
                })
                .collect(),
            purged_of: None,
            blob: None,
        };
        let payload: BTreeMap<String, String> = groups
            .iter()
            .map(|(name, _)| (name.to_string(), json!({ "text": text }).to_string()))
            .collect();
        self.seal(header, payload)
    }

    pub fn dek(&self) -> &Dek {
        &self.dek
    }

    pub fn account(&self) -> &AccountId {
        &self.account
    }

    pub fn purged(&self, entity: &str, id: &str, op: &str, snapshot: &[(&str, String)]) -> Vec<u8> {
        let header = Header {
            entity: entity.to_string(),
            id: id.to_string(),
            op: op.to_string(),
            kind: Kind::Purged,
            groups: BTreeMap::new(),
            purged_of: Some(
                snapshot
                    .iter()
                    .map(|(g, h)| (g.to_string(), h.clone()))
                    .collect(),
            ),
            blob: None,
        };
        self.seal(header, BTreeMap::new())
    }

    fn seal(&self, header: Header, groups: BTreeMap<String, String>) -> Vec<u8> {
        seal_operation(&self.account, &self.dek, &Operation { header, groups }).expect("seal")
    }
}

/// One frame of a push body.
pub struct Item<'a> {
    pub entity: &'a str,
    pub id: &'a str,
    pub op: &'a str,
    pub bytes: &'a [u8],
}

pub fn body(items: &[Item<'_>]) -> Vec<u8> {
    let tuples: Vec<(&str, &str, &str, &[u8])> = items
        .iter()
        .map(|i| (i.entity, i.id, i.op, i.bytes))
        .collect();
    encode_frames(&tuples).expect("frames")
}

pub fn cursor(generation: u64, after: u64, epoch: u64) -> String {
    format!("{generation}.{after}.{epoch}")
}

/// Pushes one envelope with a fresh-pull cursor of the current generation and returns the single result.
pub async fn push(rig: &Rig, acct: &Acct, item: &Item<'_>) -> Value {
    let generation = generation(rig).await;
    let (status, answer) = rig
        .http
        .push(
            Some(&acct.token),
            &cursor(generation, 0, 0),
            Some("1"),
            body(std::slice::from_ref(item)),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    answer["results"][0].clone()
}

pub async fn push_with(
    rig: &Rig,
    acct: &Acct,
    cursor: &str,
    protocol: Option<&str>,
    item: &Item<'_>,
) -> (StatusCode, Value) {
    rig.http
        .push(
            Some(&acct.token),
            cursor,
            protocol,
            body(std::slice::from_ref(item)),
        )
        .await
}

pub async fn pull(
    rig: &Rig,
    acct: &Acct,
    generation: u64,
    after: u64,
    epoch: u64,
    limit: usize,
) -> (StatusCode, Value) {
    rig.http
        .post(
            "/v1/pull",
            Some(&acct.token),
            &json!({"generation": generation.to_string(), "after": after, "known_epoch": epoch, "limit": limit}),
        )
        .await
}

/// The installation's current generation.
pub async fn generation(rig: &Rig) -> u64 {
    let client = rig.state.pool.get().await.expect("connection");
    let row = client
        .query_one(
            "SELECT generation FROM lineage ORDER BY pos DESC LIMIT 1",
            &[],
        )
        .await
        .expect("lineage");
    u64::try_from(row.get::<_, i64>(0)).expect("positive")
}

/// The `seq`s in an account's log, ascending.
pub async fn log_seqs(rig: &Rig, acct: &Acct) -> Vec<u64> {
    let client = rig.state.pool.get().await.expect("connection");
    let rows = client
        .query(
            "SELECT seq FROM envelopes WHERE account_id = $1 ORDER BY seq",
            &[&acct.creds.account_id],
        )
        .await
        .expect("seqs");
    rows.iter()
        .map(|r| u64::try_from(r.get::<_, i64>(0)).expect("positive"))
        .collect()
}

pub fn outcome(result: &Value) -> &str {
    result["result"].as_str().unwrap_or("?")
}

pub fn parse_push(answer: &Value) -> PushResponse {
    serde_json::from_value(answer.clone()).expect("a push response")
}

static LOGS: OnceLock<Mutex<Vec<(ThreadId, String)>>> = OnceLock::new();

struct Capture;

impl log::Log for Capture {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        // The most verbose setting an operator can choose (`KINIGE_LOG=trace`), through the filter the binary uses.
        kinige_server::logging::permits(
            metadata.level(),
            metadata.target(),
            log::LevelFilter::Trace,
        )
    }

    fn log(&self, record: &log::Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let line = format!("{} {}", record.level(), record.args());
        LOGS.get_or_init(Mutex::default)
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((std::thread::current().id(), line));
    }

    fn flush(&self) {}
}

/// Installs a logger that keeps every record of the server, so a test can read and scan what it wrote.
pub fn capture_logs() {
    static CAPTURE: Capture = Capture;
    if log::set_logger(&CAPTURE).is_ok() {
        log::set_max_level(log::LevelFilter::Trace);
    }
}

/// Every line logged so far in this process, by every test.
pub fn logged() -> Vec<String> {
    LOGS.get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .map(|(_, line)| line.clone())
        .collect()
}

/// The lines logged on the calling thread: a `#[tokio::test]` runs its server on its own thread, so these are one test's.
pub fn logged_here() -> Vec<String> {
    let me = std::thread::current().id();
    LOGS.get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .filter(|(thread, _)| *thread == me)
        .map(|(_, line)| line.clone())
        .collect()
}
