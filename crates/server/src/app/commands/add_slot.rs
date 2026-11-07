//! `add_slot` (CAP-5) : l'organisateur ajoute un créneau à son événement `open`, sous son lien
//! `/o/` (AD-4 *Mutabilité*). Sans jeton de formulaire (AD-7).
//!
//! Tout se passe dans la transaction (AD-16) : relecture de l'événement et de ses créneaux, règle
//! d'état, règles de la création (`validate_added_slot` : début futur et distinct, durée de
//! l'instance par défaut, 14 créneaux au plus, passés compris), insertion avec un id de
//! l'`IdGenerator`, puis la vue organisateur relue avant le commit.

use domain::Timestamp;
use domain::event::{EventAction, NewSlot, NewSlotError, event_may, validate_added_slot};
use domain::reference::ReferenceSet;
use sqlx::Connection;

use crate::app::AppState;
use crate::app::commands::EventWriteError as E;
use crate::app::queries::organizer_view;
use crate::http::dto::{self, OrganizerEvent};
use crate::http::errors::{Code, Problem};
use crate::infra::events;
use crate::infra::ids::PublicId;

/// `event_id` vient de la ligne du lien organisateur, jamais du corps.
pub async fn add_slot(
    state: &AppState,
    event_id: &str,
    body: dto::NewSlot,
) -> Result<OrganizerEvent, Problem> {
    let now = state.clock.now();
    let Some(reference) = state.reference.get() else {
        return Err(Code::ReferenceUnavailable.into());
    };
    let (view, slot_id) = write(state, &reference, event_id, body.into(), now)
        .await
        .map_err(|e| e.into_problem("add_slot", event_id))?;
    tracing::info!(event = event_id, slot = %slot_id, "slot added");
    Ok(view)
}

async fn write(
    state: &AppState,
    reference: &ReferenceSet,
    event_id: &str,
    input: NewSlot,
    now: Timestamp,
) -> Result<(OrganizerEvent, PublicId), E> {
    let mut conn = state.db.write.acquire().await?;
    let mut tx = conn.begin().await?;
    let event = events::load(&mut tx, event_id)
        .await?
        .ok_or_else(|| E::refused(Code::AuthNotFound))?;
    event_may(event.status, EventAction::AddSlot).map_err(E::refused)?;
    // La durée de l'instance de l'événement, `enabled` ou non : l'ajout ne pose pas l'instance.
    let duration = reference
        .instances
        .iter()
        .find(|i| i.id == event.instance)
        .map(|i| i.duration_min);
    let existing: Vec<Timestamp> = event.slots.iter().map(|s| s.starts_at).collect();
    let slot = validate_added_slot(now, &existing, duration, input).map_err(|e| match e {
        NewSlotError::CountOutOfRange => E::refused(Code::SlotCountOutOfRange),
        NewSlotError::Fields(fields) => E::Refused(Problem::fields(fields)),
    })?;
    let slot_id = state.ids.public_id();
    events::insert_slot(&mut tx, event_id, slot_id.as_str(), &slot).await?;
    let view = organizer_view::load(&mut tx, reference, event_id)
        .await?
        .ok_or_else(|| E::refused(Code::AuthNotFound))?;
    tx.commit().await?;
    Ok((view, slot_id))
}
