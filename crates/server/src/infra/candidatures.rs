//! SQL des candidatures (AD-4). Écritures seulement dans la transaction d'une commande (AD-16). Ni
//! pseudo ni secret journalisé ici (AD-15) ; faction et rôle ne sont jamais stockés (AD-6).

use domain::Timestamp;
use domain::candidature::{CandidatureStatus, NewAvailability, Origin, ValidCandidature};
use sqlx::SqliteConnection;

/// Insère la candidature et une ligne `availabilities` par créneau coché.
pub async fn insert(
    conn: &mut SqliteConnection,
    id: &str,
    event_id: &str,
    candidature: &ValidCandidature,
    status: CandidatureStatus,
    origin: Origin,
    now: Timestamp,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO candidatures (id, event_id, pseudo, server, class, cp, status, origin, \
         created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(event_id)
    .bind(&candidature.pseudo)
    .bind(&candidature.server)
    .bind(&candidature.class)
    .bind(i64::from(candidature.cp))
    .bind(status.as_str())
    .bind(origin.as_str())
    .bind(now)
    .bind(now)
    .execute(&mut *conn)
    .await?;
    for a in &candidature.availabilities {
        sqlx::query(
            "INSERT INTO availabilities (candidature_id, slot_id, preferred) VALUES (?, ?, ?)",
        )
        .bind(id)
        .bind(&a.slot_id)
        .bind(i64::from(a.preferred))
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

/// Candidature lue en base, avec ses disponibilités triées par début de créneau. Lue seulement par
/// le droit participant de cette candidature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidatureRow {
    pub id: String,
    pub event_id: String,
    pub pseudo: String,
    pub server: String,
    pub class: String,
    pub cp: u32,
    pub status: CandidatureStatus,
    pub availabilities: Vec<NewAvailability>,
}

fn decode<T>(value: Option<T>, column: &'static str) -> Result<T, sqlx::Error> {
    value.ok_or_else(|| {
        sqlx::Error::Decode(format!("invalid value in candidatures.{column}").into())
    })
}

/// La candidature `id` dans l'événement `event_id` ; `None` si elle n'y est pas. Prend une connexion :
/// celle du pool de lecture, ou la transaction d'une commande.
pub async fn load(
    conn: &mut SqliteConnection,
    event_id: &str,
    id: &str,
) -> Result<Option<CandidatureRow>, sqlx::Error> {
    let row: Option<(String, String, String, i64, String)> = sqlx::query_as(
        "SELECT pseudo, server, class, cp, status FROM candidatures WHERE id = ? AND event_id = ?",
    )
    .bind(id)
    .bind(event_id)
    .fetch_optional(&mut *conn)
    .await?;
    let Some((pseudo, server, class, cp, status)) = row else {
        return Ok(None);
    };
    let availabilities: Vec<(String, i64)> = sqlx::query_as(
        "SELECT a.slot_id, a.preferred FROM availabilities a JOIN slots s ON s.id = a.slot_id \
         WHERE a.candidature_id = ? ORDER BY s.starts_at, s.id",
    )
    .bind(id)
    .fetch_all(&mut *conn)
    .await?;
    Ok(Some(CandidatureRow {
        id: id.to_owned(),
        event_id: event_id.to_owned(),
        pseudo,
        server,
        class,
        cp: decode(u32::try_from(cp).ok(), "cp")?,
        status: decode(CandidatureStatus::parse(&status), "status")?,
        availabilities: availabilities
            .into_iter()
            .map(|(slot_id, preferred)| NewAvailability {
                slot_id,
                preferred: preferred != 0,
            })
            .collect(),
    }))
}

/// Remplace le contenu de la candidature : champs, puis disponibilités des créneaux `replaced`
/// remplacées par celles de `candidature`. `status` et `origin` ne changent jamais ici (AD-4).
pub async fn update_content(
    conn: &mut SqliteConnection,
    id: &str,
    candidature: &ValidCandidature,
    replaced: &[String],
    now: Timestamp,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE candidatures SET pseudo = ?, server = ?, class = ?, cp = ?, updated_at = ? \
         WHERE id = ?",
    )
    .bind(&candidature.pseudo)
    .bind(&candidature.server)
    .bind(&candidature.class)
    .bind(i64::from(candidature.cp))
    .bind(now)
    .bind(id)
    .execute(&mut *conn)
    .await?;
    for slot_id in replaced {
        sqlx::query("DELETE FROM availabilities WHERE candidature_id = ? AND slot_id = ?")
            .bind(id)
            .bind(slot_id)
            .execute(&mut *conn)
            .await?;
    }
    for a in &candidature.availabilities {
        sqlx::query(
            "INSERT INTO availabilities (candidature_id, slot_id, preferred) VALUES (?, ?, ?)",
        )
        .bind(id)
        .bind(&a.slot_id)
        .bind(i64::from(a.preferred))
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

/// Pose le statut de la candidature et `updated_at`.
pub async fn set_status(
    conn: &mut SqliteConnection,
    id: &str,
    status: CandidatureStatus,
    now: Timestamp,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE candidatures SET status = ?, updated_at = ? WHERE id = ?")
        .bind(status.as_str())
        .bind(now)
        .bind(id)
        .execute(&mut *conn)
        .await?;
    Ok(())
}
