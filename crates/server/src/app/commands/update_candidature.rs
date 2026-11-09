//! `update_candidature` (CAP-4) : le participant remplace le contenu de sa candidature, sous son
//! lien `/p/`. Sans jeton de formulaire (AD-7 : le secret participant ne se devine pas).
//!
//! Tout se passe dans la transaction (AD-16) : relecture de l'événement, de ses créneaux et de la
//! candidature, règles d'état (annulé, retiré, verrouillé), validation en mode modification,
//! écriture, puis la vue participant relue avant le commit. Une disponibilité vers un créneau
//! disparu est donc `candidature.slot_unknown`, jamais une violation de clé étrangère. `status` et
//! `origin` ne changent jamais. Ni pseudo ni secret journalisé (AD-15).

use domain::Timestamp;
use domain::candidature::{
    CurrentCandidature, NewCandidature, ParticipantAction, participant_may, validate_update,
};
use domain::reference::ReferenceSet;
use sqlx::Connection;

use crate::app::AppState;
use crate::app::commands::submit_candidature::context;
use crate::app::queries::participant_view;
use crate::http::dto::{ParticipantView, SubmitCandidature};
use crate::http::errors::{Code, Problem};
use crate::infra::{candidatures, events};

/// `event_id` et `candidature_id` viennent de la ligne du lien participant, jamais du corps.
pub async fn update_candidature(
    state: &AppState,
    event_id: &str,
    candidature_id: &str,
    body: SubmitCandidature,
) -> Result<ParticipantView, Problem> {
    let now = state.clock.now();
    let Some(reference) = state.reference.get() else {
        return Err(Code::ReferenceUnavailable.into());
    };
    let view = write(
        state,
        &reference,
        event_id,
        candidature_id,
        body.into(),
        now,
    )
    .await
    .map_err(|e| match e {
        WriteError::Refused(p) => p,
        WriteError::Db(e) => {
            tracing::error!(
                error = %e,
                event = event_id,
                candidature = candidature_id,
                "update_candidature failed"
            );
            Code::ServerInternal.into()
        }
    })?;
    tracing::info!(
        event = event_id,
        candidature = candidature_id,
        "candidature updated"
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
    input: NewCandidature,
    now: Timestamp,
) -> Result<ParticipantView, WriteError> {
    let mut conn = state.db.write.acquire().await?;
    let mut tx = conn.begin().await?;
    // Événement ou candidature purgés : le lien ne mène plus à rien.
    let event = events::load(&mut tx, event_id)
        .await?
        .ok_or_else(|| refused(Code::AuthNotFound))?;
    let current = candidatures::load(&mut tx, event_id, candidature_id)
        .await?
        .ok_or_else(|| refused(Code::AuthNotFound))?;
    participant_may(event.status, current.status, ParticipantAction::Update).map_err(refused)?;
    let current = CurrentCandidature {
        server: current.server,
        class: current.class,
        availabilities: current.availabilities,
    };
    let update = validate_update(reference, &context(&event, reference), now, &current, input)
        .map_err(|e| WriteError::Refused(Problem::fields(e)))?;
    candidatures::update_content(
        &mut tx,
        candidature_id,
        &update.candidature,
        &update.replaced,
        now,
    )
    .await?;
    let view = participant_view::load(&mut tx, reference, event_id, candidature_id)
        .await?
        .ok_or_else(|| refused(Code::AuthNotFound))?;
    tx.commit().await?;
    Ok(view)
}
