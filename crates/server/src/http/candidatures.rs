//! Lecture publique et candidature (CAP-2, CAP-3). Quatre routes ; le secret du lien direct ne passe
//! que dans `X-Link-Secret`, jamais dans une URL d'API (AD-3).
//!
//! - `GET /api/events/{event_id}` : public, classe `read` ;
//! - `GET /api/join` : droit `join` ([`JoinLink`]), classe `read` ;
//! - `POST /api/events/{event_id}/candidatures` : public, `pending`, classe `candidature` ;
//! - `POST /api/join/candidatures` : droit `join`, `accepted`, classe `candidature`.
//!
//! Les handlers ne font que traduire : la requête et la commande portent les règles (AD-5, AD-16).

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use domain::candidature::Origin;

use crate::app::AppState;
use crate::app::commands::submit_candidature::{Submitted, submit_candidature};
use crate::app::queries::public_event::public_event;
use crate::http::dto::{CreatedCandidature, ParticipantLinks, SubmitCandidature};
use crate::http::form_token::{FormEnvelope, FormJson};
use crate::http::links::JoinLink;

pub async fn get_public_event(
    State(state): State<AppState>,
    Path(event_id): Path<String>,
) -> Response {
    match public_event(&state, &event_id).await {
        Ok(event) => Json(event).into_response(),
        Err(problem) => problem.into_response(),
    }
}

pub async fn get_join_event(State(state): State<AppState>, link: JoinLink) -> Response {
    match public_event(&state, link.event_id()).await {
        Ok(event) => Json(event).into_response(),
        Err(problem) => problem.into_response(),
    }
}

pub async fn post_public_candidature(
    State(state): State<AppState>,
    Path(event_id): Path<String>,
    FormJson(body): FormJson<FormEnvelope<SubmitCandidature>>,
) -> Response {
    respond(submit_candidature(&state, &event_id, Origin::Public, body).await)
}

pub async fn post_join_candidature(
    State(state): State<AppState>,
    link: JoinLink,
    FormJson(body): FormJson<FormEnvelope<SubmitCandidature>>,
) -> Response {
    respond(submit_candidature(&state, link.event_id(), Origin::Direct, body).await)
}

/// 201 `CreatedCandidature` ; `Cache-Control: no-store` comme toute l'API.
fn respond(result: Result<Submitted, crate::http::errors::Problem>) -> Response {
    match result {
        Ok(s) => (StatusCode::CREATED, Json(CreatedCandidature::from(s))).into_response(),
        Err(problem) => problem.into_response(),
    }
}

impl From<Submitted> for CreatedCandidature {
    fn from(s: Submitted) -> Self {
        let secret = s.participant.expose();
        Self {
            links: ParticipantLinks {
                participant: format!("/p/{secret}"),
            },
            event_id: s.event_id,
            candidature_id: s.candidature_id.to_string(),
            participant_secret: secret.to_owned(),
            status: s.status,
        }
    }
}
