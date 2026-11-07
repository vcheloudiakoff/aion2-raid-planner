//! Commandes : chaque écriture ouvre une transaction sur le pool d'écriture, applique les règles de
//! `domain` avec le `ReferenceSet` pris une fois, écrit et commit (AD-16).

pub mod add_slot;
pub mod cancel_event;
pub mod create_event;
pub mod lock_event;
pub mod remove_slot;
pub mod set_candidature_status;
pub mod submit_candidature;
pub mod unlock_event;
pub mod update_candidature;
pub mod update_event_settings;
pub mod withdraw_candidature;

use crate::http::errors::{Code, Problem};

/// Échec d'une commande de l'organisateur sur son événement (réglages, créneaux, annulation,
/// verrouillage) : un refus rendu tel quel, ou une erreur SQL ou interne journalisée puis rendue en
/// 500 sans détail (AD-5).
pub(crate) enum EventWriteError {
    Refused(Problem),
    Db(sqlx::Error),
    /// Cause fixe, sans donnée : elle part seule dans le journal.
    Internal(&'static str),
}

impl From<sqlx::Error> for EventWriteError {
    fn from(e: sqlx::Error) -> Self {
        Self::Db(e)
    }
}

impl EventWriteError {
    pub(crate) fn refused(code: impl Into<Code>) -> Self {
        Self::Refused(code.into().into())
    }

    /// Le problème rendu au client ; l'erreur SQL part dans `tracing` avec l'id public de
    /// l'événement, jamais un secret (AD-15).
    pub(crate) fn into_problem(self, command: &'static str, event_id: &str) -> Problem {
        match self {
            Self::Refused(p) => p,
            Self::Db(e) => {
                tracing::error!(error = %e, event = event_id, command, "event command failed");
                Code::ServerInternal.into()
            }
            Self::Internal(cause) => {
                tracing::error!(cause, event = event_id, command, "event command failed");
                Code::ServerInternal.into()
            }
        }
    }
}
