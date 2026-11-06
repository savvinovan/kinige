//! `kinige-server`: the sync server core (spike S4c-1). Configuration is the environment (see `config::Settings` and the README).
//! TLS is not terminated here: a reverse proxy does it (S4c-2).

use kinige_server::config::{Config, Settings};
use kinige_server::{AppState, compact_loop, logging, router};
use std::env;
use std::fs;
use std::net::SocketAddr;
use std::process::ExitCode;
use tokio::net::TcpListener;
use tokio::signal;

#[tokio::main]
async fn main() -> ExitCode {
    logging::init(env::var("KINIGE_LOG").ok().as_deref());
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(reason) => {
            log::error!("the server stopped: {reason}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), String> {
    let settings = Settings::from_lookup(
        |name| env::var(name).ok(),
        |path| fs::read_to_string(path).ok(),
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
    let listener = TcpListener::bind(&settings.listen)
        .await
        .map_err(|_| "the listen address cannot be bound".to_string())?;
    log::info!("listening");
    axum::serve(
        listener,
        router(state).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown())
    .await
    .map_err(|_| "the server loop failed".to_string())
}

/// Resolves on Ctrl-C or, on Unix, SIGTERM, which is what container runtimes send.
async fn shutdown() {
    let interrupt = async {
        let _ = signal::ctrl_c().await;
    };
    #[cfg(unix)]
    {
        use signal::unix::{SignalKind, signal as unix_signal};
        let terminate = async {
            match unix_signal(SignalKind::terminate()) {
                Ok(mut stream) => {
                    stream.recv().await;
                }
                // No handler could be installed: wait for Ctrl-C only.
                Err(_) => std::future::pending::<()>().await,
            }
        };
        tokio::select! {
            () = interrupt => {}
            () = terminate => {}
        }
    }
    #[cfg(not(unix))]
    interrupt.await;
}
