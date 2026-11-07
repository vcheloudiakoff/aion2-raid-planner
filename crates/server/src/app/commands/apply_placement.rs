//! `apply_placement` (CAP-3 de l'epic 3) : l'organisateur enregistre, sous son lien `/o/`, le
//! placement complet des groupes retenus de son événement `locked`. Seul writer des `assignments`
//! (AD-4) ; sans jeton de formulaire (AD-7) ; la dernière écriture gagne (AD-16).
//!
//! Tout se passe dans la transaction : relecture de l'événement, règle d'état
//! (`event_may(Place)` : `open` = `event.not_locked`, annulé = `event.closed`), puis les violations
//! structurelles de `domain::placement::validate` (400 `placement.*` avec les champs fautifs). Une
//! indisponibilité ou une borne de classe n'est jamais refusée : la vue relue les porte en
//! avertissements. Le remplacement supprime les placements des groupes retenus et ceux des
//! candidatures du corps (un groupe en sommeil perd ainsi la candidature déplacée), puis insère le
//! corps, banc renuméroté ; les autres placements des groupes en sommeil sont gardés (décision de
//! Vincent sur les groupes en sommeil). Ni pseudo ni secret journalisé (AD-15).

use domain::event::{EventAction, event_may};
use domain::placement::{LayoutGroup, PlacementInput, validate};
use domain::reference::ReferenceSet;
use sqlx::Connection;

use crate::app::AppState;
use crate::app::commands::EventWriteError as E;
use crate::app::queries::organizer_view;
use crate::http::dto::OrganizerEvent;
use crate::http::errors::{Code, Problem};
use crate::infra::{candidatures, events, groups};

/// `event_id` vient de la ligne du lien organisateur ; les ids des entrées viennent du corps, donc
/// vérifiés ici contre l'événement.
pub async fn apply_placement(
    state: &AppState,
    event_id: &str,
    entries: &[PlacementInput],
) -> Result<OrganizerEvent, Problem> {
    let Some(reference) = state.reference.get() else {
        return Err(Code::ReferenceUnavailable.into());
    };
    let view = write(state, &reference, event_id, entries)
        .await
        .map_err(|e| e.into_problem("apply_placement", event_id))?;
    tracing::info!(
        event = event_id,
        placements = entries.len(),
        "placement applied"
    );
    Ok(view)
}

async fn write(
    state: &AppState,
    reference: &ReferenceSet,
    event_id: &str,
    entries: &[PlacementInput],
) -> Result<OrganizerEvent, E> {
    let mut conn = state.db.write.acquire().await?;
    let mut tx = conn.begin().await?;
    let event = events::load(&mut tx, event_id)
        .await?
        .ok_or_else(|| E::refused(Code::AuthNotFound))?;
    event_may(event.status, EventAction::Place).map_err(E::refused)?;
    let statuses = candidatures::statuses(&mut tx, event_id).await?;
    let statuses: Vec<_> = statuses.iter().map(|(id, s)| (id.as_str(), *s)).collect();
    let rows = groups::list(&mut tx, event_id).await?;
    let layout: Vec<LayoutGroup<'_>> = rows
        .iter()
        .map(|g| LayoutGroup {
            id: &g.id,
            retained: g.retained,
            parties: g.parties,
            party_size: g.party_size,
        })
        .collect();
    let placements =
        validate(entries, &statuses, &layout).map_err(|e| E::Refused(Problem::fields(e)))?;
    groups::replace_assignments(&mut tx, event_id, &placements).await?;
    let view = organizer_view::load(&mut tx, reference, event_id)
        .await?
        .ok_or_else(|| E::refused(Code::AuthNotFound))?;
    tx.commit().await?;
    Ok(view)
}
