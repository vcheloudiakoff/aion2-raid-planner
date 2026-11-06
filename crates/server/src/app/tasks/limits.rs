//! Purge périodique des limiteurs (AD-7) : `retain_recent()` à chaque tick, pour qu'une rafale
//! d'adresses ne laisse pas une carte qui grossit sans borne.

use std::sync::Arc;
use std::time::Duration;

use tokio::task::JoinHandle;
use tokio::time::MissedTickBehavior;

use crate::http::limits::Limits;

/// Période de purge.
pub const TICK: Duration = Duration::from_secs(60);

/// Lance la boucle ; premier tick une période après l'appel.
pub fn spawn(limits: Arc<Limits>, period: Duration) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval_at(tokio::time::Instant::now() + period, period);
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            interval.tick().await;
            limits.retain_recent();
        }
    })
}
