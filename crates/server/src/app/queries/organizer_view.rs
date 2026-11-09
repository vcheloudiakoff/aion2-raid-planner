//! `organizer_view` (CAP-5) : l'événement vu par son lien organisateur `/o/`. Le `PublicEvent`, le
//! lien Discord, les bornes par classe et toutes les candidatures, dans l'ordre de création, avec
//! `Pseudo[SRV]` composé ici et les doublons signalés (AD-4 *Données exposées*, AD-6). Rien n'est
//! dédoublonné ni réordonné. Par créneau, les disponibles parmi les `accepted` et leur couverture
//! par classe (`domain::counts::slot_coverage`), et s'il porte un groupe. Les groupes, en sommeil
//! compris, triés par début de créneau puis par occurrence, chacun avec ses placements et leurs
//! avertissements (`domain::placement::warnings`) : titulaire ou banc qui n'a pas coché le créneau
//! du groupe, classe dont le compte de titulaires sort de ses bornes.

use domain::counts::{CoverageInput, role_of, slot_coverage};
use domain::placement::{self, Bound, Placed, Seat};
use domain::pseudo_srv::{duplicates, pseudo_srv};
use domain::reference::ReferenceSet;
use sqlx::SqliteConnection;

use crate::app::AppState;
use crate::app::queries::public_event;
use crate::http::dto::{
    Availability, ClassBound, ClassWarning, GroupBench, GroupStarter, OrganizerCandidature,
    OrganizerEvent, OrganizerGroup, OrganizerSlot,
};
use crate::http::errors::{Code, Problem};
use crate::infra::{candidatures, events, groups};

/// Lecture sur le pool de lecture, dans une transaction. Un événement disparu (purge) donne `auth.not_found` : pour le
/// porteur, le lien ne mène plus à rien.
pub async fn organizer_view(state: &AppState, event_id: &str) -> Result<OrganizerEvent, Problem> {
    let Some(reference) = state.reference.get() else {
        return Err(Code::ReferenceUnavailable.into());
    };
    // Une seule transaction de lecture : les comptes, le lien Discord et la liste voient le même
    // instantané WAL, jamais deux états de l'événement.
    let loaded = async {
        let mut tx = state.db.read.begin().await?;
        let view = load(&mut tx, &reference, event_id).await?;
        tx.commit().await?;
        Ok::<_, sqlx::Error>(view)
    }
    .await;
    match loaded {
        Ok(Some(view)) => Ok(view),
        Ok(None) => Err(Code::AuthNotFound.into()),
        Err(e) => {
            tracing::error!(error = %e, event = event_id, "organizer view failed");
            Err(Code::ServerInternal.into())
        }
    }
}

/// Construit la vue sur une connexion : celle du pool de lecture, ou la transaction d'une commande
/// avant son commit (la réponse rend alors l'agrégat tel qu'écrit, AD-16). `None` si l'événement
/// n'existe pas.
pub async fn load(
    conn: &mut SqliteConnection,
    reference: &ReferenceSet,
    event_id: &str,
) -> Result<Option<OrganizerEvent>, sqlx::Error> {
    let Some(snapshot) = events::load(conn, event_id).await? else {
        return Ok(None);
    };
    let discord_url = events::discord_url(conn, event_id).await?.flatten();
    let rows = candidatures::list(conn, event_id).await?;
    let groups = groups::list(conn, event_id).await?;
    let assignments = groups::assignments(conn, event_id).await?;
    let keys: Vec<_> = rows
        .iter()
        .map(|c| (c.pseudo.as_str(), c.server.as_str(), c.status))
        .collect();
    let flags = duplicates(&keys);
    let checked: Vec<Vec<&str>> = rows
        .iter()
        .map(|c| {
            c.availabilities
                .iter()
                .map(|a| a.slot_id.as_str())
                .collect()
        })
        .collect();
    let slots = {
        let inputs: Vec<CoverageInput<'_>> = rows
            .iter()
            .zip(&checked)
            .map(|(c, slot_ids)| CoverageInput {
                status: c.status,
                class: &c.class,
                slot_ids,
            })
            .collect();
        let slot_ids: Vec<&str> = snapshot.slots.iter().map(|s| s.id.as_str()).collect();
        let bounded: Vec<&str> = snapshot.bounds.iter().map(|b| b.class.as_str()).collect();
        slot_coverage(&slot_ids, &bounded, &inputs)
            .into_iter()
            .map(|s| OrganizerSlot {
                has_groups: groups.iter().any(|g| g.slot_id == s.slot_id),
                slot_id: s.slot_id,
                available_accepted: s.available_accepted,
                coverage: s.coverage,
            })
            .collect()
    };
    let placed: Vec<Placed<'_>> = rows
        .iter()
        .zip(&checked)
        .map(|(c, slot_ids)| Placed {
            id: &c.id,
            class: &c.class,
            slot_ids,
        })
        .collect();
    let bounds: Vec<Bound<'_>> = snapshot
        .bounds
        .iter()
        .map(|b| Bound {
            class: &b.class,
            min: b.min,
            max: b.max,
        })
        .collect();
    let groups = groups
        .into_iter()
        .map(|g| {
            let seats: Vec<(&str, Seat)> = assignments
                .iter()
                .filter(|a| a.group_id == g.id)
                .map(|a| (a.candidature_id.as_str(), a.seat))
                .collect();
            let w = placement::warnings(&g.slot_id, &seats, &placed, &bounds);
            OrganizerGroup {
                id: g.id,
                slot_id: g.slot_id,
                occurrence: g.occurrence,
                retained: g.retained,
                parties: g.parties,
                party_size: g.party_size,
                room_label: g.room_label,
                room_name: g.room_name,
                starters: w
                    .starters
                    .into_iter()
                    .map(|s| GroupStarter {
                        candidature_id: s.candidature_id,
                        party: s.party,
                        index: s.index,
                        unavailable: s.unavailable,
                    })
                    .collect(),
                bench: w
                    .bench
                    .into_iter()
                    .map(|b| GroupBench {
                        candidature_id: b.candidature_id,
                        bench_index: b.bench_index,
                        unavailable: b.unavailable,
                    })
                    .collect(),
                class_warnings: w
                    .class_warnings
                    .into_iter()
                    .map(|(class, c)| {
                        (
                            class,
                            ClassWarning {
                                count: c.count,
                                min: c.min,
                                max: c.max,
                            },
                        )
                    })
                    .collect(),
            }
        })
        .collect();
    let candidatures = rows
        .into_iter()
        .zip(flags)
        .map(|(c, duplicate)| OrganizerCandidature {
            pseudo_srv: pseudo_srv(reference, &c.pseudo, &c.server),
            role: role_of(reference, &c.class).to_owned(),
            id: c.id,
            pseudo: c.pseudo,
            server: c.server,
            class: c.class,
            cp: c.cp,
            status: c.status,
            origin: c.origin,
            availabilities: c
                .availabilities
                .into_iter()
                .map(|a| Availability {
                    slot_id: a.slot_id,
                    preferred: a.preferred,
                })
                .collect(),
            duplicate,
            teams: c.teams.teams,
            preferred_team: c.teams.preferred,
        })
        .collect();
    let team_roles = snapshot.team_roles.clone();
    let class_bounds = snapshot
        .bounds
        .iter()
        .map(|b| {
            (
                b.class.clone(),
                ClassBound {
                    min: b.min,
                    max: b.max,
                },
            )
        })
        .collect();
    Ok(Some(OrganizerEvent {
        event: public_event::build(reference, snapshot),
        discord_url,
        class_bounds,
        team_roles,
        slots,
        groups,
        candidatures,
    }))
}
