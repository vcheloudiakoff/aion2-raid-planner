//! `POST /api/events` (CAP-1), classe de limiteur `create`. Le handler ne fait que traduire : la
//! commande porte le jeton, la validation et la transaction (AD-16).

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use crate::app::AppState;
use crate::app::commands::create_event::{Created, create_event};
use crate::http::dto::{CreateEvent, CreatedEvent, CreatedLinks};
use crate::http::form_token::{FormEnvelope, FormJson};

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
