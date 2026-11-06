//! Sauvegarde quotidienne en processus (AD-2, AD-14) : pas de cron du NAS, pas de conteneur de plus.
//! À chaque tick, `VACUUM INTO` si la dernière `daily` a plus de 24 h, puis rotation.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use sqlx::SqlitePool;
use tokio::task::JoinHandle;
use tokio::time::MissedTickBehavior;

use crate::infra::backup::{self, Kind};
use crate::infra::clock::Clock;

/// Période de vérification.
pub const TICK: Duration = Duration::from_secs(3600);

/// Lance la boucle ; le premier tick a lieu tout de suite. Une erreur est journalisée et la
/// sauvegarde retentée au tick suivant.
pub fn spawn(
    pool: SqlitePool,
    clock: Arc<dyn Clock>,
    dir: PathBuf,
    period: Duration,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(period);
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            interval.tick().await;
            tick(&pool, clock.as_ref(), &dir).await;
        }
    })
}

/// Un passage : sauvegarde si due, puis rotation. Public pour les tests.
pub async fn tick(pool: &SqlitePool, clock: &dyn Clock, dir: &std::path::Path) {
    let now = clock.now();
    let due = {
        let dir = dir.to_owned();
        tokio::task::spawn_blocking(move || backup::daily_due(&dir, now)).await
    };
    match due {
        Ok(Ok(true)) => match backup::vacuum_into(pool, dir, Kind::Daily, now).await {
            Ok(name) => tracing::info!(backup = %name, "daily backup written"),
            Err(e) => tracing::error!(error = %e, "daily backup failed"),
        },
        Ok(Ok(false)) => {}
        Ok(Err(e)) => tracing::error!(error = %e, "cannot list backups"),
        Err(e) => tracing::error!(error = %e, "cannot list backups"),
    }
    let pruned = {
        let dir = dir.to_owned();
        tokio::task::spawn_blocking(move || backup::prune(&dir, now)).await
    };
    match pruned {
        Ok(Ok(removed)) => {
            for name in removed {
                tracing::info!(backup = %name, "old backup removed");
            }
        }
        Ok(Err(e)) => tracing::error!(error = %e, "backup rotation failed"),
        Err(e) => tracing::error!(error = %e, "backup rotation failed"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::clock::FixedClock;
    use crate::infra::sqlite::{self, Db};

    fn files(dir: &std::path::Path) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        v.sort();
        v
    }

    #[tokio::test]
    async fn one_daily_per_day_and_two_kept() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("backups");
        std::fs::create_dir(&dir).unwrap();
        let db = Db::open(sqlite::file_options(&tmp.path().join("app.db")))
            .await
            .unwrap();
        let clock = FixedClock::new("2026-11-01T00:00:00Z".parse().unwrap());

        tick(&db.read, &clock, &dir).await;
        clock.set("2026-11-01T23:00:00Z".parse().unwrap());
        tick(&db.read, &clock, &dir).await;
        assert_eq!(files(&dir), ["daily-20261101T000000Z.db"]);

        for at in ["2026-11-02T00:00:00Z", "2026-11-03T00:30:00Z"] {
            clock.set(at.parse().unwrap());
            tick(&db.read, &clock, &dir).await;
        }
        assert_eq!(
            files(&dir),
            ["daily-20261102T000000Z.db", "daily-20261103T003000Z.db"]
        );
        db.close().await;
    }

    #[tokio::test]
    async fn failure_is_logged_and_retried_next_tick() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("backups");
        let db = Db::open(sqlite::file_options(&tmp.path().join("app.db")))
            .await
            .unwrap();
        let clock = FixedClock::new("2026-11-01T00:00:00Z".parse().unwrap());
        // Dossier absent : le `VACUUM INTO` échoue, sans panique.
        tick(&db.read, &clock, &dir).await;
        std::fs::create_dir(&dir).unwrap();
        clock.set("2026-11-01T01:00:00Z".parse().unwrap());
        tick(&db.read, &clock, &dir).await;
        assert_eq!(files(&dir), ["daily-20261101T010000Z.db"]);
        db.close().await;
    }
}
