//! `regenerate_room_name` (CAP-4 de l'epic 3) : l'organisateur demande, sous son lien `/o/`, un
//! nouveau nom de salle pour un groupe retenu de son événement `locked`. Seule autre écriture du
//! nom après sa création au verrouillage (AD-18) ; sans jeton de formulaire (AD-7).
//!
//! Tout se passe dans la transaction (AD-16) : relecture de l'événement, règle d'état
//! (`event_may(RegenerateRoom)` : `open` = `event.not_locked`, annulé = `event.closed`, permis même
//! si le créneau a commencé), puis le groupe visé, qui doit être un groupe retenu de l'événement
//! (sinon 400 `placement.unknown_group` sur `groupId`). Le nouveau nom est composé depuis le
//! `room_label` figé du groupe ; le nom actuel compte comme pris, et cinq collisions donnent 500
//! sans rien écrire. Ni le nom ni un secret n'est journalisé (AD-15).

use domain::event::{EventAction, event_may};
use domain::reference::ReferenceSet;
use sqlx::Connection;

use crate::app::AppState;
use crate::app::commands::EventWriteError as E;
use crate::app::commands::room_name::free_room_name;
use crate::app::queries::organizer_view;
use crate::http::dto::OrganizerEvent;
use crate::http::errors::{Code, Problem};
use crate::infra::{events, groups};

/// `event_id` vient de la ligne du lien organisateur ; `group_id` du corps, donc cherché ici dans
/// l'événement.
pub async fn regenerate_room_name(
    state: &AppState,
    event_id: &str,
    group_id: &str,
) -> Result<OrganizerEvent, Problem> {
    let Some(reference) = state.reference.get() else {
        return Err(Code::ReferenceUnavailable.into());
    };
    let view = write(state, &reference, event_id, group_id)
        .await
        .map_err(|e| e.into_problem("regenerate_room_name", event_id))?;
    tracing::info!(event = event_id, group = group_id, "room name regenerated");
    Ok(view)
}

async fn write(
    state: &AppState,
    reference: &ReferenceSet,
    event_id: &str,
    group_id: &str,
) -> Result<OrganizerEvent, E> {
    let mut conn = state.db.write.acquire().await?;
    let mut tx = conn.begin().await?;
    let event = events::load(&mut tx, event_id)
        .await?
        .ok_or_else(|| E::refused(Code::AuthNotFound))?;
    event_may(event.status, EventAction::RegenerateRoom).map_err(E::refused)?;
    let group = groups::list(&mut tx, event_id)
        .await?
        .into_iter()
        .find(|g| g.id == group_id && g.retained)
        .ok_or_else(|| {
            E::Refused(Problem::fields(vec![(
                "groupId".to_owned(),
                Code::PlacementUnknownGroup,
            )]))
        })?;
    let name = free_room_name(state, &mut tx, &group.room_label, Some(&group.room_name)).await?;
    groups::set_room_name(&mut tx, event_id, &group.id, &name).await?;
    let view = organizer_view::load(&mut tx, reference, event_id)
        .await?
        .ok_or_else(|| E::refused(Code::AuthNotFound))?;
    tx.commit().await?;
    Ok(view)
}
