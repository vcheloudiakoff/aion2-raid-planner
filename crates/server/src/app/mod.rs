//! Commandes, requêtes et santé. Appelle `infra` directement, sans trait de dépôt.

pub mod health;
pub mod tasks;

use std::sync::Arc;

use sqlx::migrate::Migrator;

use crate::infra::clock::Clock;
use crate::infra::ids::IdGenerator;
use crate::infra::reference::ReferenceStore;
use crate::infra::sqlite::{Db, MIGRATOR};

/// État partagé par toutes les routes.
#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    pub clock: Arc<dyn Clock>,
    pub ids: Arc<dyn IdGenerator>,
    /// Jeu de référence courant, vide tant qu'aucun jeu valide n'a été chargé.
    pub reference: ReferenceStore,
    /// Migrations attendues par la santé : celles embarquées en production.
    pub migrator: &'static Migrator,
}

impl AppState {
    pub fn new(
        db: Db,
        clock: Arc<dyn Clock>,
        ids: Arc<dyn IdGenerator>,
        reference: ReferenceStore,
    ) -> Self {
        Self {
            db,
            clock,
            ids,
            reference,
            migrator: &MIGRATOR,
        }
    }
}
