//! L'organisateur trie les candidatures (CAP-5), sous son lien `/o/` ([`OrganizerLink`]) : le secret
//! ne passe que dans `X-Link-Secret`, et ni l'événement ni la candidature n'entrent dans une URL
//! d'API (AD-3).
//!
//! - `GET /api/organizer` : classe `read` ;
//! - `POST /api/organizer/candidature-status` : corps `{candidatureId, status}`, `status` valant
//!   `accepted` ou `rejected`, classe `write`.
//!
//! Le POST lit son corps par `FormJson` : sans jeton de formulaire ni honeypot ni disjoncteur
//! (AD-7) ; un `status` hors de ces deux valeurs est un corps illisible, `form.malformed`. Les
//! handlers ne font que traduire : la requête et la commande portent les règles.

use axum::Json;
use axum::extract::State;
use axum::response::{IntoResponse, Response};

use crate::app::AppState;
use crate::app::commands::set_candidature_status::set_candidature_status;
use crate::app::queries::organizer_view::organizer_view;
use crate::http::dto::{OrganizerEvent, SetCandidatureStatus};
use crate::http::errors::Problem;
use crate::http::form_token::FormJson;
use crate::http::links::OrganizerLink;

pub async fn get_organizer(State(state): State<AppState>, link: OrganizerLink) -> Response {
    respond(organizer_view(&state, link.event_id()).await)
}

pub async fn post_candidature_status(
    State(state): State<AppState>,
    link: OrganizerLink,
    FormJson(body): FormJson<SetCandidatureStatus>,
) -> Response {
    respond(
        set_candidature_status(&state, link.event_id(), &body.candidature_id, body.status).await,
    )
}

/// 200 `OrganizerEvent` ; `Cache-Control: no-store` comme toute l'API.
fn respond(result: Result<OrganizerEvent, Problem>) -> Response {
    match result {
        Ok(view) => Json(view).into_response(),
        Err(problem) => problem.into_response(),
    }
}
