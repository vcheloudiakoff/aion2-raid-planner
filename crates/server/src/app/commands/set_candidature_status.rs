//! `set_candidature_status` (CAP-5) : l'organisateur accepte ou refuse une candidature, sous son
//! lien `/o/`. Transitions d'AD-4 seulement : `pending → accepted|rejected`, `accepted ↔ rejected`,
//! en `open` comme en `locked`, jamais depuis `withdrawn` ni sur un événement annulé. Sans jeton de
//! formulaire (AD-7 : le secret organisateur ne se devine pas).
//!
//! Tout se passe dans la transaction (AD-16) : relecture de l'événement et de la candidature, que
//! l'on cherche dans l'événement du lien (une candidature d'un autre événement est
//! `candidature.not_found`), règle d'état, écriture du statut et de `updated_at`, puis la vue
//! organisateur relue avant le commit. Le statut courant redemandé n'écrit rien. Ni pseudo ni secret
//! journalisé (AD-15).

use domain::Timestamp;
use domain::candidature::{Decision, Transition, organizer_may};
use domain::reference::ReferenceSet;
use sqlx::Connection;

use crate::app::AppState;
use crate::app::queries::organizer_view;
use crate::http::dto::OrganizerEvent;
use crate::http::errors::{Code, Problem};
use crate::infra::{candidatures, events};

/// `event_id` vient de la ligne du lien organisateur ; `candidature_id` du corps, donc vérifié ici.
pub async fn set_candidature_status(
    state: &AppState,
    event_id: &str,
    candidature_id: &str,
    decision: Decision,
) -> Result<OrganizerEvent, Problem> {
    let now = state.clock.now();
    let Some(reference) = state.reference.get() else {
        return Err(Code::ReferenceUnavailable.into());
    };
    let (view, transition) = write(state, &reference, event_id, candidature_id, decision, now)
        .await
        .map_err(|e| match e {
            WriteError::Refused(p) => p,
            WriteError::Db(e) => {
                tracing::error!(
                    error = %e,
                    event = event_id,
                    candidature = candidature_id,
                    "set_candidature_status failed"
                );
                Code::ServerInternal.into()
            }
        })?;
    if transition == Transition::Change {
        tracing::info!(
            event = event_id,
            candidature = candidature_id,
            status = decision.status().as_str(),
            "candidature status set"
        );
    }
    Ok(view)
}

enum WriteError {
    Refused(Problem),
    Db(sqlx::Error),
}

impl From<sqlx::Error> for WriteError {
    fn from(e: sqlx::Error) -> Self {
        Self::Db(e)
    }
}

fn refused(code: impl Into<Code>) -> WriteError {
    WriteError::Refused(code.into().into())
}

async fn write(
    state: &AppState,
    reference: &ReferenceSet,
    event_id: &str,
    candidature_id: &str,
    decision: Decision,
    now: Timestamp,
) -> Result<(OrganizerEvent, Transition), WriteError> {
    let mut conn = state.db.write.acquire().await?;
    let mut tx = conn.begin().await?;
    let (event_status, _) = events::status_and_candidature_count(&mut tx, event_id)
        .await?
        .ok_or_else(|| refused(Code::AuthNotFound))?;
    let current = candidatures::load_in_event(&mut tx, event_id, candidature_id)
        .await?
        .ok_or_else(|| refused(Code::CandidatureNotFound))?;
    let transition = organizer_may(event_status, current, decision).map_err(refused)?;
    if transition == Transition::Change {
        // Aucune ligne `assignments` avant l'epic 3 : le refus n'a pas de placement à retirer ici.
        candidatures::set_status(&mut tx, candidature_id, decision.status(), now).await?;
    }
    let view = organizer_view::load(&mut tx, reference, event_id)
        .await?
        .ok_or_else(|| refused(Code::AuthNotFound))?;
    tx.commit().await?;
    Ok((view, transition))
}
