//! What every request handler is given.

use crate::clock::{Clock, SystemClock};
use crate::config::Config;
use crate::error::ApiError;
use crate::rate::Limiter;
use crate::store;
use deadpool_postgres::Pool;
use kinige_crypto::secret::ServerSecret;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub pool: Pool,
    pub clock: Arc<dyn Clock>,
    /// For the salt of an unknown login (§8.4); never leaves the server.
    pub secret: Arc<ServerSecret>,
    pub limiter: Arc<Limiter>,
    pub config: Arc<Config>,
    /// The header a trusted reverse proxy sets to the client's address, if any (README, "Configuration").
    pub client_addr_header: Option<Arc<str>>,
}

impl AppState {
    /// Production: the system clock, migrations applied. `config` is the behaviour, usually `Config::default()`.
    pub async fn open(
        database_url: &str,
        secret: ServerSecret,
        config: Config,
        client_addr_header: Option<String>,
    ) -> Result<AppState, ApiError> {
        let pool = store::connect(database_url)?;
        store::migrate(&pool).await?;
        Ok(AppState::new(
            pool,
            Arc::new(SystemClock),
            secret,
            config,
            client_addr_header,
        ))
    }

    pub fn new(
        pool: Pool,
        clock: Arc<dyn Clock>,
        secret: ServerSecret,
        config: Config,
        client_addr_header: Option<String>,
    ) -> AppState {
        AppState {
            pool,
            clock,
            secret: Arc::new(secret),
            limiter: Arc::new(Limiter::default()),
            config: Arc::new(config),
            client_addr_header: client_addr_header.map(Arc::from),
        }
    }
}
