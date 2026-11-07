//! `cancel_event` (CAP-5) : l'organisateur annule son événement, sous son lien `/o/` :
//! `open|locked → cancelled`, terminal (AD-4). Sans jeton de formulaire (AD-7).
//!
//! Tout se passe dans la transaction (AD-16) : relecture du statut, règle d'état (un événement déjà
//! annulé est `event.closed`), écriture de `status` et de `cancelled_at = clock.now()`, puis la vue
//! organisateur relue avant le commit. Aucune autre ligne ne change : les liens restent valides et
//! mènent à un événement en lecture seule jusqu'à la purge.

use domain::Timestamp;
use domain::event::{EventAction, event_may};
use domain::reference::ReferenceSet;
use sqlx::Connection;

use crate::app::AppState;
use crate::app::commands::EventWriteError as E;
use crate::app::queries::organizer_view;
use crate::http::dto::OrganizerEvent;
use crate::http::errors::{Code, Problem};
use crate::infra::events;

/// `event_id` vient de la ligne du lien organisateur.
pub async fn cancel_event(state: &AppState, event_id: &str) -> Result<OrganizerEvent, Problem> {
    let now = state.clock.now();
    let Some(reference) = state.reference.get() else {
        return Err(Code::ReferenceUnavailable.into());
    };
    let view = write(state, &reference, event_id, now)
        .await
        .map_err(|e| e.into_problem("cancel_event", event_id))?;
    tracing::info!(event = event_id, "event cancelled");
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
    let (status, _) = events::status_and_candidature_count(&mut tx, event_id)
        .await?
        .ok_or_else(|| E::refused(Code::AuthNotFound))?;
    event_may(status, EventAction::Cancel).map_err(E::refused)?;
    events::cancel(&mut tx, event_id, now).await?;
    let view = organizer_view::load(&mut tx, reference, event_id)
        .await?
        .ok_or_else(|| E::refused(Code::AuthNotFound))?;
    tx.commit().await?;
    Ok(view)
}
