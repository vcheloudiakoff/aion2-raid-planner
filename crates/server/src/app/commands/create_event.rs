//! `create_event` (CAP-1) : publie un événement en `open` et crée ses liens organisateur et
//! candidature directe, dans une seule transaction.
//!
//! Ordre : jeton de formulaire et honeypot, validation pure, transaction sur `db.write`, disjoncteur
//! `Trip::Events`, insertions, commit, puis comptage par le disjoncteur. Le pool d'écriture à une seule
//! connexion sérialise les transactions : `check` et `record` forment une paire atomique (voir
//! `Breaker`). Les secrets ne sont jamais journalisés ; l'événement l'est par son id public (AD-15).

use domain::Timestamp;
use domain::event::{ValidEvent, validate_new_event};
use sqlx::Connection;

use crate::app::AppState;
use crate::http::dto::CreateEvent;
use crate::http::errors::{Code, Problem};
use crate::http::form_token::{FormClass, FormEnvelope};
use crate::http::limits::Trip;
use crate::infra::events;
use crate::infra::ids::{PublicId, Secret};
use crate::infra::links::{self, LinkRow, Right};

/// Ce que la création rend, une seule fois : l'id public et les deux secrets en clair.
#[derive(Debug)]
pub struct Created {
    pub event_id: PublicId,
    pub organizer: Secret,
    pub join: Secret,
}

pub async fn create_event(
    state: &AppState,
    body: FormEnvelope<CreateEvent>,
) -> Result<Created, Problem> {
    let now = state.clock.now();
    body.check(&state.form_tokens, FormClass::Create, now)?;

    let Some(reference) = state.reference.get() else {
        return Err(Code::ReferenceUnavailable.into());
    };
    let event = validate_new_event(&reference, now, body.data.into()).map_err(Problem::fields)?;

    let created = Created {
        event_id: state.ids.public_id(),
        organizer: state.ids.secret(),
        join: state.ids.secret(),
    };
    let slot_ids: Vec<String> = event
        .slots
        .iter()
        .map(|_| state.ids.public_id().to_string())
        .collect();

    write(state, &created, &slot_ids, &event, now)
        .await
        .map_err(|e| match e {
            WriteError::Paused(p) => p,
            WriteError::Db(e) => {
                tracing::error!(error = %e, "create_event failed");
                Code::ServerInternal.into()
            }
        })?;
    tracing::info!(event = %created.event_id, "event created");
    Ok(created)
}

enum WriteError {
    Paused(Problem),
    Db(sqlx::Error),
}

impl From<sqlx::Error> for WriteError {
    fn from(e: sqlx::Error) -> Self {
        Self::Db(e)
    }
}

async fn write(
    state: &AppState,
    created: &Created,
    slot_ids: &[String],
    event: &ValidEvent,
    now: Timestamp,
) -> Result<(), WriteError> {
    // La connexion d'écriture unique reste tenue du `check` au `record` : aucune autre commande ne
    // peut passer le disjoncteur entre le commit et le comptage.
    let mut conn = state.db.write.acquire().await?;
    let mut tx = conn.begin().await?;
    state
        .limits
        .breaker
        .check(Trip::Events)
        .map_err(WriteError::Paused)?;
    let event_id = created.event_id.as_str();
    events::insert(&mut tx, event_id, slot_ids, event, now).await?;
    for (secret, right) in [
        (&created.organizer, Right::Organizer),
        (&created.join, Right::Join),
    ] {
        let row = LinkRow {
            right,
            event_id: event_id.to_owned(),
            candidature_id: None,
        };
        links::insert(&mut tx, secret, &row).await?;
    }
    tx.commit().await?;
    state.limits.breaker.record(Trip::Events);
    drop(conn);
    Ok(())
}
