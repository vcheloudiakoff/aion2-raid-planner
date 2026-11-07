//! Ma candidature (CAP-4), sous le lien personnel `/p/` ([`ParticipantLink`]) : le secret ne passe
//! que dans `X-Link-Secret`, et ni l'événement ni la candidature n'entrent dans une URL d'API (AD-3).
//!
//! - `GET /api/participant` : classe `read` ;
//! - `POST /api/participant/candidature` : modification, classe `write` ;
//! - `POST /api/participant/withdrawal` : retrait, corps `{}`, classe `write`.
//!
//! Les POST lisent leur corps par `FormJson` : sans jeton de formulaire ni honeypot ni disjoncteur
//! (AD-7). Les handlers ne font que traduire : la requête et les commandes portent les règles.

use axum::Json;
use axum::extract::State;
use axum::response::{IntoResponse, Response};

use crate::app::AppState;
use crate::app::commands::update_candidature::update_candidature;
use crate::app::commands::withdraw_candidature::withdraw_candidature;
use crate::app::queries::participant_view::participant_view;
use crate::http::dto::{ParticipantView, SubmitCandidature, Withdrawal};
use crate::http::errors::Problem;
use crate::http::form_token::FormJson;
use crate::http::links::ParticipantLink;

pub async fn get_participant(State(state): State<AppState>, link: ParticipantLink) -> Response {
    respond(participant_view(&state, link.event_id(), link.candidature_id()).await)
}

pub async fn post_candidature(
    State(state): State<AppState>,
    link: ParticipantLink,
    FormJson(body): FormJson<SubmitCandidature>,
) -> Response {
    respond(update_candidature(&state, link.event_id(), link.candidature_id(), body).await)
}

pub async fn post_withdrawal(
    State(state): State<AppState>,
    link: ParticipantLink,
    FormJson(Withdrawal {}): FormJson<Withdrawal>,
) -> Response {
    respond(withdraw_candidature(&state, link.event_id(), link.candidature_id()).await)
}

/// 200 `ParticipantView` ; `Cache-Control: no-store` comme toute l'API.
fn respond(result: Result<ParticipantView, Problem>) -> Response {
    match result {
        Ok(view) => Json(view).into_response(),
        Err(problem) => problem.into_response(),
    }
}
