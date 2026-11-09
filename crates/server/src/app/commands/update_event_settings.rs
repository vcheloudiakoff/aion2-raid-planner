//! `update_event_settings` (CAP-5) : l'organisateur remplace les réglages de son événement, sous son
//! lien `/o/`, en `open` comme en `locked` (AD-4 *Mutabilité*). Sans jeton de formulaire (AD-7).
//!
//! Tout se passe dans la transaction (AD-16) : relecture de l'événement, de ses bornes et du nombre
//! de ses candidatures, règle d'état, règles pures de `validate_settings` (champs figés dès la
//! première candidature, puis règles de la création sur les seules valeurs changées), écriture,
//! puis la vue organisateur relue avant le commit. Ni note ni lien Discord journalisé (AD-15).

use std::collections::BTreeMap;

use domain::event::{
    ClassBound, CurrentSettings, EventAction, NewSettings, event_may, validate_settings,
    validate_team_roles,
};
use domain::reference::ReferenceSet;
use sqlx::Connection;

use crate::app::AppState;
use crate::app::commands::EventWriteError as E;
use crate::app::queries::organizer_view;
use crate::http::dto::{OrganizerEvent, UpdateEventSettings};
use crate::http::errors::{Code, Problem};
use crate::infra::events;

/// `event_id` vient de la ligne du lien organisateur, jamais du corps.
pub async fn update_event_settings(
    state: &AppState,
    event_id: &str,
    body: UpdateEventSettings,
) -> Result<OrganizerEvent, Problem> {
    let Some(reference) = state.reference.get() else {
        return Err(Code::ReferenceUnavailable.into());
    };
    let team_roles = body.team_roles.clone();
    let view = write(state, &reference, event_id, body.into(), team_roles)
        .await
        .map_err(|e| e.into_problem("update_event_settings", event_id))?;
    tracing::info!(event = event_id, "event settings updated");
    Ok(view)
}

async fn write(
    state: &AppState,
    reference: &ReferenceSet,
    event_id: &str,
    input: NewSettings,
    team_roles: Option<Vec<BTreeMap<String, i64>>>,
) -> Result<OrganizerEvent, E> {
    let mut conn = state.db.write.acquire().await?;
    let mut tx = conn.begin().await?;
    let event = events::load(&mut tx, event_id)
        .await?
        .ok_or_else(|| E::refused(Code::AuthNotFound))?;
    event_may(event.status, EventAction::Settings).map_err(E::refused)?;
    let (_, candidatures) = events::status_and_candidature_count(&mut tx, event_id)
        .await?
        .ok_or_else(|| E::refused(Code::AuthNotFound))?;
    let current = CurrentSettings {
        instance: event.instance,
        region: event.region,
        faction_scope: event.faction_scope,
        has_candidatures: candidatures > 0,
        class_bounds: event
            .bounds
            .into_iter()
            .map(|b| ClassBound {
                class: b.class,
                min: b.min,
                max: b.max,
            })
            .collect(),
    };
    let settings = validate_settings(reference, &current, input)
        .map_err(|e| E::Refused(Problem::fields(e)))?;
    let team_roles = match team_roles {
        None => None,
        Some(t) => {
            let (parties, party_size) = reference
                .instances
                .iter()
                .find(|i| i.id == settings.instance)
                .map_or((1, 0), |i| (i.parties, i.party_size));
            Some(
                validate_team_roles(reference, parties, party_size, t)
                    .map_err(|e| E::Refused(Problem::fields(e)))?,
            )
        }
    };
    events::update_settings(&mut tx, event_id, &settings).await?;
    if let Some(t) = &team_roles {
        events::update_team_roles(&mut tx, event_id, t).await?;
    }
    let view = organizer_view::load(&mut tx, reference, event_id)
        .await?
        .ok_or_else(|| E::refused(Code::AuthNotFound))?;
    tx.commit().await?;
    Ok(view)
}
