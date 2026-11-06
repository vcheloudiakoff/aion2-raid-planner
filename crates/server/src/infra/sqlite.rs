//! Base SQLite unique (AD-2) : deux pools, pragmas fixés, migrations embarquées.

use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use sqlx::migrate::{MigrateError, Migrator};
use sqlx::sqlite::{
    SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions, SqliteSynchronous,
};

/// Chemin fixe de la base dans le conteneur ; non configurable.
pub const DB_PATH: &str = "/data/app.db";

pub const WRITE_POOL_SIZE: u32 = 1;
pub const READ_POOL_SIZE: u32 = 4;
pub const BUSY_TIMEOUT: Duration = Duration::from_millis(5000);

/// Migrations embarquées dans le binaire, forward-only.
pub static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

/// Les deux pools : toute commande prend `write` (une connexion), toute lecture prend `read`.
#[derive(Debug, Clone)]
pub struct Db {
    pub write: SqlitePool,
    pub read: SqlitePool,
}

/// Applique les pragmas imposés par AD-2 à des options de connexion.
pub fn with_pragmas(options: SqliteConnectOptions) -> SqliteConnectOptions {
    options
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .foreign_keys(true)
        .busy_timeout(BUSY_TIMEOUT)
}

/// Options pour un fichier sur disque, créé s'il manque.
pub fn file_options(path: &Path) -> SqliteConnectOptions {
    SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
}

/// Options pour une base mémoire partagée nommée, pour les tests : les deux pools voient la même base.
pub fn shared_memory_options(name: &str) -> SqliteConnectOptions {
    SqliteConnectOptions::from_str(&format!("sqlite:file:{name}?mode=memory&cache=shared"))
        .expect("valid in-memory sqlite url")
}

impl Db {
    /// Ouvre les deux pools sur la même base. Le pool d'écriture s'ouvre d'abord : c'est lui qui
    /// fait passer un fichier neuf en WAL.
    pub async fn open(options: SqliteConnectOptions) -> Result<Self, sqlx::Error> {
        let options = with_pragmas(options);
        let write = SqlitePoolOptions::new()
            .max_connections(WRITE_POOL_SIZE)
            .min_connections(1)
            .connect_with(options.clone())
            .await?;
        let read = SqlitePoolOptions::new()
            .max_connections(READ_POOL_SIZE)
            .connect_with(options)
            .await?;
        Ok(Self { write, read })
    }

    pub async fn migrate(&self) -> Result<(), MigrateError> {
        MIGRATOR.run(&self.write).await
    }

    pub fn is_closed(&self) -> bool {
        self.write.is_closed() || self.read.is_closed()
    }

    pub async fn close(&self) {
        self.read.close().await;
        self.write.close().await;
    }
}

/// Vérifie que chaque migration de `migrator` est appliquée avec succès et inchangée.
pub async fn migrations_up_to_date(
    pool: &SqlitePool,
    migrator: &Migrator,
) -> Result<(), MigrationLag> {
    let applied: Vec<(i64, Vec<u8>, bool)> =
        sqlx::query_as("SELECT version, checksum, success FROM _sqlx_migrations")
            .fetch_all(pool)
            .await
            .map_err(MigrationLag::Query)?;
    for m in migrator
        .iter()
        .filter(|m| m.migration_type.is_up_migration())
    {
        match applied.iter().find(|(v, _, _)| *v == m.version) {
            Some((_, checksum, true)) if checksum.as_slice() == m.checksum.as_ref() => {}
            Some(_) => return Err(MigrationLag::Mismatch(m.version)),
            None => return Err(MigrationLag::Missing(m.version)),
        }
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum MigrationLag {
    #[error("cannot read applied migrations: {0}")]
    Query(sqlx::Error),
    #[error("embedded migration {0} is not applied")]
    Missing(i64),
    #[error("migration {0} failed or was modified after being applied")]
    Mismatch(i64),
}
