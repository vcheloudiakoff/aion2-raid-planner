//! SQL des candidatures (AD-4). Écritures seulement dans la transaction d'une commande (AD-16). Ni
//! pseudo ni secret journalisé ici (AD-15) ; faction et rôle ne sont jamais stockés (AD-6).

use domain::Timestamp;
use domain::candidature::{CandidatureStatus, Origin, ValidCandidature};
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
