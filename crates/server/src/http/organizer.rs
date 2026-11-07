//! L'organisateur trie les candidatures et règle son événement (CAP-5), sous son lien `/o/`
//! ([`OrganizerLink`]) : le secret ne passe que dans `X-Link-Secret`, et ni l'événement, ni la
//! candidature, ni le créneau n'entrent dans une URL d'API (AD-3).
//!
//! - `GET /api/organizer` : classe `read` ;
//! - `POST /api/organizer/candidature-status` : corps `{candidatureId, status}`, `status` valant
//!   `accepted` ou `rejected`, classe `write` ;
//! - `POST /api/organizer/settings` : corps `UpdateEventSettings`, classe `write` ;
//! - `POST /api/organizer/slots` : corps `{startsAt, durationMin?}`, classe `write` ;
//! - `POST /api/organizer/slot-removal` : corps `{slotId}`, classe `write` ;
//! - `POST /api/organizer/cancellation` : corps `{}`, classe `write`.
//!
//! Les POST lisent leur corps par `FormJson` : sans jeton de formulaire ni honeypot ni disjoncteur
//! (AD-7) ; un corps de mauvaise forme (un `status` hors des deux valeurs, un champ manquant) est
//! `form.malformed`. Chaque POST répond 200 avec l'`OrganizerEvent` à jour. Les handlers ne font que
//! traduire : la requête et les commandes portent les règles.

use axum::Json;
use axum::extract::State;
use axum::response::{IntoResponse, Response};

use crate::app::AppState;
use crate::app::commands::add_slot::add_slot;
use crate::app::commands::cancel_event::cancel_event;
use crate::app::commands::remove_slot::remove_slot;
use crate::app::commands::set_candidature_status::set_candidature_status;
use crate::app::commands::update_event_settings::update_event_settings;
use crate::app::queries::organizer_view::organizer_view;
use crate::http::dto::{
    Cancellation, NewSlot, OrganizerEvent, SetCandidatureStatus, SlotRemoval, UpdateEventSettings,
};
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

pub async fn post_settings(
    State(state): State<AppState>,
    link: OrganizerLink,
    FormJson(body): FormJson<UpdateEventSettings>,
) -> Response {
    respond(update_event_settings(&state, link.event_id(), body).await)
}

pub async fn post_slot(
    State(state): State<AppState>,
    link: OrganizerLink,
    FormJson(body): FormJson<NewSlot>,
) -> Response {
    respond(add_slot(&state, link.event_id(), body).await)
}

pub async fn post_slot_removal(
    State(state): State<AppState>,
    link: OrganizerLink,
    FormJson(body): FormJson<SlotRemoval>,
) -> Response {
    respond(remove_slot(&state, link.event_id(), &body.slot_id).await)
}

pub async fn post_cancellation(
    State(state): State<AppState>,
    link: OrganizerLink,
    FormJson(Cancellation {}): FormJson<Cancellation>,
) -> Response {
    respond(cancel_event(&state, link.event_id()).await)
}

/// 200 `OrganizerEvent` ; `Cache-Control: no-store` comme toute l'API.
fn respond(result: Result<OrganizerEvent, Problem>) -> Response {
    match result {
        Ok(view) => Json(view).into_response(),
        Err(problem) => problem.into_response(),
    }
}
