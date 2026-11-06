//! Point d'entrée : sous-commandes `serve` (défaut), `healthcheck` et `validate-reference`.

use std::net::{Ipv4Addr, SocketAddr};
use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;

use server::app::AppState;
use server::app::tasks;
use server::config::{Config, LISTEN_PORT};
use server::infra::clock::SystemClock;
use server::infra::ids::SystemIdGenerator;
use server::infra::reference::{self, Loader, ReferenceStore};
use server::infra::sqlite::{self, Db};
use server::{healthcheck, http};

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    match (args.next().as_deref(), args.next(), args.next()) {
        (None | Some("serve"), None, None) => serve(),
        // Avant `Config` : la CI valide `reference/` sans variable d'environnement.
        (Some("validate-reference"), dir, None) => validate_reference(Path::new(
            dir.as_deref().unwrap_or(reference::REFERENCE_DIR),
        )),
        (Some("healthcheck"), None, None) => {
            match healthcheck::check(healthcheck::ADDR, healthcheck::TIMEOUT) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("healthcheck failed: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        _ => {
            eprintln!("usage: server [serve | healthcheck | validate-reference [DIR]]");
            ExitCode::from(2)
        }
    }
}

/// Même validation qu'au démarrage et au rechargement, sans repli sur le jeu embarqué.
fn validate_reference(dir: &Path) -> ExitCode {
    match reference::load_dir(dir) {
        Ok(set) => {
            println!(
                "reference OK: {} regions, {} servers, {} instances, {} classes, {} roles",
                set.regions.len(),
                set.servers.len(),
                set.instances.len(),
                set.classes.len(),
                set.roles.len()
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            for problem in e.problems() {
                eprintln!("{problem}");
            }
            ExitCode::FAILURE
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

    // Sans jeu valide, le serveur démarre quand même : santé et référence en 503, nouvel essai à
    // chaque tick.
    let store = ReferenceStore::new();
    let mut loader = Loader::new(reference::REFERENCE_DIR, store.clone());
    let loader = tokio::task::spawn_blocking(move || {
        loader.poll();
        loader
    })
    .await?;
    let _reload = tasks::reload::spawn(loader, tasks::reload::TICK);

    let state = AppState::new(
        db.clone(),
        Arc::new(SystemClock),
        Arc::new(SystemIdGenerator),
        store,
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
