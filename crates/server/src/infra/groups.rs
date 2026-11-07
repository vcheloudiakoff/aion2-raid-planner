//! SQL des groupes et des placements de l'agrégat événement (AD-4, migration 0003). Écritures
//! seulement dans la transaction d'une commande (AD-16), qui passe `&mut *tx`.

use domain::Timestamp;
use domain::placement::{Placement, Seat};
use sqlx::SqliteConnection;

/// Groupe lu en base.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupRow {
    pub id: String,
    pub slot_id: String,
    pub occurrence: u32,
    pub retained: bool,
    pub parties: u32,
    pub party_size: u32,
    pub room_label: String,
    pub room_name: String,
}

/// Groupe à créer : disposition et libellé figés depuis l'instance, nom déjà composé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewGroup<'a> {
    pub id: &'a str,
    pub slot_id: &'a str,
    pub occurrence: u32,
    pub parties: u32,
    pub party_size: u32,
    pub room_label: &'a str,
    pub room_name: &'a str,
}

fn to_u32(value: i64, column: &'static str) -> Result<u32, sqlx::Error> {
    u32::try_from(value)
        .map_err(|_| sqlx::Error::Decode(format!("invalid value in groups.{column}").into()))
}

type GroupColumns = (String, String, i64, i64, i64, i64, String, String);

/// Groupes de l'événement, en sommeil compris, triés par début de créneau puis par occurrence.
pub async fn list(
    conn: &mut SqliteConnection,
    event_id: &str,
) -> Result<Vec<GroupRow>, sqlx::Error> {
    let rows: Vec<GroupColumns> = sqlx::query_as(
        "SELECT g.id, g.slot_id, g.occurrence, g.retained, g.parties, g.party_size, g.room_label, \
         g.room_name FROM groups g JOIN slots s ON s.id = g.slot_id WHERE g.event_id = ? \
         ORDER BY s.starts_at, s.id, g.occurrence",
    )
    .bind(event_id)
    .fetch_all(&mut *conn)
    .await?;
    rows.into_iter()
        .map(
            |(id, slot_id, occurrence, retained, parties, party_size, room_label, room_name)| {
                Ok(GroupRow {
                    id,
                    slot_id,
                    occurrence: to_u32(occurrence, "occurrence")?,
                    retained: retained == 1,
                    parties: to_u32(parties, "parties")?,
                    party_size: to_u32(party_size, "party_size")?,
                    room_label,
                    room_name,
                })
            },
        )
        .collect()
}

/// Vrai si un groupe, de n'importe quel événement, porte déjà ce nom de salle.
pub async fn room_name_taken(
    conn: &mut SqliteConnection,
    room_name: &str,
) -> Result<bool, sqlx::Error> {
    let row: Option<(i64,)> = sqlx::query_as("SELECT 1 FROM groups WHERE room_name = ?")
        .bind(room_name)
        .fetch_optional(&mut *conn)
        .await?;
    Ok(row.is_some())
}

/// Insère un groupe retenu.
pub async fn insert(
    conn: &mut SqliteConnection,
    event_id: &str,
    group: &NewGroup<'_>,
    created_at: Timestamp,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO groups (id, event_id, slot_id, occurrence, retained, parties, party_size, \
         room_label, room_name, created_at) VALUES (?, ?, ?, ?, 1, ?, ?, ?, ?, ?)",
    )
    .bind(group.id)
    .bind(event_id)
    .bind(group.slot_id)
    .bind(i64::from(group.occurrence))
    .bind(i64::from(group.parties))
    .bind(i64::from(group.party_size))
    .bind(group.room_label)
    .bind(group.room_name)
    .bind(created_at)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// Pose `retained` sur un groupe de l'événement ; rien d'autre ne change (nom, disposition,
/// placements).
pub async fn set_retained(
    conn: &mut SqliteConnection,
    event_id: &str,
    group_id: &str,
    retained: bool,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE groups SET retained = ? WHERE id = ? AND event_id = ?")
        .bind(i64::from(retained))
        .bind(group_id)
        .bind(event_id)
        .execute(&mut *conn)
        .await?;
    Ok(())
}

/// Retire le placement d'une candidature, s'il existe : au retrait et au refus, dans la même
/// transaction (exception nommée à la propriété du placement, AD-4).
pub async fn delete_assignment(
    conn: &mut SqliteConnection,
    candidature_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM assignments WHERE candidature_id = ?")
        .bind(candidature_id)
        .execute(&mut *conn)
        .await?;
    Ok(())
}

/// Placement lu en base.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignmentRow {
    pub candidature_id: String,
    pub group_id: String,
    pub seat: Seat,
}

type AssignmentColumns = (String, String, Option<i64>, Option<i64>, Option<i64>);

fn seat(
    party: Option<i64>,
    party_index: Option<i64>,
    bench_index: Option<i64>,
) -> Result<Seat, sqlx::Error> {
    let to = |v: i64, column| {
        u32::try_from(v).map_err(|_| {
            sqlx::Error::Decode(format!("invalid value in assignments.{column}").into())
        })
    };
    match (party, party_index, bench_index) {
        (Some(party), Some(index), None) => Ok(Seat::Starter {
            party: to(party, "party")?,
            index: to(index, "party_index")?,
        }),
        (None, None, Some(bench)) => Ok(Seat::Bench {
            bench_index: to(bench, "bench_index")?,
        }),
        _ => Err(sqlx::Error::Decode(
            "invalid seat in assignments".to_owned().into(),
        )),
    }
}

/// Placements de l'événement, groupes en sommeil compris, triés par groupe, puis le banc par rang
/// (`party` nul trié en tête), puis les titulaires par équipe et slot. L'appelant ne compte pas sur
/// cet ordre : `domain::placement::warnings` retrie.
pub async fn assignments(
    conn: &mut SqliteConnection,
    event_id: &str,
) -> Result<Vec<AssignmentRow>, sqlx::Error> {
    let rows: Vec<AssignmentColumns> = sqlx::query_as(
        "SELECT candidature_id, group_id, party, party_index, bench_index FROM assignments \
         WHERE event_id = ? ORDER BY group_id, party, party_index, bench_index",
    )
    .bind(event_id)
    .fetch_all(&mut *conn)
    .await?;
    rows.into_iter()
        .map(
            |(candidature_id, group_id, party, party_index, bench_index)| {
                Ok(AssignmentRow {
                    candidature_id,
                    group_id,
                    seat: seat(party, party_index, bench_index)?,
                })
            },
        )
        .collect()
}

/// Remplace le placement des groupes retenus (`apply_placement`, seul writer, AD-4) : supprime les
/// lignes des groupes retenus de l'événement, puis toute ligne d'une candidature de `placements`
/// (un groupe en sommeil la perd, `UNIQUE(candidature_id)`), puis insère `placements`. Les autres
/// lignes des groupes en sommeil restent. Dans la transaction de la commande.
pub async fn replace_assignments(
    conn: &mut SqliteConnection,
    event_id: &str,
    placements: &[Placement],
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "DELETE FROM assignments WHERE event_id = ?1 AND group_id IN \
         (SELECT id FROM groups WHERE event_id = ?1 AND retained = 1)",
    )
    .bind(event_id)
    .execute(&mut *conn)
    .await?;
    for p in placements {
        sqlx::query("DELETE FROM assignments WHERE event_id = ? AND candidature_id = ?")
            .bind(event_id)
            .bind(&p.candidature_id)
            .execute(&mut *conn)
            .await?;
    }
    for p in placements {
        let (party, party_index, bench_index) = match p.seat {
            Seat::Starter { party, index } => {
                (Some(i64::from(party)), Some(i64::from(index)), None)
            }
            Seat::Bench { bench_index } => (None, None, Some(i64::from(bench_index))),
        };
        sqlx::query(
            "INSERT INTO assignments (event_id, candidature_id, group_id, party, party_index, \
             bench_index) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(event_id)
        .bind(&p.candidature_id)
        .bind(&p.group_id)
        .bind(party)
        .bind(party_index)
        .bind(bench_index)
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}
