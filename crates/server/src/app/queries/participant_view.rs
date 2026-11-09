//! `participant_view` (CAP-4, epic 3 CAP-6) : la candidature vue par son lien personnel `/p/`. Sa
//! candidature et l'événement public ; `discordUrl` seulement si la candidature est `accepted` (AD-4
//! *Données exposées*). En `locked`, pour une candidature `accepted` placée dans un groupe retenu,
//! sa place, le créneau, le nom de salle et ses coéquipiers visibles
//! (`domain::placement::{participant_placement, teammates}`), réduits à `Pseudo[SRV]`, classe et
//! équipe. En `open` après un déverrouillage, `backInPreparation`. Pour une candidature `accepted`,
//! en `open` comme en `locked`, la liste des acceptés de l'événement
//! (`domain::candidature::accepted_list`), réduits à `Pseudo[SRV]` et classe, et le rang du lecteur.

use std::collections::HashMap;

use domain::candidature::{CandidatureStatus, accepted_list};
use domain::event::EventStatus;
use domain::placement::{self, Assigned, GroupRef, Seat};
use domain::pseudo_srv::pseudo_srv;
use domain::reference::ReferenceSet;
use sqlx::SqliteConnection;

use crate::app::AppState;
use crate::app::queries::public_event;
use crate::http::dto::{
    Availability, ParticipantPlacement, ParticipantSeat, ParticipantView, RosterEntry, Teammate,
    TeammateParty,
};
use crate::http::errors::{Code, Problem};
use crate::infra::{candidatures, events, groups};

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
    // Une seule transaction de lecture : la candidature, les groupes et les placements voient le
    // même instantané WAL.
    let loaded = async {
        let mut tx = state.db.read.begin().await?;
        let view = load(&mut tx, &reference, event_id, candidature_id).await?;
        tx.commit().await?;
        Ok::<_, sqlx::Error>(view)
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
    let group_rows = groups::list(conn, event_id).await?;
    let all = candidatures::list(conn, event_id).await?;
    let back_in_preparation = c.status == CandidatureStatus::Accepted
        && snapshot.status == EventStatus::Open
        && !group_rows.is_empty();
    let (placement, teammates) = seat_and_teammates(
        conn,
        reference,
        &snapshot,
        &group_rows,
        &all,
        &c.id,
        c.status,
    )
    .await?;
    let (accepted_roster, accepted_roster_self) = match roster(reference, &all, &c.id) {
        Some((entries, own)) => (Some(entries), Some(own)),
        None => (None, None),
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
            .map(|a| Availability {
                slot_id: a.slot_id,
                preferred: a.preferred,
            })
            .collect(),
        teams: c.teams.teams,
        preferred_team: c.teams.preferred,
        event: public_event::build(reference, snapshot),
        discord_url,
        placement,
        teammates,
        back_in_preparation,
        accepted_roster,
        accepted_roster_self,
    }))
}

/// Liste des acceptés vue par la candidature `me` : `None` si elle n'est pas `accepted`
/// (`domain::candidature::accepted_list`), sinon chaque accepté réduit à `Pseudo[SRV]` et classe, et
/// le rang de `me` dans la liste.
fn roster(
    reference: &ReferenceSet,
    all: &[candidatures::CandidatureRow],
    me: &str,
) -> Option<(Vec<RosterEntry>, usize)> {
    let statuses: Vec<(&str, CandidatureStatus)> =
        all.iter().map(|c| (c.id.as_str(), c.status)).collect();
    let by_id: HashMap<&str, &candidatures::CandidatureRow> =
        all.iter().map(|c| (c.id.as_str(), c)).collect();
    let list = accepted_list(me, &statuses)?;
    // Les ids viennent de `all` : chacun a sa ligne, le rang `own` reste juste.
    let entries = list
        .ids
        .into_iter()
        .map(|id| {
            let c = by_id[id];
            RosterEntry {
                pseudo_srv: pseudo_srv(reference, &c.pseudo, &c.server),
                class: c.class.clone(),
            }
        })
        .collect();
    Some((entries, list.own))
}

/// Place de la candidature `me` dans un groupe retenu, et ses coéquipiers visibles ; `(None, None)`
/// hors des conditions de `domain::placement::participant_placement` (événement `locked`,
/// candidature `accepted`, placée dans un groupe retenu).
async fn seat_and_teammates(
    conn: &mut SqliteConnection,
    reference: &ReferenceSet,
    snapshot: &events::EventSnapshot,
    group_rows: &[groups::GroupRow],
    others: &[candidatures::CandidatureRow],
    me: &str,
    status: CandidatureStatus,
) -> Result<(Option<ParticipantPlacement>, Option<Vec<TeammateParty>>), sqlx::Error> {
    let rows = groups::assignments(conn, &snapshot.id).await?;
    let by_id: HashMap<&str, &candidatures::CandidatureRow> =
        others.iter().map(|c| (c.id.as_str(), c)).collect();
    let assigned: Vec<Assigned<'_>> = rows
        .iter()
        .filter_map(|a| {
            by_id.get(a.candidature_id.as_str()).map(|c| Assigned {
                candidature_id: &a.candidature_id,
                status: c.status,
                group_id: &a.group_id,
                seat: a.seat,
            })
        })
        .collect();
    let refs: Vec<GroupRef<'_>> = group_rows
        .iter()
        .map(|g| GroupRef {
            id: &g.id,
            retained: g.retained,
        })
        .collect();
    let locked = snapshot.status == EventStatus::Locked;
    let Some(mine) = placement::participant_placement(me, status, locked, &refs, &assigned) else {
        return Ok((None, None));
    };
    let decode = |what: &str| sqlx::Error::Decode(format!("placement without its {what}").into());
    let group = group_rows
        .iter()
        .find(|g| g.id == mine.group_id)
        .ok_or_else(|| decode("group"))?;
    let slot = snapshot
        .slots
        .iter()
        .find(|s| s.id == group.slot_id)
        .ok_or_else(|| decode("slot"))?;
    let teammates = placement::teammates(me, &mine, group.parties, &assigned)
        .into_iter()
        .map(|t| TeammateParty {
            party: t.party,
            members: t
                .members
                .into_iter()
                .filter_map(|id| by_id.get(id))
                .map(|c| Teammate {
                    pseudo_srv: pseudo_srv(reference, &c.pseudo, &c.server),
                    class: c.class.clone(),
                })
                .collect(),
        })
        .collect();
    let seat = match mine.seat {
        Seat::Starter { party, index } => ParticipantSeat::Starter { party, index },
        Seat::Bench { bench_index } => ParticipantSeat::Bench { bench_index },
    };
    Ok((
        Some(ParticipantPlacement {
            group_number: mine.group_number,
            slot_id: slot.id.clone(),
            starts_at: slot.starts_at,
            room_name: group.room_name.clone(),
            seat,
        }),
        Some(teammates),
    ))
}
