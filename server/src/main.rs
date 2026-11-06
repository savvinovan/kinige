//! `kinige-server`: the sync server core (spike S4c-1). Configuration is the environment (see `config::Settings` and the README).
//! TLS is not terminated here: a reverse proxy does it (S4c-2).

use kinige_server::config::{Config, Settings};
use kinige_server::{AppState, compact_loop, logging, router};
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> std::process::ExitCode {
    logging::init(std::env::var("KINIGE_LOG").ok().as_deref());
    match run().await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(reason) => {
            log::error!("the server stopped: {reason}");
            std::process::ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), String> {
    let settings = Settings::from_lookup(
        |name| std::env::var(name).ok(),
        |path| std::fs::read_to_string(path).ok(),
    )
    .map_err(|error| error.to_string())?;
    let state = AppState::open(
        &settings.database_url,
        settings.secret,
        Config::default(),
        settings.client_addr_header,
    )
    .await
    .map_err(|error| format!("the database is not usable ({})", error.code()))?;
    tokio::spawn(compact_loop(state.clone(), settings.compact_every_secs));
    let listener = tokio::net::TcpListener::bind(&settings.listen)
        .await
        .map_err(|_| "the listen address cannot be bound".to_string())?;
    log::info!("listening");
    axum::serve(
        listener,
        router(state).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(async {
        let _ = tokio::signal::ctrl_c().await;
    })
    .await
    .map_err(|_| "the server loop failed".to_string())
}
