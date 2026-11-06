//! Rechargement de la référence (AD-6) : un `poll` par tick, hors du runtime asynchrone.

use std::time::Duration;

use tokio::task::JoinHandle;
use tokio::time::MissedTickBehavior;

use crate::infra::reference::Loader;

/// Période de scrutation ; AD-6 exige au plus 60 s.
pub const TICK: Duration = Duration::from_secs(10);

/// Lance la boucle. Le premier tick a lieu une période après l'appel : le chargement initial est
/// fait par l'appelant avant d'ouvrir le port.
pub fn spawn(mut loader: Loader, period: Duration) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval_at(tokio::time::Instant::now() + period, period);
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            interval.tick().await;
            loader = match tokio::task::spawn_blocking(move || {
                loader.poll();
                loader
            })
            .await
            {
                Ok(l) => l,
                Err(e) => {
                    // Une panique dans `poll` emporte le chargeur ; le jeu courant reste en service.
                    tracing::error!(error = %e, "reference reload task stopped");
                    return;
                }
            };
        }
    })
}
