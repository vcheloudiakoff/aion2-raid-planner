//! `participant_view` (CAP-4) : la candidature vue par son lien personnel `/p/`. Sa seule candidature
//! et l'événement public, jamais une autre candidature ni un autre nom ; `discordUrl` seulement si
//! la candidature est `accepted` (AD-4 *Données exposées*).

use domain::candidature::CandidatureStatus;
use domain::reference::ReferenceSet;
use sqlx::SqliteConnection;

use crate::app::AppState;
use crate::app::queries::public_event;
use crate::http::dto::{ParticipantAvailability, ParticipantView};
use crate::http::errors::{Code, Problem};
use crate::infra::{candidatures, events};

/// Lecture sur le pool de lecture. Une candidature ou un événement disparus (purge) donnent
/// `auth.not_found` : pour le porteur, le lien ne mène plus à rien.
pub async fn participant_view(
    state: &AppState,
    event_id: &str,
    candidature_id: &str,
) -> Result<ParticipantView, Problem> {
    let Some(reference) = state.reference.get() else {
        return Err(Code::ReferenceUnavailable.into());
    };
    let loaded = async {
        let mut conn = state.db.read.acquire().await?;
        load(&mut conn, &reference, event_id, candidature_id).await
    }
    .await;
    match loaded {
        Ok(Some(view)) => Ok(view),
        Ok(None) => Err(Code::AuthNotFound.into()),
        Err(e) => {
            tracing::error!(
                error = %e,
                event = event_id,
                candidature = candidature_id,
                "participant view failed"
            );
            Err(Code::ServerInternal.into())
        }
    }
}

/// Construit la vue sur une connexion : celle du pool de lecture, ou la transaction d'une commande
/// avant son commit (la réponse rend alors l'agrégat tel qu'écrit, AD-16). `None` si l'événement
/// ou la candidature n'existe pas.
pub async fn load(
    conn: &mut SqliteConnection,
    reference: &ReferenceSet,
    event_id: &str,
    candidature_id: &str,
) -> Result<Option<ParticipantView>, sqlx::Error> {
    let Some(snapshot) = events::load(conn, event_id).await? else {
        return Ok(None);
    };
    let Some(c) = candidatures::load(conn, event_id, candidature_id).await? else {
        return Ok(None);
    };
    let discord_url = if c.status == CandidatureStatus::Accepted {
        events::discord_url(conn, event_id).await?.flatten()
    } else {
        None
    };
    Ok(Some(ParticipantView {
        event_id: c.event_id,
        candidature_id: c.id,
        pseudo: c.pseudo,
        server: c.server,
        class: c.class,
        cp: c.cp,
        status: c.status,
        availabilities: c
            .availabilities
            .into_iter()
            .map(|a| ParticipantAvailability {
                slot_id: a.slot_id,
                preferred: a.preferred,
            })
            .collect(),
        event: public_event::build(reference, snapshot),
        discord_url,
    }))
}
