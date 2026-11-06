//! Source unique de la santé, utilisée par `GET /api/health` et donc par la sous-commande
//! `healthcheck` (AD-8).

use super::AppState;
use crate::infra::sqlite::migrations_up_to_date;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Health {
    Ok,
    Unavailable,
}

/// Base ouverte, toutes les migrations embarquées appliquées et référence chargée. La cause d'un
/// échec part dans `tracing` au niveau `warn`, jamais dans la réponse.
pub async fn health(state: &AppState) -> Health {
    if state.db.is_closed() {
        tracing::warn!("health: database pool closed");
        return Health::Unavailable;
    }
    if state.reference.get().is_none() {
        tracing::warn!("health: reference not loaded");
        return Health::Unavailable;
    }
    match migrations_up_to_date(&state.db.read, state.migrator).await {
        Ok(()) => Health::Ok,
        Err(e) => {
            tracing::warn!(error = %e, "health: migrations not up to date");
            Health::Unavailable
        }
    }
}
