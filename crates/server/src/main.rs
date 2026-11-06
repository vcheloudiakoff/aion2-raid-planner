//! Point d'entrée : sous-commandes `serve` (défaut) et `healthcheck`.

use std::net::{Ipv4Addr, SocketAddr};
use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;

use server::app::AppState;
use server::config::{Config, LISTEN_PORT};
use server::infra::clock::SystemClock;
use server::infra::ids::SystemIdGenerator;
use server::infra::sqlite::{self, Db};
use server::{healthcheck, http};

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    match (args.next().as_deref(), args.next()) {
        (None | Some("serve"), None) => serve(),
        (Some("healthcheck"), None) => {
            match healthcheck::check(healthcheck::ADDR, healthcheck::TIMEOUT) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("healthcheck failed: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        _ => {
            eprintln!("usage: server [serve | healthcheck]");
            ExitCode::from(2)
        }
    }
}

fn serve() -> ExitCode {
    let config = match Config::from_env() {
        Ok(c) => c,
        Err(e) => {
            // Le message ne contient que le nom de la variable, jamais sa valeur.
            eprintln!("configuration error: {e}");
            return ExitCode::FAILURE;
        }
    };
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(&config.rust_log))
        .with_writer(std::io::stdout)
        .init();

    let runtime = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("cannot start runtime: {e}");
            return ExitCode::FAILURE;
        }
    };
    match runtime.block_on(run(config)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!(error = %e, "server stopped on error");
            ExitCode::FAILURE
        }
    }
}

async fn run(_config: Config) -> Result<(), Box<dyn std::error::Error>> {
    let db = Db::open(sqlite::file_options(Path::new(sqlite::DB_PATH))).await?;
    db.migrate().await?;
    let state = AppState::new(
        db.clone(),
        Arc::new(SystemClock),
        Arc::new(SystemIdGenerator),
    );

    let addr = SocketAddr::from((Ipv4Addr::UNSPECIFIED, LISTEN_PORT));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(
        build = http::headers::APP_BUILD,
        port = LISTEN_PORT,
        "listening"
    );
    http::serve(listener, http::router(state), shutdown_signal()).await?;
    db.close().await;
    tracing::info!("stopped");
    Ok(())
}

/// SIGTERM (arrêt du conteneur) ou Ctrl-C.
async fn shutdown_signal() {
    let ctrl_c = async {
        // Sans gestionnaire Ctrl-C, attendre SIGTERM seul plutôt que s'arrêter aussitôt.
        if tokio::signal::ctrl_c().await.is_err() {
            std::future::pending::<()>().await;
        }
    };
    #[cfg(unix)]
    let term = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! {
        () = ctrl_c => {},
        () = term => {},
    }
}
