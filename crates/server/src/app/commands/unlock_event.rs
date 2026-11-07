//! `unlock_event` (CAP-2 de l'epic 3) : l'organisateur ramène son événement `locked` en préparation,
//! sous son lien `/o/`, tant qu'un créneau est à venir (AD-4 *États*). Sans jeton de formulaire
//! (AD-7).
//!
//! Tout se passe dans la transaction (AD-16) : relecture de l'événement, règle d'état
//! (`event_may(Unlock)` : annulé = `event.closed`, déjà `open` = `event.not_locked`), puis au moins
//! un créneau à venir (`event.no_upcoming_slot`). Seul `status` change, vers `open` : groupes,
//! placements et noms de salle sont gardés, et reviennent au prochain verrouillage identique.

use domain::Timestamp;
use domain::event::{EventAction, EventStatus, event_may, has_upcoming_slot};
use domain::reference::ReferenceSet;
use sqlx::Connection;

use crate::app::AppState;
use crate::app::commands::EventWriteError as E;
use crate::app::queries::organizer_view;
use crate::http::dto::OrganizerEvent;
use crate::http::errors::{Code, Problem};
use crate::infra::events;

/// `event_id` vient de la ligne du lien organisateur.
pub async fn unlock_event(state: &AppState, event_id: &str) -> Result<OrganizerEvent, Problem> {
    let now = state.clock.now();
    let Some(reference) = state.reference.get() else {
        return Err(Code::ReferenceUnavailable.into());
    };
    let view = write(state, &reference, event_id, now)
        .await
        .map_err(|e| e.into_problem("unlock_event", event_id))?;
    tracing::info!(event = event_id, "event unlocked");
    Ok(view)
}

async fn write(
    state: &AppState,
    reference: &ReferenceSet,
    event_id: &str,
    now: Timestamp,
) -> Result<OrganizerEvent, E> {
    let mut conn = state.db.write.acquire().await?;
    let mut tx = conn.begin().await?;
    let event = events::load(&mut tx, event_id)
        .await?
        .ok_or_else(|| E::refused(Code::AuthNotFound))?;
    event_may(event.status, EventAction::Unlock).map_err(E::refused)?;
    let starts: Vec<Timestamp> = event.slots.iter().map(|s| s.starts_at).collect();
    if !has_upcoming_slot(now, &starts) {
        return Err(E::refused(Code::EventNoUpcomingSlot));
    }
    events::set_status(&mut tx, event_id, EventStatus::Open).await?;
    let view = organizer_view::load(&mut tx, reference, event_id)
        .await?
        .ok_or_else(|| E::refused(Code::AuthNotFound))?;
    tx.commit().await?;
    Ok(view)
}
