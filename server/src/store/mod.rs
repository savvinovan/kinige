//! The database: the pool, the embedded migrations and the small helpers the queries share. The queries only store and lock; every
//! decision they act on is a function in `rules`.

pub mod accounts;
pub mod compact;
pub mod envelopes;
pub mod sessions;

use crate::error::ApiError;
use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod};
use std::str::FromStr;
use tokio_postgres::{NoTls, Row};

/// Connections kept by the pool.
const POOL_SIZE: usize = 16;
/// A constant that no other application takes: serializes migrations of concurrent starts.
const MIGRATION_LOCK: i64 = 0x004b_494e_4947_4501;

/// The migrations, in order. Each is applied once, in its own transaction, and recorded.
const MIGRATIONS: &[(i32, &str)] = &[(1, include_str!("../../migrations/0001_init.sql"))];

/// A pool for `url` (`postgres://user:password@host:port/database`). No connection is opened yet. TLS to the database is not used: the
/// database sits next to the server (README).
pub fn connect(url: &str) -> Result<Pool, ApiError> {
    let config = tokio_postgres::Config::from_str(url).map_err(|_| {
        // The text of the error can echo the URL, password included.
        log::error!("the database URL does not parse");
        ApiError::Internal
    })?;
    pool_for(config)
}

pub fn pool_for(config: tokio_postgres::Config) -> Result<Pool, ApiError> {
    let manager = Manager::from_config(
        config,
        NoTls,
        ManagerConfig {
            recycling_method: RecyclingMethod::Fast,
        },
    );
    Pool::builder(manager)
        .max_size(POOL_SIZE)
        .build()
        .map_err(|_| {
            log::error!("the database pool did not build");
            ApiError::Internal
        })
}

/// Applies the migrations that have not been applied. Safe to run from several processes at once.
pub async fn migrate(pool: &Pool) -> Result<(), ApiError> {
    let mut client = pool.get().await?;
    let tx = client.transaction().await?;
    tx.execute("SELECT pg_advisory_xact_lock($1)", &[&MIGRATION_LOCK])
        .await?;
    tx.batch_execute(
        "CREATE TABLE IF NOT EXISTS schema_migrations (version integer PRIMARY KEY, applied_ms bigint NOT NULL)",
    )
    .await?;
    for (version, sql) in MIGRATIONS {
        let done = tx
            .query_opt(
                "SELECT 1 FROM schema_migrations WHERE version = $1",
                &[version],
            )
            .await?;
        if done.is_none() {
            tx.batch_execute(sql).await?;
            tx.execute(
                "INSERT INTO schema_migrations (version, applied_ms) VALUES ($1, 0)",
                &[version],
            )
            .await?;
            log::info!("migration {version} applied");
        }
    }
    tx.commit().await?;
    Ok(())
}

/// A `u64` from the request as a Postgres `bigint`; above `i64::MAX` is a malformed request, not a database error.
pub fn to_i64(value: u64, what: &'static str) -> Result<i64, ApiError> {
    i64::try_from(value).map_err(|_| ApiError::BadRequest(what))
}

/// A `bigint` from the database as a `u64`; the columns hold no negative numbers.
pub fn to_u64(value: i64) -> u64 {
    u64::try_from(value).unwrap_or(0)
}

/// A `bytea` column of exactly `N` bytes.
pub fn fixed<const N: usize>(row: &Row, column: &str) -> Result<[u8; N], ApiError> {
    let bytes: Vec<u8> = row.try_get(column)?;
    <[u8; N]>::try_from(bytes.as_slice()).map_err(|_| {
        log::error!("a stored value has an unexpected length");
        ApiError::Internal
    })
}
