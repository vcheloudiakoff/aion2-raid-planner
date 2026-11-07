//! Composition d'un nom de salle libre (AD-18), partagée par `lock_event` (groupe neuf) et
//! `regenerate_room_name` (nouveau nom d'un groupe retenu). Le nom est composé côté serveur, depuis
//! `ROOM_NAME_TEMPLATE`, le `room_label` figé du groupe et un code aléatoire ; il n'est jamais
//! journalisé (AD-15).

use domain::room::compose;
use sqlx::SqliteConnection;

use crate::app::AppState;
use crate::app::commands::EventWriteError as E;
use crate::infra::groups;

/// Essais de nom de salle, le premier compris.
pub const ROOM_NAME_ATTEMPTS: usize = 5;

/// Nom de salle libre parmi tous les groupes (index unique global) : un code neuf par essai,
/// [`ROOM_NAME_ATTEMPTS`] essais au plus, puis une erreur interne (500). `current`, le nom actuel
/// du groupe à renommer, compte comme pris : le nom change toujours.
pub(crate) async fn free_room_name(
    state: &AppState,
    conn: &mut SqliteConnection,
    room_label: &str,
    current: Option<&str>,
) -> Result<String, E> {
    for _ in 0..ROOM_NAME_ATTEMPTS {
        let name = compose(
            &state.room_name_template,
            room_label,
            &state.ids.room_code(),
        );
        if current == Some(name.as_str()) {
            continue;
        }
        if !groups::room_name_taken(conn, &name).await? {
            return Ok(name);
        }
    }
    Err(E::Internal("room name collisions"))
}
