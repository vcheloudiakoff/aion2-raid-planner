//! `POST /api/events` (CAP-1), classe de limiteur `create`, et la page HTML `/e/{event_id}`. Le
//! handler ne fait que traduire : la commande porte le jeton, la validation et la transaction
//! (AD-16).

use axum::Json;
use axum::extract::{Path, State};
use axum::http::header::HeaderValue;
use axum::http::{Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

use crate::app::AppState;
use crate::app::commands::create_event::{Created, create_event};
use crate::http::assets;
use crate::http::dto::{CreateEvent, CreatedEvent, CreatedLinks};
use crate::http::form_token::{FormEnvelope, FormJson};
use crate::http::headers::X_ROBOTS_TAG;
use crate::infra::events as infra_events;
use crate::infra::ids::ID_LEN;

/// 201 `CreatedEvent` ; `Cache-Control: no-store` comme toute l'API (`headers::security_headers`).
pub async fn post_event(
    State(state): State<AppState>,
    FormJson(body): FormJson<FormEnvelope<CreateEvent>>,
) -> Response {
    match create_event(&state, body).await {
        Ok(created) => (StatusCode::CREATED, Json(CreatedEvent::from(created))).into_response(),
        Err(problem) => problem.into_response(),
    }
}

impl From<Created> for CreatedEvent {
    fn from(c: Created) -> Self {
        let (organizer, join) = (c.organizer.expose(), c.join.expose());
        Self {
            links: CreatedLinks {
                organizer: format!("/o/{organizer}"),
                join: format!("/j/{join}"),
                public: format!("/e/{}", c.event_id),
            },
            event_id: c.event_id.to_string(),
            organizer_secret: organizer.to_owned(),
            join_secret: join.to_owned(),
        }
    }
}

/// Forme d'un id public : 22 caractères base64url sans padding. Rien d'autre n'atteint la base.
fn is_public_id(id: &str) -> bool {
    id.len() == ID_LEN
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
}

/// Repli de la SPA : tout autre chemin sous `/e/` que la route exacte (`/e/<id>/`, `/e/<id>/x`)
/// porte `noindex`, faute de pouvoir dire s'il vise un événement `unlisted`.
pub async fn spa_fallback<A: RustEmbed>(method: Method, uri: Uri) -> Response {
    let under_event = uri.path() == "/e" || uri.path().starts_with("/e/");
    let mut res = assets::serve::<A>(method, uri).await;
    if under_event {
        res.headers_mut()
            .insert(X_ROBOTS_TAG, HeaderValue::from_static("noindex"));
    }
    res
}

/// Page HTML `/e/{event_id}` : `index.html` comme toute route de la SPA, avec `X-Robots-Tag:
/// noindex` si l'événement est `unlisted`, retiré de la liste par l'administration ou inconnu (AD-3).
/// Une lecture sur `db.read` ; un échec de lecture vaut « inconnu », donc noindex. Aucune métadonnée
/// `og:` : `index.html` n'en porte pas.
pub async fn event_page<A: RustEmbed>(
    State(state): State<AppState>,
    Path(event_id): Path<String>,
    method: Method,
    uri: Uri,
) -> Response {
    let indexable = is_public_id(&event_id)
        && match infra_events::is_indexable(&state.db.read, &event_id).await {
            Ok(v) => v,
            Err(e) => {
                tracing::error!(error = %e, "event page read failed");
                false
            }
        };
    let mut res = assets::serve::<A>(method, uri).await;
    if !indexable {
        res.headers_mut()
            .insert(X_ROBOTS_TAG, HeaderValue::from_static("noindex"));
    }
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_id_shape() {
        assert!(is_public_id("aZ09-_aZ09-_aZ09-_aZ09"));
        for bad in [
            "aZ09-_aZ09-_aZ09-_aZ0",
            "aZ09-_aZ09-_aZ09-_aZ0=",
            "aZ09-_aZ09-_aZ09-_aZ0%",
            "é",
        ] {
            assert!(!is_public_id(bad), "{bad}");
        }
    }
}
