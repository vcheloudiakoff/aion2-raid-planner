//! `remove_slot` (CAP-5) : l'organisateur supprime un créneau de son événement `open`, sous son lien
//! `/o/` (AD-4 *Mutabilité*). Sans jeton de formulaire (AD-7).
//!
//! Tout se passe dans la transaction (AD-16) : relecture de l'événement et de ses créneaux, règle
//! d'état, créneau cherché dans l'événement du lien (inconnu ou d'un autre événement :
//! `slot.not_found`), au moins un créneau gardé, suppression, puis la vue organisateur relue avant
//! le commit. Un créneau passé se supprime aussi. Ses `availabilities` partent par la cascade
//! déclarée ; les statuts des candidatures ne changent jamais, et une candidature restée sans
//! disponibilité est gardée. Aucun groupe avant l'epic 3.

use domain::event::{EventAction, event_may, slot_count_allowed};
use domain::reference::ReferenceSet;
use sqlx::Connection;

use crate::app::AppState;
use crate::app::commands::EventWriteError as E;
use crate::app::queries::organizer_view;
use crate::http::dto::OrganizerEvent;
use crate::http::errors::{Code, Problem};
use crate::infra::events;

/// `event_id` vient de la ligne du lien organisateur ; `slot_id` du corps, donc vérifié ici.
pub async fn remove_slot(
    state: &AppState,
    event_id: &str,
    slot_id: &str,
) -> Result<OrganizerEvent, Problem> {
    let Some(reference) = state.reference.get() else {
        return Err(Code::ReferenceUnavailable.into());
    };
    let view = write(state, &reference, event_id, slot_id)
        .await
        .map_err(|e| e.into_problem("remove_slot", event_id))?;
    tracing::info!(event = event_id, slot = slot_id, "slot removed");
    Ok(view)
}

async fn write(
    state: &AppState,
    reference: &ReferenceSet,
    event_id: &str,
    slot_id: &str,
) -> Result<OrganizerEvent, E> {
    let mut conn = state.db.write.acquire().await?;
    let mut tx = conn.begin().await?;
    let event = events::load(&mut tx, event_id)
        .await?
        .ok_or_else(|| E::refused(Code::AuthNotFound))?;
    event_may(event.status, EventAction::RemoveSlot).map_err(E::refused)?;
    if !event.slots.iter().any(|s| s.id == slot_id) {
        return Err(E::refused(Code::SlotNotFound));
    }
    if !slot_count_allowed(event.slots.len() - 1) {
        return Err(E::refused(Code::SlotCountOutOfRange));
    }
    if !events::delete_slot(&mut tx, event_id, slot_id).await? {
        return Err(E::refused(Code::SlotNotFound));
    }
    let view = organizer_view::load(&mut tx, reference, event_id)
        .await?
        .ok_or_else(|| E::refused(Code::AuthNotFound))?;
    tx.commit().await?;
    Ok(view)
}
