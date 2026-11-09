//! `set_teams` : le participant change ses Équipes souhaitées sous son lien `/p/`, en `open` comme
//! en `locked` (décision de Vincent du 2026-11-09 : un raid verrouillé doit pouvoir les recevoir).
//! Rien d'autre ne change ; le placement reste tel quel. Sans jeton de formulaire (AD-7), règles
//! vérifiées dans la transaction après relecture (AD-16).

use domain::Timestamp;
use domain::candidature::{ParticipantAction, TeamChoice, participant_may, validate_teams};
use domain::reference::ReferenceSet;
use sqlx::Connection;

use crate::app::AppState;
use crate::app::queries::participant_view;
use crate::http::dto::ParticipantView;
use crate::http::errors::{Code, Problem};
use crate::infra::{candidatures, events};

/// `event_id` et `candidature_id` viennent de la ligne du lien participant.
pub async fn set_teams(
    state: &AppState,
    event_id: &str,
    candidature_id: &str,
    input: TeamChoice,
) -> Result<ParticipantView, Problem> {
    let now = state.clock.now();
    let Some(reference) = state.reference.get() else {
        return Err(Code::ReferenceUnavailable.into());
    };
    write(state, &reference, event_id, candidature_id, input, now)
        .await
        .map_err(|e| match e {
            WriteError::Refused(p) => p,
            WriteError::Db(e) => {
                tracing::error!(error = %e, event = event_id, "set_teams failed");
                Code::ServerInternal.into()
            }
        })
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
    input: TeamChoice,
    now: Timestamp,
) -> Result<ParticipantView, WriteError> {
    let mut conn = state.db.write.acquire().await?;
    let mut tx = conn.begin().await?;
    let event = events::load(&mut tx, event_id)
        .await?
        .ok_or_else(|| refused(Code::AuthNotFound))?;
    let current = candidatures::load(&mut tx, event_id, candidature_id)
        .await?
        .ok_or_else(|| refused(Code::AuthNotFound))?;
    participant_may(event.status, current.status, ParticipantAction::Teams).map_err(refused)?;
    let parties = reference
        .instances
        .iter()
        .find(|i| i.id == event.instance)
        .map_or(1, |i| i.parties);
    let teams =
        validate_teams(parties, &input).map_err(|e| WriteError::Refused(Problem::fields(e)))?;
    candidatures::set_teams(&mut tx, candidature_id, &teams, now).await?;
    let view = participant_view::load(&mut tx, reference, event_id, candidature_id)
        .await?
        .ok_or_else(|| refused(Code::AuthNotFound))?;
    tx.commit().await?;
    Ok(view)
}
