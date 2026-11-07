//! SQL de l'agrégat événement (AD-4). Écritures seulement dans la transaction d'une commande (AD-16),
//! qui passe `&mut *tx`. Aucun « maintenant » ici : l'instant vient de la `Clock` de la commande.

use domain::Timestamp;
use domain::event::ValidEvent;
use sqlx::SqliteConnection;

/// Statut initial d'un événement.
pub const STATUS_OPEN: &str = "open";

/// Insère l'événement, ses créneaux (un id par créneau, dans l'ordre) et ses bornes par classe.
pub async fn insert(
    conn: &mut SqliteConnection,
    event_id: &str,
    slot_ids: &[String],
    event: &ValidEvent,
    created_at: Timestamp,
) -> Result<(), sqlx::Error> {
    debug_assert_eq!(slot_ids.len(), event.slots.len());
    sqlx::query(
        "INSERT INTO events (id, instance, region, faction_scope, visibility, status, min_cp, \
         discord_url, note, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(event_id)
    .bind(&event.instance)
    .bind(&event.region)
    .bind(event.faction_scope.as_str())
    .bind(event.visibility.as_str())
    .bind(STATUS_OPEN)
    .bind(event.min_cp.map(i64::from))
    .bind(&event.discord_url)
    .bind(&event.note)
    .bind(created_at)
    .execute(&mut *conn)
    .await?;
    for (id, slot) in slot_ids.iter().zip(&event.slots) {
        sqlx::query(
            "INSERT INTO slots (id, event_id, starts_at, duration_min) VALUES (?, ?, ?, ?)",
        )
        .bind(id)
        .bind(event_id)
        .bind(slot.starts_at)
        .bind(i64::from(slot.duration_min))
        .execute(&mut *conn)
        .await?;
    }
    for bound in &event.class_bounds {
        sqlx::query(
            "INSERT INTO class_bounds (event_id, class, min_count, max_count) VALUES (?, ?, ?, ?)",
        )
        .bind(event_id)
        .bind(&bound.class)
        .bind(i64::from(bound.min))
        .bind(i64::from(bound.max))
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}
