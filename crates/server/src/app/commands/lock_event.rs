//! `lock_event` (CAP-2 de l'epic 3) : l'organisateur fixe la date de son événement `open`, sous son
//! lien `/o/`, avec la liste des créneaux retenus, un id par groupe (doublon permis). Sans jeton de
//! formulaire (AD-7).
//!
//! Tout se passe dans la transaction (AD-16) : relecture de l'événement, règle d'état
//! (`event_may(Lock)`), règles de la liste (`validate_lock` : 1 à 14 entrées, créneaux de
//! l'événement, à venir), instance de l'événement dans la référence (même retirée, AD-6), puis
//! l'appariement d'AD-4 (`lock_plan`) :
//! - un groupe existant au même `(créneau, occurrence)` est repris tel quel (disposition, nom,
//!   placements) et retenu ;
//! - une entrée sans groupe en crée un, avec `parties`, `party_size` et `room_label` figés depuis
//!   l'instance, et un nom composé depuis `ROOM_NAME_TEMPLATE` (AD-18) ; sur collision, un nouveau
//!   code, cinq essais en tout, puis 500 ;
//! - tout autre groupe d'un créneau à venir passe en sommeil, jamais supprimé ; un groupe d'un
//!   créneau commencé n'est pas touché.
//!
//! Puis `status = locked`, et la vue organisateur relue avant le commit. Les statuts des candidatures
//! ne changent pas, et les disponibilités ne sont pas vérifiées : un groupe incomplet est permis.
//! Aucun nom de salle ni pseudo dans le journal (AD-15).

use domain::Timestamp;
use domain::event::{
    EventAction, EventStatus, ExistingGroup, LockError, event_may, lock_plan, validate_lock,
};
use domain::reference::ReferenceSet;
use sqlx::Connection;

use crate::app::AppState;
use crate::app::commands::EventWriteError as E;
use crate::app::commands::room_name::free_room_name;
use crate::app::queries::organizer_view;
use crate::http::dto::OrganizerEvent;
use crate::http::errors::{Code, Problem};
use crate::infra::events;
use crate::infra::groups::{self, NewGroup};

/// `event_id` vient de la ligne du lien organisateur ; `slot_ids` du corps, donc vérifiés ici.
pub async fn lock_event(
    state: &AppState,
    event_id: &str,
    slot_ids: &[String],
) -> Result<OrganizerEvent, Problem> {
    let now = state.clock.now();
    let Some(reference) = state.reference.get() else {
        return Err(Code::ReferenceUnavailable.into());
    };
    let (view, created) = write(state, &reference, event_id, slot_ids, now)
        .await
        .map_err(|e| e.into_problem("lock_event", event_id))?;
    tracing::info!(
        event = event_id,
        groups = slot_ids.len(),
        created,
        "event locked"
    );
    Ok(view)
}

async fn write(
    state: &AppState,
    reference: &ReferenceSet,
    event_id: &str,
    slot_ids: &[String],
    now: Timestamp,
) -> Result<(OrganizerEvent, usize), E> {
    let mut conn = state.db.write.acquire().await?;
    let mut tx = conn.begin().await?;
    let event = events::load(&mut tx, event_id)
        .await?
        .ok_or_else(|| E::refused(Code::AuthNotFound))?;
    event_may(event.status, EventAction::Lock).map_err(E::refused)?;
    let slots: Vec<(&str, Timestamp)> = event
        .slots
        .iter()
        .map(|s| (s.id.as_str(), s.starts_at))
        .collect();
    let entries = validate_lock(now, &slots, slot_ids).map_err(|e| match e {
        LockError::GroupCountOutOfRange => E::refused(Code::EventGroupCountOutOfRange),
        LockError::SlotNotFound => E::refused(Code::SlotNotFound),
        LockError::SlotInPast(at) => E::Refused(Problem::fields(
            at.into_iter()
                .map(|i| (format!("slotIds.{i}"), Code::SlotInPast))
                .collect(),
        )),
    })?;
    // La disposition se lit dans l'instance de l'événement, `enabled` ou non : le verrouillage ne
    // pose pas l'instance (AD-6). Absente du jeu courant, rien n'est figé.
    let instance = reference
        .instances
        .iter()
        .find(|i| i.id == event.instance)
        .ok_or_else(|| E::refused(Code::ReferenceUnavailable))?;

    let upcoming = |slot_id: &str| {
        slots
            .iter()
            .any(|&(id, start)| id == slot_id && start > now)
    };
    let existing: Vec<ExistingGroup> = groups::list(&mut tx, event_id)
        .await?
        .into_iter()
        .map(|g| ExistingGroup {
            upcoming: upcoming(&g.slot_id),
            id: g.id,
            slot_id: g.slot_id,
            occurrence: g.occurrence,
        })
        .collect();
    let plan = lock_plan(&entries, &existing);
    for id in &plan.reuse {
        groups::set_retained(&mut tx, event_id, id, true).await?;
    }
    for id in &plan.sleep {
        groups::set_retained(&mut tx, event_id, id, false).await?;
    }
    for entry in &plan.create {
        let room_name = free_room_name(state, &mut tx, &instance.room_label, None).await?;
        let id = state.ids.public_id();
        let group = NewGroup {
            id: id.as_str(),
            slot_id: &entry.slot_id,
            occurrence: entry.occurrence,
            parties: instance.parties,
            party_size: instance.party_size,
            room_label: &instance.room_label,
            room_name: &room_name,
        };
        groups::insert(&mut tx, event_id, &group, now).await?;
    }
    events::set_status(&mut tx, event_id, EventStatus::Locked).await?;
    let view = organizer_view::load(&mut tx, reference, event_id)
        .await?
        .ok_or_else(|| E::refused(Code::AuthNotFound))?;
    tx.commit().await?;
    Ok((view, plan.create.len()))
}
