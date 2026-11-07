//! Routes Axum, assets embarqués et en-têtes. Même origine pour l'API et la SPA : pas de CORS.

pub mod assets;
pub mod candidatures;
pub mod client_ip;
pub mod dto;
pub mod errors;
pub mod events;
pub mod form_token;
pub mod headers;
pub mod limits;
pub mod links;
pub mod organizer;
pub mod participant;
pub mod reference;

#[cfg(test)]
mod abuse_tests;

use std::future::Future;
use std::net::SocketAddr;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Router, middleware};
use rust_embed::RustEmbed;

use crate::app::AppState;
use crate::app::health::{Health, health};
use errors::Code;
use limits::{Guard, RouteClass};

/// Routeur de production, SPA servie depuis `web/dist/` embarqué.
pub fn router(state: AppState) -> Router {
    router_with::<assets::WebDist>(state)
}

/// Routeur sur un dossier d'assets quelconque (les tests embarquent une SPA de démonstration).
pub fn router_with<A: RustEmbed + Send + Sync + 'static>(state: AppState) -> Router {
    // Classe `read` : IP visiteur exigée, quota `RATE_READ` (AD-7). `route_layer` ne couvre que les
    // routes trouvées : le repli 404 et les 405 restent en `nokey`.
    let read = Router::new()
        .route("/reference", get(reference::get_reference))
        .route("/form-token", get(form_token::get_form_token))
        .route("/events/{event_id}", get(candidatures::get_public_event))
        .route("/join", get(candidatures::get_join_event))
        .route("/participant", get(participant::get_participant))
        .route("/organizer", get(organizer::get_organizer))
        .route_layer(middleware::from_fn_with_state(
            Guard::new(&state.limits, RouteClass::Read),
            limits::enforce,
        ));
    // Classe `create` : jeton de formulaire et honeypot vérifiés par la commande, quota `RATE_CREATE`.
    let create = Router::new()
        .route("/events", post(events::post_event))
        .route_layer(middleware::from_fn_with_state(
            Guard::new(&state.limits, RouteClass::Create),
            limits::enforce,
        ));
    // Classe `candidature` : jeton de formulaire et honeypot vérifiés par la commande, quota
    // `RATE_CANDIDATURE`.
    let candidature = Router::new()
        .route(
            "/events/{event_id}/candidatures",
            post(candidatures::post_public_candidature),
        )
        .route(
            "/join/candidatures",
            post(candidatures::post_join_candidature),
        )
        .route_layer(middleware::from_fn_with_state(
            Guard::new(&state.limits, RouteClass::Candidature),
            limits::enforce,
        ));
    // Classe `write` : commandes sous le droit d'un lien, sans jeton de formulaire (AD-7), quota
    // `RATE_WRITE`.
    let write = Router::new()
        .route(
            "/participant/candidature",
            post(participant::post_candidature),
        )
        .route(
            "/participant/withdrawal",
            post(participant::post_withdrawal),
        )
        .route(
            "/organizer/candidature-status",
            post(organizer::post_candidature_status),
        )
        .route("/organizer/settings", post(organizer::post_settings))
        .route("/organizer/slots", post(organizer::post_slot))
        .route(
            "/organizer/slot-removal",
            post(organizer::post_slot_removal),
        )
        .route(
            "/organizer/cancellation",
            post(organizer::post_cancellation),
        )
        .route("/organizer/lock", post(organizer::post_lock))
        .route("/organizer/unlock", post(organizer::post_unlock))
        .route("/organizer/placement", post(organizer::post_placement))
        .route_layer(middleware::from_fn_with_state(
            Guard::new(&state.limits, RouteClass::Write),
            limits::enforce,
        ));
    // Classe `nokey` : ni IP ni limiteur.
    let api = Router::new()
        .route("/health", get(get_health))
        .merge(read)
        .merge(create)
        .merge(candidature)
        .merge(write)
        .fallback(api_not_found)
        .method_not_allowed_fallback(api_method_not_allowed);
    // `/e/{event_id}` sert `index.html` comme le repli, avec un `noindex` conditionnel (AD-3) ;
    // classe `nokey` comme tout `index.html`.
    Router::new()
        .nest("/api", api)
        .route("/e/{event_id}", get(events::event_page::<A>))
        .fallback(events::spa_fallback::<A>)
        .layer(middleware::from_fn(headers::security_headers))
        .with_state(state)
}

async fn get_health(State(state): State<AppState>) -> Response {
    match health(&state).await {
        Health::Ok => StatusCode::OK.into_response(),
        Health::Unavailable => Code::ServerUnavailable.into_response(),
    }
}

/// Une route `/api/*` inconnue est un 404, jamais `index.html`.
async fn api_not_found() -> Code {
    Code::ServerNotFound
}

/// Route `/api/*` connue, méthode non prévue.
async fn api_method_not_allowed() -> Code {
    Code::ServerMethodNotAllowed
}

/// Sert le routeur jusqu'à ce que `shutdown` se résolve, puis laisse finir les requêtes en cours.
/// L'adresse du pair (`ConnectInfo`) n'est lue qu'avec `TRUST_CLIENT_IP_HEADER=false`.
pub async fn serve(
    listener: tokio::net::TcpListener,
    router: Router,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> std::io::Result<()> {
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown)
    .await
}
