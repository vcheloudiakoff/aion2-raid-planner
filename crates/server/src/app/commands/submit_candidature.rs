//! `submit_candidature` (CAP-3) : dépose une candidature par `/e/` (`pending`, `public`) ou par `/j/`
//! (`accepted`, `direct`), et crée son lien participant, dans une seule transaction.
//!
//! Ordre (celui de `create_event`) : jeton de formulaire et honeypot, lecture de l'événement pour
//! valider contre sa région, sa portée, ses créneaux et ses bornes, validation pure, transaction sur
//! `db.write`, disjoncteur `Trip::Candidatures`, relecture du statut et du compte, insertions, commit,
//! puis comptage par le disjoncteur. Ni pseudo ni secret journalisé ; l'événement et la candidature
//! le sont par leur id public (AD-15).

use domain::Timestamp;
use domain::candidature::{
    CANDIDATURES_MAX, CandidatureStatus, EventContext, Origin, SlotRef, ValidCandidature,
    validate_candidature,
};
use domain::event::EventStatus;
use sqlx::Connection;

use crate::app::AppState;
use crate::app::queries::public_event;
use crate::http::dto::SubmitCandidature;
use crate::http::errors::{Code, Problem};
use crate::http::form_token::{FormClass, FormEnvelope};
use crate::http::limits::Trip;
use crate::infra::candidatures;
use crate::infra::events::{self, EventSnapshot};
use crate::infra::ids::{PublicId, Secret};
use crate::infra::links::{self, LinkRow, Right};

/// Ce que la candidature rend, une seule fois : les ids et le secret participant en clair.
#[derive(Debug)]
pub struct Submitted {
    pub event_id: String,
    pub candidature_id: PublicId,
    pub participant: Secret,
    pub status: CandidatureStatus,
}

/// `event_id` vient du chemin (`/e/`) ou de la ligne du lien direct (`/j/`), jamais du corps.
pub async fn submit_candidature(
    state: &AppState,
    event_id: &str,
    origin: Origin,
    body: FormEnvelope<SubmitCandidature>,
) -> Result<Submitted, Problem> {
    let now = state.clock.now();
    body.check(&state.form_tokens, FormClass::Candidature, now)?;

    let Some(reference) = state.reference.get() else {
        return Err(Code::ReferenceUnavailable.into());
    };
    let event = public_event::read(state, event_id).await?;
    // Un événement annulé refuse avant toute règle de champ ; la transaction le revérifie.
    if event.status == EventStatus::Cancelled {
        return Err(Code::EventClosed.into());
    }
    let candidature = validate_candidature(&reference, &context(&event), now, body.data.into())
        .map_err(Problem::fields)?;

    let submitted = Submitted {
        event_id: event.id,
        candidature_id: state.ids.public_id(),
        participant: state.ids.secret(),
        status: origin.initial_status(),
    };
    write(state, &submitted, origin, &candidature, now)
        .await
        .map_err(|e| match e {
            WriteError::Refused(p) => p,
            WriteError::Db(e) => {
                tracing::error!(error = %e, event = %submitted.event_id, "submit_candidature failed");
                Code::ServerInternal.into()
            }
        })?;
    tracing::info!(
        event = %submitted.event_id,
        candidature = %submitted.candidature_id,
        origin = origin.as_str(),
        "candidature submitted"
    );
    Ok(submitted)
}

fn context(event: &EventSnapshot) -> EventContext {
    EventContext {
        region: event.region.clone(),
        faction_scope: event.faction_scope,
        slots: event
            .slots
            .iter()
            .map(|s| SlotRef {
                id: s.id.clone(),
                starts_at: s.starts_at,
            })
            .collect(),
        classes: event.bounds.iter().map(|b| b.class.clone()).collect(),
    }
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

async fn write(
    state: &AppState,
    submitted: &Submitted,
    origin: Origin,
    candidature: &ValidCandidature,
    now: Timestamp,
) -> Result<(), WriteError> {
    // La connexion d'écriture unique reste tenue du `check` au `record` (voir `create_event`).
    let mut conn = state.db.write.acquire().await?;
    let mut tx = conn.begin().await?;
    state
        .limits
        .breaker
        .check(Trip::Candidatures)
        .map_err(WriteError::Refused)?;
    let event_id = submitted.event_id.as_str();
    match events::status_and_candidature_count(&mut tx, event_id).await? {
        None => return Err(WriteError::Refused(Code::EventNotFound.into())),
        // Une candidature en `locked` reste acceptée et non placée (AD-4).
        Some((EventStatus::Cancelled, _)) => {
            return Err(WriteError::Refused(Code::EventClosed.into()));
        }
        Some((_, n)) if n >= CANDIDATURES_MAX => {
            return Err(WriteError::Refused(Code::CandidatureLimitReached.into()));
        }
        Some(_) => {}
    }
    let candidature_id = submitted.candidature_id.as_str();
    candidatures::insert(
        &mut tx,
        candidature_id,
        event_id,
        candidature,
        submitted.status,
        origin,
        now,
    )
    .await?;
    let row = LinkRow {
        right: Right::Participant,
        event_id: event_id.to_owned(),
        candidature_id: Some(candidature_id.to_owned()),
    };
    links::insert(&mut tx, &submitted.participant, &row).await?;
    tx.commit().await?;
    state.limits.breaker.record(Trip::Candidatures);
    drop(conn);
    Ok(())
}
