//! The real server for the simulation (feature `real-server`, S4c-1, KINIG-98): the `kinige-server` router on Postgres, called in process
//! through `tower::ServiceExt::oneshot`, so request parsing, the SQL, the account lock and the real envelope bytes are exercised together
//! with the protocol. Every simulated account is registered through the API with random 32-byte keys (no Argon2id), on a database the host
//! created for the test run and drops with it.
//!
//! Backups and restores are real (S4c-2): a backup is `kinige_server::backup::create` into a directory, and a restore loads that
//! directory with `kinige_server::backup::restore` into a **clean database with an empty blob store**, the code path the operator uses
//! for a restore into a clean installation. The router and the pool are swapped for the new installation's; the sessions are not in a
//! dump, so the simulated device logs in again, as a real one does after a restore.

use crate::backend::{Backup, Cursor, SyncServer};
use crate::envelope::{Envelope, Key, Kind};
use crate::real::entity_name;
use crate::server::{PullResponse, PushResponse};
use crate::world::T0;
use kinige_server::backup;
use kinige_server::dto::{
    PullResponse as WirePull, PushOutcome, PushResponse as WirePush, PushResult,
};
use kinige_server::store::compact;
use kinige_server::testing::{Creds, Http, TestDb};
use kinige_server::wire::{encode_frames, from_b64, to_b64};
use kinige_server::{AppState, Config, ManualClock, router};
use serde_json::json;
use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::runtime::Runtime;

/// The runtime and the throwaway database the simulated servers share. Cases get an account each, so they never see each other's log.
pub struct RealServerHost {
    runtime: Runtime,
    db: TestDb,
}

impl RealServerHost {
    /// Creates the database on the server `KINIGE_TEST_DATABASE_URL` points at.
    pub fn connect() -> Result<Arc<RealServerHost>, String> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(|error| error.to_string())?;
        let db = runtime.block_on(TestDb::create())?;
        Ok(Arc::new(RealServerHost { runtime, db }))
    }

    /// A server for one run: a new account on the shared database, a new clock, a new pool.
    pub fn server(self: &Arc<Self>) -> Box<dyn SyncServer> {
        Box::new(HttpServer::new(Arc::clone(self)))
    }
}

pub struct HttpServer {
    host: Arc<RealServerHost>,
    http: Http,
    state: AppState,
    clock: Arc<ManualClock>,
    account: String,
    creds: Creds,
    token: String,
    generation: u64,
    /// Entities for which the server accepted a `purged` (the first acceptance answers without a snapshot; a repeat carries one).
    purged: Vec<Key>,
    /// The clean installations the restores made, kept so that their databases and blob directories live as long as this server.
    installations: Vec<TestDb>,
    /// A directory for the backups, removed with the host's database.
    backups: PathBuf,
}

impl HttpServer {
    fn new(host: Arc<RealServerHost>) -> HttpServer {
        let clock = Arc::new(ManualClock::new(T0));
        let state = host
            .runtime
            .block_on(host.db.state(clock.clone(), Config::default()))
            .unwrap_or_else(|error| panic!("the real server did not start: {error}"));
        let http = Http::new(router(state.clone()));
        let creds = Creds::random();
        let token = host
            .runtime
            .block_on(creds.register(&http))
            .unwrap_or_else(|error| panic!("the simulated account did not register: {error}"));
        let generation = host.runtime.block_on(current_generation(&state));
        let backups = host
            .db
            .scratch_dir("s4a-backups")
            .unwrap_or_else(|error| panic!("a directory for the backups: {error}"));
        HttpServer {
            account: creds.account_id.clone(),
            creds,
            host,
            http,
            state,
            clock,
            token,
            generation,
            purged: Vec::new(),
            installations: Vec::new(),
            backups,
        }
    }

    /// The restore of an installation: a clean database and an empty store, the dump loaded by the operator's code path, then the
    /// device logs in again (a dump has no sessions).
    fn restore_into_a_clean_installation(&mut self, dir: &std::path::Path, now_ms: u64) {
        self.clock.set(now_ms);
        let (db, state, report) = self.block(async {
            let db = TestDb::create()
                .await
                .unwrap_or_else(|error| panic!("a clean database: {error}"));
            let state = db
                .state(self.clock.clone(), Config::default())
                .await
                .unwrap_or_else(|error| panic!("a clean installation: {error}"));
            let report = backup::restore(&state.pool, &state.blobs.objects, &*self.clock, dir)
                .await
                .unwrap_or_else(|error| panic!("the restore failed: {error}"));
            (db, state, report)
        });
        self.generation = report.generation;
        self.http = Http::new(router(state.clone()));
        self.state = state;
        self.installations.push(db);
        let login = json!({"login": self.creds.login, "auth_key": to_b64(&self.creds.auth_key)});
        let (status, answer) = self.block(self.http.post("/v1/login", None, &login));
        assert_eq!(status, 200, "the device logs in after a restore: {answer}");
        self.token = answer["token"]
            .as_str()
            .unwrap_or_else(|| panic!("a login without a token"))
            .to_string();
    }

    fn block<T>(&self, future: impl Future<Output = T>) -> T {
        self.host.runtime.block_on(future)
    }

    /// One `bigint` from a query on this account (`$1`), and an `op_id` (`$2`) when there is one.
    fn scalar(&self, sql: &str, op: Option<&str>) -> Option<i64> {
        self.block(async {
            let client = self.state.pool.get().await.expect("a connection");
            let row = match op {
                Some(op) => client.query_opt(sql, &[&self.account, &op]).await,
                None => client.query_opt(sql, &[&self.account]).await,
            };
            row.expect("a query").map(|row| row.get::<_, i64>(0))
        })
    }
}

impl SyncServer for HttpServer {
    fn generation(&self) -> u64 {
        self.generation
    }

    fn push(&mut self, env: &Arc<Envelope>, now_ms: u64, cursor: &Cursor) -> PushResponse {
        self.clock.set(now_ms);
        let header = &env.header;
        let body = encode_frames(&[(
            entity_name(header.entity),
            &header.id,
            &header.op_id,
            env.wire(),
        )])
        .unwrap_or_else(|| panic!("a frame for {}", header.id));
        // Three parts: a push carries no lineage, a pusher outside the current generation is stale whatever it could say.
        let cursor_text = format!(
            "{}.{}.{}",
            cursor.generation, cursor.seq, cursor.known_epoch
        );
        let (status, answer) =
            self.block(
                self.http
                    .push(Some(&self.token), &cursor_text, Some("1"), body),
            );
        assert_eq!(status, 200, "the real server refused a push: {answer}");
        let wire: WirePush = serde_json::from_value(answer)
            .unwrap_or_else(|error| panic!("a push response that does not parse: {error}"));
        let PushResult {
            result,
            seq,
            purged_of,
            ..
        } = wire
            .results
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("a push response without a result"));
        let seq_of = || seq.unwrap_or_else(|| panic!("an answer without a seq"));
        match result {
            PushOutcome::Ok => {
                if header.kind == Kind::Purged && purged_of.is_none() {
                    self.purged.push(header.key());
                }
                PushResponse::Ok(seq_of())
            }
            PushOutcome::Duplicate => PushResponse::Duplicate(seq_of()),
            PushOutcome::OpConflict => PushResponse::OpConflict,
            PushOutcome::Purged => PushResponse::Purged,
            PushOutcome::StalePurge => PushResponse::StalePurge,
            PushOutcome::ClockSkew => PushResponse::ClockSkew,
            PushOutcome::Stale => PushResponse::Stale,
            PushOutcome::TooOld => panic!("the real server called the simulation's client too old"),
        }
    }

    fn pull(&mut self, cursor: Cursor, limit: usize) -> PullResponse {
        let lineage: Vec<_> = cursor
            .lineage
            .iter()
            .map(|(generation, cut_ms)| json!({"generation": generation.to_string(), "cut_ms": cut_ms}))
            .collect();
        let request = json!({
            "generation": cursor.generation.to_string(),
            "after": cursor.seq,
            "known_epoch": cursor.known_epoch,
            "lineage": lineage,
            "limit": limit,
        });
        let (status, answer) = self.block(self.http.post("/v1/pull", Some(&self.token), &request));
        assert_eq!(status, 200, "the real server refused a pull: {answer}");
        let wire: WirePull = serde_json::from_value(answer)
            .unwrap_or_else(|error| panic!("a pull response that does not parse: {error}"));
        match wire {
            WirePull::Page {
                after,
                next,
                head,
                epoch,
                items,
                ..
            } => PullResponse::Page {
                after,
                items: items
                    .into_iter()
                    .map(|item| {
                        let bytes = from_b64(&item.envelope)
                            .unwrap_or_else(|| panic!("an envelope that is not base64"));
                        let env = Envelope::from_wire(bytes, &item.entity, &item.id, &item.op)
                            .unwrap_or_else(|| panic!("an envelope with an unreadable header"));
                        (item.seq, Arc::new(env))
                    })
                    .collect(),
                next,
                head,
                epoch,
            },
            WirePull::Stale { .. } => PullResponse::Stale,
            WirePull::GenerationChanged {
                generation,
                stale,
                epoch,
                lineage,
            } => PullResponse::GenerationChanged {
                generation: generation
                    .parse()
                    .unwrap_or_else(|_| panic!("a generation that is not a number")),
                stale,
                epoch,
                lineage: lineage
                    .into_iter()
                    .map(|entry| {
                        let id = entry
                            .generation
                            .parse()
                            .unwrap_or_else(|_| panic!("a generation that is not a number"));
                        (id, entry.cut_ms)
                    })
                    .collect(),
            },
        }
    }

    fn compact(&mut self, now_ms: u64) {
        self.clock.set(now_ms);
        self.block(compact::account(
            &self.state.pool,
            &self.account,
            &*self.clock,
        ))
        .unwrap_or_else(|error| panic!("compaction failed: {}", error.code()));
    }

    fn head(&self) -> u64 {
        self.scalar("SELECT seq_head FROM accounts WHERE id = $1", None)
            .map_or(0, |n| u64::try_from(n).unwrap_or(0))
    }

    fn issued(&self) -> u64 {
        self.head()
    }

    fn log_len(&self) -> usize {
        self.scalar("SELECT count(*) FROM envelopes WHERE account_id = $1", None)
            .map_or(0, |n| usize::try_from(n).unwrap_or(0))
    }

    fn settled(&self) -> bool {
        true
    }

    fn seq_of(&self, op_id: &str) -> Option<u64> {
        self.scalar(
            "SELECT seq FROM ops WHERE account_id = $1 AND op_id = $2",
            Some(op_id),
        )
        .and_then(|n| u64::try_from(n).ok())
    }

    fn all_envelopes(&self) -> Vec<Arc<Envelope>> {
        self.block(async {
            let client = self.state.pool.get().await.expect("a connection");
            let rows = client
                .query(
                    "SELECT entity, entity_id, op_id, bytes FROM envelopes WHERE account_id = $1 ORDER BY seq",
                    &[&self.account],
                )
                .await
                .expect("the log");
            rows.iter()
                .map(|row| {
                    let (entity, id, op): (String, String, String) =
                        (row.get(0), row.get(1), row.get(2));
                    let env = Envelope::from_wire(row.get(3), &entity, &id, &op)
                        .unwrap_or_else(|| panic!("a stored envelope with an unreadable header"));
                    Arc::new(env)
                })
                .collect()
        })
    }

    fn purged_ever(&self) -> Vec<Key> {
        self.purged.clone()
    }

    fn drops_count(&self) -> usize {
        self.block(compact::drops(&self.state.pool, &self.account))
            .map_or(0, |drops| drops.len())
    }

    fn backup(&mut self, now_ms: u64) -> Backup {
        self.clock.set(now_ms);
        let (dir, _) = self
            .block(backup::create(
                &self.state.pool,
                &self.state.blobs.objects,
                &*self.clock,
                &self.backups,
            ))
            .unwrap_or_else(|error| panic!("the backup failed: {error}"));
        Backup(Box::new(dir))
    }

    fn restore(&mut self, backup: &Backup, _nonce: u64, now_ms: u64) {
        if let Some(dir) = backup.0.downcast_ref::<PathBuf>() {
            self.restore_into_a_clean_installation(dir, now_ms);
        }
    }
}

/// The newest generation of the installation behind `state`.
async fn current_generation(state: &AppState) -> u64 {
    let client = state.pool.get().await.expect("a connection");
    let row = client
        .query_one(
            "SELECT generation FROM lineage ORDER BY pos DESC LIMIT 1",
            &[],
        )
        .await
        .expect("the lineage");
    u64::try_from(row.get::<_, i64>(0)).expect("a positive generation")
}
