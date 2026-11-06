//! The real server for the simulation (feature `real-server`, S4c-1, KINIG-98): the `kinige-server` router on Postgres, called in process
//! through `tower::ServiceExt::oneshot`, so request parsing, the SQL, the account lock and the real envelope bytes are exercised together
//! with the protocol. Every simulated account is registered through the API with random 32-byte keys (no Argon2id), on a database the host
//! created for the test run and drops with it.
//!
//! Restores are not modelled (S4c-2): `as_memory` is `None`, so the world skips the `Backup` and `RestoreServer` steps, and the mixes that
//! rely on them (`Mix::RESTORE`, the restore-epoch generator) are not run against this server.

use crate::backend::{Cursor, SyncServer};
use crate::envelope::{Envelope, Key, Kind};
use crate::real::entity_name;
use crate::server::{PullResponse, PushResponse};
use kinige_server::dto::{
    PullResponse as WirePull, PushOutcome, PushResponse as WirePush, PushResult,
};
use kinige_server::store::compact;
use kinige_server::testing::{Creds, Http, TestDb};
use kinige_server::wire::{encode_frames, from_b64};
use kinige_server::{AppState, Clock, ManualClock, router};
use serde_json::json;
use std::future::Future;
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
    token: String,
    generation: u64,
    /// Entities for which the server accepted a `purged` (the first acceptance answers without a snapshot; a repeat carries one).
    purged: Vec<Key>,
}

impl HttpServer {
    fn new(host: Arc<RealServerHost>) -> HttpServer {
        let clock = Arc::new(ManualClock::new(crate::world::T0));
        let state = host
            .runtime
            .block_on(
                host.db
                    .state(clock.clone(), kinige_server::Config::default()),
            )
            .unwrap_or_else(|error| panic!("the real server did not start: {error}"));
        let http = Http::new(router(state.clone()));
        let creds = Creds::random();
        let token = host
            .runtime
            .block_on(creds.register(&http))
            .unwrap_or_else(|error| panic!("the simulated account did not register: {error}"));
        let generation = host.runtime.block_on(async {
            let client = state.pool.get().await.expect("a connection");
            let row = client
                .query_one(
                    "SELECT generation FROM lineage ORDER BY pos DESC LIMIT 1",
                    &[],
                )
                .await
                .expect("the lineage");
            u64::try_from(row.get::<_, i64>(0)).expect("a positive generation")
        });
        HttpServer {
            account: creds.account_id.clone(),
            host,
            http,
            state,
            clock,
            token,
            generation,
            purged: Vec::new(),
        }
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

    fn push(&mut self, env: &Arc<Envelope>, now_ms: u64, cursor: Cursor) -> PushResponse {
        self.clock.set(now_ms);
        let header = &env.header;
        let body = encode_frames(&[(
            entity_name(header.entity),
            &header.id,
            &header.op_id,
            env.wire(),
        )])
        .unwrap_or_else(|| panic!("a frame for {}", header.id));
        let cursor_text = format!("{}.{}.{}", cursor.0, cursor.1, cursor.2);
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

    fn pull(
        &mut self,
        generation: u64,
        after: u64,
        known_epoch: u64,
        limit: usize,
    ) -> PullResponse {
        let request = json!({
            "generation": generation.to_string(),
            "after": after,
            "known_epoch": known_epoch,
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
            } => PullResponse::GenerationChanged {
                generation: generation
                    .parse()
                    .unwrap_or_else(|_| panic!("a generation that is not a number")),
                stale,
                epoch,
            },
        }
    }

    fn compact(&mut self, now_ms: u64) {
        self.clock.set(now_ms);
        self.block(compact::account(
            &self.state.pool,
            &self.account,
            self.clock.now_ms(),
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
}
