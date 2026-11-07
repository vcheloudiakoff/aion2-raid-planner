//! Point d'entrée : sous-commandes `serve` (défaut), `healthcheck`, `validate-reference`,
//! `list-error-codes`, `backup` et `restore`.

use std::net::{Ipv4Addr, SocketAddr};
use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;

use server::app::AppState;
use server::app::tasks;
use server::config::{Config, LISTEN_PORT, check_room_template};
use server::infra::backup::{self, Kind};
use server::infra::clock::{Clock, SystemClock};
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
        // Avant `Config` : la CI compare le registre aux locales du client (AD-5, AD-12).
        (Some("list-error-codes"), None, None) => {
            for code in http::errors::Code::sorted() {
                println!("{code}");
            }
            ExitCode::SUCCESS
        }
        (Some("healthcheck"), None, None) => {
            match healthcheck::check(healthcheck::ADDR, healthcheck::TIMEOUT) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("healthcheck failed: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        // Sans `Config` ni migration : lancé par le déploiement dans le conteneur en place.
        (Some("backup"), kind, None) => {
            match kind.as_deref().map_or(Some(Kind::PreDeploy), Kind::parse) {
                Some(kind) => run_blocking(backup_now(kind)),
                None => usage(),
            }
        }
        // Service arrêté : `docker compose run --rm server restore NOM`.
        (Some("restore"), None, None) => list_backups(),
        (Some("restore"), Some(name), None) => run_blocking(restore(name)),
        _ => usage(),
    }
}

fn usage() -> ExitCode {
    eprintln!(
        "usage: server [serve | healthcheck | validate-reference [DIR] | list-error-codes | backup [daily | pre-deploy] | restore [NAME]]"
    );
    ExitCode::from(2)
}

/// Exécute une sous-commande asynchrone sur un runtime à un fil ; erreur = code 1.
fn run_blocking(
    fut: impl std::future::Future<Output = Result<(), Box<dyn std::error::Error>>>,
) -> ExitCode {
    let result = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(Into::into)
        .and_then(|rt| rt.block_on(fut));
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Sauvegarde immédiate puis rotation ; le nom écrit sort seul sur stdout.
async fn backup_now(kind: Kind) -> Result<(), Box<dyn std::error::Error>> {
    // La base doit exister : pas de fichier vide créé par une sauvegarde.
    let options = sqlite::with_pragmas(
        sqlite::file_options(Path::new(sqlite::DB_PATH)).create_if_missing(false),
    );
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await?;
    let now = SystemClock.now();
    let dir = Path::new(backup::BACKUP_DIR);
    let result = backup::vacuum_into(&pool, dir, kind, now).await;
    pool.close().await;
    let name = result?;
    // Le nom d'abord : un souci de rotation n'empêche pas un déploiement dont la sauvegarde existe.
    println!("{name}");
    match backup::prune(dir, now) {
        Ok(removed) => {
            for r in removed {
                eprintln!("old backup removed: {r}");
            }
        }
        Err(e) => eprintln!("warning: backup rotation failed: {e}"),
    }
    Ok(())
}

/// Sans argument : liste des sauvegardes, code 2.
fn list_backups() -> ExitCode {
    match backup::list(Path::new(backup::BACKUP_DIR)) {
        Ok(list) => {
            for b in &list {
                println!("{}", b.name);
            }
            eprintln!(
                "usage: server restore NAME ({} backups listed above)",
                list.len()
            );
            ExitCode::from(2)
        }
        Err(e) => {
            eprintln!("error: cannot list backups: {e}");
            ExitCode::FAILURE
        }
    }
}

async fn restore(name: String) -> Result<(), Box<dyn std::error::Error>> {
    backup::restore(
        Path::new(backup::BACKUP_DIR),
        &name,
        Path::new(sqlite::DB_PATH),
    )
    .await?;
    println!("restored {name}");
    Ok(())
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

async fn run(config: Config) -> Result<(), Box<dyn std::error::Error>> {
    // Référence d'abord : un gabarit de nom de salle qui ne tient pas arrête le démarrage avant
    // toute migration. Sans jeu valide, le serveur démarre quand même : santé et référence en 503,
    // nouvel essai à chaque tick.
    let store = ReferenceStore::new();
    let mut loader = Loader::new(reference::REFERENCE_DIR, store.clone());
    let mut loader = tokio::task::spawn_blocking(move || {
        loader.poll();
        loader
    })
    .await?;
    if let Some(set) = store.get() {
        check_room_template(&config.room_name_template, &set)?;
    }
    // Ensuite, un jeu rechargé que le gabarit ne tient plus est refusé, l'ancien gardé.
    loader.guard_room_template(config.room_name_template.clone());

    let db = Db::open(sqlite::file_options(Path::new(sqlite::DB_PATH))).await?;
    db.migrate().await?;

    let _reload = tasks::reload::spawn(loader, tasks::reload::TICK);

    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let _backup = tasks::backup::spawn(
        db.read.clone(),
        clock.clone(),
        backup::BACKUP_DIR.into(),
        tasks::backup::TICK,
    );

    let state = AppState::new(
        db.clone(),
        clock,
        Arc::new(SystemIdGenerator),
        store,
        &config,
    );
    let _limits = tasks::limits::spawn(state.limits.clone(), tasks::limits::TICK);

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
