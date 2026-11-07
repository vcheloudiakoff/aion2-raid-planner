//! SQL de l'agrégat événement (AD-4). Lectures publiques sans donnée nominative. Écritures
//! seulement dans la transaction d'une commande (AD-16), qui passe `&mut *tx`. Aucun « maintenant »
//! ici : l'instant vient de la `Clock` de la commande.

use domain::Timestamp;
use domain::candidature::CandidatureStatus;
use domain::event::{EventStatus, FactionScope, ValidEvent, Visibility};
use sqlx::SqliteConnection;
use sqlx::sqlite::SqlitePool;

/// Statut initial d'un événement.
pub const STATUS_OPEN: &str = EventStatus::Open.as_str();

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

/// Créneau lu en base.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotRow {
    pub id: String,
    pub starts_at: Timestamp,
    pub duration_min: u32,
}

/// Borne lue en base.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundRow {
    pub class: String,
    pub min: u32,
    pub max: u32,
}

/// L'événement tel qu'une lecture publique ou une candidature le voit : jamais `discord_url`, jamais
/// la liste nominative ; des candidatures, seulement la classe de celles qui sont comptées.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventSnapshot {
    pub id: String,
    pub instance: String,
    pub region: String,
    pub faction_scope: FactionScope,
    pub visibility: Visibility,
    pub status: EventStatus,
    pub min_cp: Option<u32>,
    pub note: String,
    pub admin_unlisted_at: Option<Timestamp>,
    /// Triés par `starts_at`.
    pub slots: Vec<SlotRow>,
    /// Triées par classe.
    pub bounds: Vec<BoundRow>,
    /// Classe de chaque candidature `pending` ou `accepted`.
    pub counted_classes: Vec<String>,
}

fn decode<T>(value: Option<T>, column: &'static str) -> Result<T, sqlx::Error> {
    value.ok_or_else(|| sqlx::Error::Decode(format!("invalid value in events.{column}").into()))
}

fn to_u32(value: i64, column: &'static str) -> Result<u32, sqlx::Error> {
    decode(u32::try_from(value).ok(), column)
}

type EventColumns = (
    String,
    String,
    String,
    String,
    String,
    String,
    Option<i64>,
    String,
    Option<Timestamp>,
);

/// Lit l'événement, ses créneaux, ses bornes et les classes de ses candidatures comptées. `None` si
/// l'id est inconnu. Prend une connexion : celle du pool de lecture, ou la transaction d'une commande.
pub async fn load(
    conn: &mut SqliteConnection,
    event_id: &str,
) -> Result<Option<EventSnapshot>, sqlx::Error> {
    let row: Option<EventColumns> = sqlx::query_as(
        "SELECT id, instance, region, faction_scope, visibility, status, min_cp, note, \
         admin_unlisted_at FROM events WHERE id = ?",
    )
    .bind(event_id)
    .fetch_optional(&mut *conn)
    .await?;
    let Some((id, instance, region, scope, visibility, status, min_cp, note, admin_unlisted_at)) =
        row
    else {
        return Ok(None);
    };
    let slots: Vec<(String, Timestamp, i64)> = sqlx::query_as(
        "SELECT id, starts_at, duration_min FROM slots WHERE event_id = ? ORDER BY starts_at, id",
    )
    .bind(event_id)
    .fetch_all(&mut *conn)
    .await?;
    let bounds: Vec<(String, i64, i64)> = sqlx::query_as(
        "SELECT class, min_count, max_count FROM class_bounds WHERE event_id = ? ORDER BY class",
    )
    .bind(event_id)
    .fetch_all(&mut *conn)
    .await?;
    let counted: Vec<(String, String)> =
        sqlx::query_as("SELECT class, status FROM candidatures WHERE event_id = ?")
            .bind(event_id)
            .fetch_all(&mut *conn)
            .await?;
    let mut counted_classes = Vec::with_capacity(counted.len());
    for (class, status) in counted {
        if decode(CandidatureStatus::parse(&status), "candidatures.status")?.is_counted() {
            counted_classes.push(class);
        }
    }
    Ok(Some(EventSnapshot {
        id,
        instance,
        region,
        faction_scope: decode(FactionScope::parse(&scope), "faction_scope")?,
        visibility: decode(Visibility::parse(&visibility), "visibility")?,
        status: decode(EventStatus::parse(&status), "status")?,
        min_cp: min_cp.map(|v| to_u32(v, "min_cp")).transpose()?,
        note,
        admin_unlisted_at,
        slots: slots
            .into_iter()
            .map(|(id, starts_at, d)| {
                Ok(SlotRow {
                    id,
                    starts_at,
                    duration_min: to_u32(d, "slots.duration_min")?,
                })
            })
            .collect::<Result<_, sqlx::Error>>()?,
        bounds: bounds
            .into_iter()
            .map(|(class, min, max)| {
                Ok(BoundRow {
                    class,
                    min: to_u32(min, "class_bounds.min_count")?,
                    max: to_u32(max, "class_bounds.max_count")?,
                })
            })
            .collect::<Result<_, sqlx::Error>>()?,
        counted_classes,
    }))
}

/// Statut courant et nombre de candidatures (tous statuts), relus dans la transaction d'une
/// commande. `None` si l'événement n'existe plus.
pub async fn status_and_candidature_count(
    conn: &mut SqliteConnection,
    event_id: &str,
) -> Result<Option<(EventStatus, i64)>, sqlx::Error> {
    let row: Option<(String, i64)> = sqlx::query_as(
        "SELECT status, (SELECT COUNT(*) FROM candidatures WHERE event_id = events.id) \
         FROM events WHERE id = ?",
    )
    .bind(event_id)
    .fetch_optional(&mut *conn)
    .await?;
    row.map(|(status, n)| Ok((decode(EventStatus::parse(&status), "status")?, n)))
        .transpose()
}

/// Vrai si la page `/e/<id>` peut être indexée : événement connu, `listed`, jamais retiré de la
/// liste par l'administration. Prend le pool de lecture.
pub async fn is_indexable(pool: &SqlitePool, event_id: &str) -> Result<bool, sqlx::Error> {
    let row: Option<(i64,)> = sqlx::query_as(
        "SELECT visibility = 'listed' AND admin_unlisted_at IS NULL FROM events WHERE id = ?",
    )
    .bind(event_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.is_some_and(|(v,)| v == 1))
}
