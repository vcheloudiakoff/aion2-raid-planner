//! Commandes, requêtes et santé. Appelle `infra` directement, sans trait de dépôt.

pub mod commands;
pub mod health;
pub mod queries;
pub mod tasks;

use std::sync::Arc;

use sqlx::migrate::Migrator;

use crate::config::Config;
use crate::http::form_token::FormTokens;
use crate::http::limits::Limits;
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
    /// Limiteurs par IP et disjoncteur (AD-7), en mémoire.
    pub limits: Arc<Limits>,
    /// Émetteur et vérificateur du jeton de formulaire (AD-7).
    pub form_tokens: Arc<FormTokens>,
}

impl AppState {
    pub fn new(
        db: Db,
        clock: Arc<dyn Clock>,
        ids: Arc<dyn IdGenerator>,
        reference: ReferenceStore,
        config: &Config,
    ) -> Self {
        let limits = Arc::new(Limits::new(config, clock.clone()));
        let form_tokens = Arc::new(FormTokens::new(
            config.form_token_secret.clone(),
            config.min_submit_delay,
        ));
        Self {
            db,
            clock,
            ids,
            reference,
            migrator: &MIGRATOR,
            limits,
            form_tokens,
        }
    }
}
