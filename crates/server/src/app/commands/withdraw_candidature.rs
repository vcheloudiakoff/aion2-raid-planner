//! `withdraw_candidature` (CAP-4) : le participant retire sa candidature, sous son lien `/p/`.
//! `pending` ou `accepted` → `withdrawn`, terminal, en `open` comme en `locked` (AD-4). Sans jeton de
//! formulaire (AD-7). Règles d'état vérifiées dans la transaction après relecture (AD-16) ; le
//! placement de la candidature est retiré dans la même transaction (AD-4) ; la vue participant est
//! relue avant le commit. Ni pseudo ni secret journalisé (AD-15).

use domain::Timestamp;
use domain::candidature::{CandidatureStatus, ParticipantAction, participant_may};
use domain::reference::ReferenceSet;
use sqlx::Connection;

use crate::app::AppState;
use crate::app::queries::participant_view;
use crate::http::dto::ParticipantView;
use crate::http::errors::{Code, Problem};
use crate::infra::{candidatures, events, groups};

/// `event_id` et `candidature_id` viennent de la ligne du lien participant.
pub async fn withdraw_candidature(
    state: &AppState,
    event_id: &str,
    candidature_id: &str,
) -> Result<ParticipantView, Problem> {
    let now = state.clock.now();
    let Some(reference) = state.reference.get() else {
        return Err(Code::ReferenceUnavailable.into());
    };
    let view = write(state, &reference, event_id, candidature_id, now)
        .await
        .map_err(|e| match e {
            WriteError::Refused(p) => p,
            WriteError::Db(e) => {
                tracing::error!(
                    error = %e,
                    event = event_id,
                    candidature = candidature_id,
                    "withdraw_candidature failed"
                );
                Code::ServerInternal.into()
            }
        })?;
    tracing::info!(
        event = event_id,
        candidature = candidature_id,
        "candidature withdrawn"
    );
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
    now: Timestamp,
) -> Result<ParticipantView, WriteError> {
    let mut conn = state.db.write.acquire().await?;
    let mut tx = conn.begin().await?;
    let event = events::status_and_candidature_count(&mut tx, event_id)
        .await?
        .ok_or_else(|| refused(Code::AuthNotFound))?;
    let current = candidatures::load(&mut tx, event_id, candidature_id)
        .await?
        .ok_or_else(|| refused(Code::AuthNotFound))?;
    participant_may(event.0, current.status, ParticipantAction::Withdraw).map_err(refused)?;
    // Le retrait emporte le placement dans la même transaction (AD-4).
    groups::delete_assignment(&mut tx, candidature_id).await?;
    candidatures::set_status(&mut tx, candidature_id, CandidatureStatus::Withdrawn, now).await?;
    let view = participant_view::load(&mut tx, reference, event_id, candidature_id)
        .await?
        .ok_or_else(|| refused(Code::AuthNotFound))?;
    tx.commit().await?;
    Ok(view)
}
