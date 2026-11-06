//! `kinige-server`: the sync server (spikes S4c-1 and S4c-2). Configuration is the environment (see `config::Settings` and the README).
//! TLS is not terminated here: a reverse proxy does it (`deploy/`). Besides serving, the binary takes and restores backups
//! (`kinige-server backup <dir> [--prune]`, `kinige-server restore <backup directory>`).

use kinige_server::backup;
use kinige_server::blobs::Blobs;
use kinige_server::blobs::objects::Objects;
use kinige_server::blobs::staging::Staging;
use kinige_server::cli::{self, Command};
use kinige_server::config::Settings;
use kinige_server::store::lock::ServerLock;
use kinige_server::{AppState, Clock, SystemClock, compact_loop, logging, router, store};
use std::env;
use std::fs;
use std::net::SocketAddr;
use std::process::ExitCode;
use tokio::net::TcpListener;
use tokio::signal;

#[tokio::main]
async fn main() -> ExitCode {
    logging::init(env::var("KINIGE_LOG").ok().as_deref());
    let args: Vec<String> = env::args().skip(1).collect();
    let command = match cli::parse(&args) {
        Ok(command) => command,
        Err(usage) => {
            log::error!("{usage}");
            return ExitCode::from(2);
        }
    };
    match run(command).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(reason) => {
            log::error!("the command stopped: {reason}");
            ExitCode::FAILURE
        }
    }
}

async fn run(command: Command) -> Result<(), String> {
    let settings = Settings::from_lookup(
        |name| env::var(name).ok(),
        |path| fs::read_to_string(path).ok(),
    )
    .map_err(|error| error.to_string())?;
    let objects = Objects::open(&settings.blobs)?;
    match command {
        Command::Serve => serve(settings, objects).await,
        Command::Backup { dir, prune } => {
            let pool = store::connect(&settings.database_url).map_err(|e| e.code().to_string())?;
            let clock = SystemClock;
            let (written, manifest) = backup::create(&pool, &objects, &clock, &dir)
                .await
                .map_err(|error| error.to_string())?;
            log::info!(
                "backup {} has {} blobs",
                written
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                manifest.blobs.len()
            );
            if prune {
                let pruned =
                    backup::prune(&dir, clock.now_ms(), settings.config.backup_retention_ms)
                        .await
                        .map_err(|error| error.to_string())?;
                log::info!(
                    "pruned {} backups and {} unfinished ones",
                    pruned.backups.len(),
                    pruned.unfinished
                );
            }
            Ok(())
        }
        Command::Restore { dir } => {
            let pool = store::connect(&settings.database_url).map_err(|e| e.code().to_string())?;
            let report = backup::restore(&pool, &objects, &SystemClock, &dir)
                .await
                .map_err(|error| error.to_string())?;
            log::info!(
                "restored: a new generation, {} deletions and {} credential changes applied again, {} orphan objects",
                report.deletions_applied,
                report.credentials_applied,
                report.orphans
            );
            // The database is restored; a damaged blob of the backup is still an error for the operator to see and to act on.
            if report.damaged_blobs > 0 {
                return Err(format!(
                    "the database is restored, but {} blobs of the backup were missing or damaged; they are marked deleted and their devices will upload them again",
                    report.damaged_blobs
                ));
            }
            Ok(())
        }
    }
}

async fn serve(settings: Settings, objects: Objects) -> Result<(), String> {
    let blobs = Blobs::new(objects, Staging::new(settings.blobs.staging_dir()));
    let state = AppState::open(
        &settings.database_url,
        settings.secret,
        settings.config,
        settings.client_addr_header,
        blobs,
    )
    .await
    .map_err(|error| format!("the database is not usable ({})", error.code()))?;
    // Held for as long as the server runs: a restore refuses to run while it is held (`store::lock::ServerLock`).
    let url = store::parse_url(&settings.database_url).map_err(|error| error.code().to_string())?;
    let _server_lock = ServerLock::acquire(&url)
        .await
        .map_err(|error| format!("the server lock cannot be taken ({})", error.code()))?;
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
