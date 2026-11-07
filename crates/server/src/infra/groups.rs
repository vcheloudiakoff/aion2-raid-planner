//! SQL des groupes et des placements de l'agrégat événement (AD-4, migration 0003). Écritures
//! seulement dans la transaction d'une commande (AD-16), qui passe `&mut *tx`.

use domain::Timestamp;
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
