//! SQL des candidatures (AD-4). Écritures seulement dans la transaction d'une commande (AD-16). Ni
//! pseudo ni secret journalisé ici (AD-15) ; faction et rôle ne sont jamais stockés (AD-6).

use std::collections::HashMap;

use domain::Timestamp;
use domain::candidature::{
    CandidatureStatus, NewAvailability, Origin, TeamChoice, ValidCandidature,
};
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
         created_at, updated_at, teams, preferred_team) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
    .bind(candidature.teams.mask())
    .bind(candidature.teams.preferred.map(i64::from))
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

/// Candidature lue en base, avec ses disponibilités triées par début de créneau. Lue par le droit
/// participant de cette candidature, ou par le droit organisateur de son événement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidatureRow {
    pub id: String,
    pub event_id: String,
    pub pseudo: String,
    pub server: String,
    pub class: String,
    pub cp: u32,
    pub status: CandidatureStatus,
    pub origin: Origin,
    pub availabilities: Vec<NewAvailability>,
    pub teams: TeamChoice,
}

fn decode<T>(value: Option<T>, column: &'static str) -> Result<T, sqlx::Error> {
    value.ok_or_else(|| {
        sqlx::Error::Decode(format!("invalid value in candidatures.{column}").into())
    })
}

/// Colonnes lues d'une candidature : `pseudo, server, class, cp, status, origin, teams,
/// preferred_team`.
type Columns = (
    String,
    String,
    String,
    i64,
    String,
    String,
    i64,
    Option<i64>,
);

fn row(
    id: String,
    event_id: &str,
    (pseudo, server, class, cp, status, origin, teams, preferred): Columns,
    availabilities: Vec<NewAvailability>,
) -> Result<CandidatureRow, sqlx::Error> {
    Ok(CandidatureRow {
        id,
        event_id: event_id.to_owned(),
        pseudo,
        server,
        class,
        cp: decode(u32::try_from(cp).ok(), "cp")?,
        status: decode(CandidatureStatus::parse(&status), "status")?,
        origin: decode(Origin::parse(&origin), "origin")?,
        availabilities,
        teams: decode(TeamChoice::from_mask(teams, preferred), "teams")?,
    })
}

fn availability((slot_id, preferred): (String, i64)) -> NewAvailability {
    NewAvailability {
        slot_id,
        preferred: preferred != 0,
    }
}

/// La candidature `id` dans l'événement `event_id` ; `None` si elle n'y est pas. Prend une connexion :
/// celle du pool de lecture, ou la transaction d'une commande.
pub async fn load(
    conn: &mut SqliteConnection,
    event_id: &str,
    id: &str,
) -> Result<Option<CandidatureRow>, sqlx::Error> {
    let columns: Option<Columns> = sqlx::query_as(
        "SELECT pseudo, server, class, cp, status, origin, teams, preferred_team \
         FROM candidatures WHERE id = ? AND event_id = ?",
    )
    .bind(id)
    .bind(event_id)
    .fetch_optional(&mut *conn)
    .await?;
    let Some(columns) = columns else {
        return Ok(None);
    };
    let availabilities: Vec<(String, i64)> = sqlx::query_as(
        "SELECT a.slot_id, a.preferred FROM availabilities a JOIN slots s ON s.id = a.slot_id \
         WHERE a.candidature_id = ? ORDER BY s.starts_at, s.id",
    )
    .bind(id)
    .fetch_all(&mut *conn)
    .await?;
    row(
        id.to_owned(),
        event_id,
        columns,
        availabilities.into_iter().map(availability).collect(),
    )
    .map(Some)
}

/// Statut de la candidature `id` si elle appartient à l'événement `event_id` ; `None` si elle est
/// inconnue ou d'un autre événement. Lue dans la transaction d'une commande de l'organisateur, qui
/// ne tient jamais pour acquis qu'un id venu du corps vise son événement.
pub async fn load_in_event(
    conn: &mut SqliteConnection,
    event_id: &str,
    id: &str,
) -> Result<Option<CandidatureStatus>, sqlx::Error> {
    let status: Option<(String,)> =
        sqlx::query_as("SELECT status FROM candidatures WHERE id = ? AND event_id = ?")
            .bind(id)
            .bind(event_id)
            .fetch_optional(&mut *conn)
            .await?;
    status
        .map(|(s,)| decode(CandidatureStatus::parse(&s), "status"))
        .transpose()
}

/// Toutes les candidatures de l'événement, tous statuts, dans l'ordre de création (`created_at`
/// puis `rowid`, l'ordre d'insertion à la seconde près), chacune avec ses disponibilités triées par
/// début de créneau. Lue par la vue organisateur et par la vue participant (coéquipiers, liste des
/// acceptés), qui n'en expose que ce que la règle du domaine permet. Deux requêtes, quel que soit le
/// nombre de candidatures.
pub async fn list(
    conn: &mut SqliteConnection,
    event_id: &str,
) -> Result<Vec<CandidatureRow>, sqlx::Error> {
    #[allow(clippy::type_complexity)]
    let rows: Vec<(
        String,
        String,
        String,
        String,
        i64,
        String,
        String,
        i64,
        Option<i64>,
    )> = sqlx::query_as(
        "SELECT id, pseudo, server, class, cp, status, origin, teams, preferred_team \
             FROM candidatures WHERE event_id = ? ORDER BY created_at, rowid",
    )
    .bind(event_id)
    .fetch_all(&mut *conn)
    .await?;
    let checked: Vec<(String, String, i64)> = sqlx::query_as(
        "SELECT a.candidature_id, a.slot_id, a.preferred FROM availabilities a \
         JOIN candidatures c ON c.id = a.candidature_id JOIN slots s ON s.id = a.slot_id \
         WHERE c.event_id = ? ORDER BY s.starts_at, s.id",
    )
    .bind(event_id)
    .fetch_all(&mut *conn)
    .await?;
    let mut by_candidature: HashMap<String, Vec<NewAvailability>> = HashMap::new();
    for (candidature_id, slot_id, preferred) in checked {
        by_candidature
            .entry(candidature_id)
            .or_default()
            .push(availability((slot_id, preferred)));
    }
    rows.into_iter()
        .map(
            |(id, pseudo, server, class, cp, status, origin, teams, preferred)| {
                let availabilities = by_candidature.remove(&id).unwrap_or_default();
                row(
                    id,
                    event_id,
                    (pseudo, server, class, cp, status, origin, teams, preferred),
                    availabilities,
                )
            },
        )
        .collect()
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
        "UPDATE candidatures SET pseudo = ?, server = ?, class = ?, cp = ?, updated_at = ?, \
         teams = ?, preferred_team = ? WHERE id = ?",
    )
    .bind(&candidature.pseudo)
    .bind(&candidature.server)
    .bind(&candidature.class)
    .bind(i64::from(candidature.cp))
    .bind(now)
    .bind(candidature.teams.mask())
    .bind(candidature.teams.preferred.map(i64::from))
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

/// Id et statut de chaque candidature de l'événement, tous statuts : ce que `apply_placement`
/// vérifie d'un placement. Lue dans la transaction de la commande.
pub async fn statuses(
    conn: &mut SqliteConnection,
    event_id: &str,
) -> Result<Vec<(String, CandidatureStatus)>, sqlx::Error> {
    let rows: Vec<(String, String)> =
        sqlx::query_as("SELECT id, status FROM candidatures WHERE event_id = ?")
            .bind(event_id)
            .fetch_all(&mut *conn)
            .await?;
    rows.into_iter()
        .map(|(id, s)| Ok((id, decode(CandidatureStatus::parse(&s), "status")?)))
        .collect()
}
