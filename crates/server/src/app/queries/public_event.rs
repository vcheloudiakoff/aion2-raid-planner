//! `public_event` (CAP-2) : l'événement vu par le public, par son id (`/e/`) ou par le lien direct
//! (`/j/`). Aucune donnée nominative ; comptes par classe et par rôle calculés ici (AD-4, AD-6).

use domain::counts::{ClassMin, public_counts};
use domain::reference::ReferenceSet;

use crate::app::AppState;
use domain::candidature::CandidatureStatus;

use crate::app::queries::participant_view::roster_entry;
use crate::http::dto::{JoinEvent, MissingRole, PublicEvent, PublicSlot};
use crate::http::errors::{Code, Problem};
use crate::infra::candidatures;
use crate::infra::events::{self, EventSnapshot};

/// `event.not_found` si l'id est inconnu. Un événement `cancelled` reste lisible.
pub async fn public_event(state: &AppState, event_id: &str) -> Result<PublicEvent, Problem> {
    let Some(reference) = state.reference.get() else {
        return Err(Code::ReferenceUnavailable.into());
    };
    let snapshot = read(state, event_id).await?;
    Ok(build(&reference, snapshot))
}

/// `PublicEvent` d'un instantané : comptes par classe et par rôle, rôles manquants.
/// Page du lien direct `/j/` : l'événement public et le planning des candidatures `accepted`
/// (décision de Vincent du 2026-11-09 : la légion voit qui est inscrit avant de candidater). Le lien
/// direct n'est remis qu'aux joueurs invités par l'organisateur.
pub async fn join_event(state: &AppState, event_id: &str) -> Result<JoinEvent, Problem> {
    let Some(reference) = state.reference.get() else {
        return Err(Code::ReferenceUnavailable.into());
    };
    let snapshot = read(state, event_id).await?;
    let all = async {
        let mut conn = state.db.read.acquire().await?;
        candidatures::list(&mut conn, event_id).await
    }
    .await
    .map_err(|e| {
        tracing::error!(error = %e, event = event_id, "join roster read failed");
        Problem::from(Code::ServerInternal)
    })?;
    let accepted_roster = all
        .iter()
        .filter(|c| c.status == CandidatureStatus::Accepted)
        .map(|c| roster_entry(&reference, c))
        .collect();
    Ok(JoinEvent {
        event: build(&reference, snapshot),
        accepted_roster,
    })
}

pub fn build(reference: &ReferenceSet, snapshot: EventSnapshot) -> PublicEvent {
    let bounds: Vec<ClassMin> = snapshot
        .bounds
        .iter()
        .map(|b| ClassMin {
            class: b.class.clone(),
            min: b.min,
        })
        .collect();
    let counts = public_counts(reference, &bounds, &snapshot.counted_classes);
    PublicEvent {
        id: snapshot.id,
        instance: snapshot.instance,
        region: snapshot.region,
        faction_scope: snapshot.faction_scope,
        visibility: snapshot.visibility,
        status: snapshot.status,
        min_cp: snapshot.min_cp,
        note: snapshot.note,
        slots: snapshot
            .slots
            .into_iter()
            .map(|s| PublicSlot {
                id: s.id,
                starts_at: s.starts_at,
                duration_min: s.duration_min,
            })
            .collect(),
        counts_by_class: counts.by_class,
        counts_by_role: counts.by_role,
        missing_roles: counts
            .missing_roles
            .into_iter()
            .map(|m| MissingRole {
                role: m.role,
                missing: m.missing,
            })
            .collect(),
    }
}

/// L'événement sur le pool de lecture ; `event.not_found` s'il n'existe pas.
pub async fn read(state: &AppState, event_id: &str) -> Result<EventSnapshot, Problem> {
    let loaded = async {
        let mut conn = state.db.read.acquire().await?;
        events::load(&mut conn, event_id).await
    }
    .await;
    match loaded {
        Ok(Some(snapshot)) => Ok(snapshot),
        Ok(None) => Err(Code::EventNotFound.into()),
        Err(e) => {
            tracing::error!(error = %e, event = event_id, "event read failed");
            Err(Code::ServerInternal.into())
        }
    }
}
