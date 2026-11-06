//! Routes Axum, assets embarqués et en-têtes. Même origine pour l'API et la SPA : pas de CORS.

pub mod assets;
pub mod dto;
pub mod errors;
pub mod headers;
pub mod reference;

use std::future::Future;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Router, middleware};
use rust_embed::RustEmbed;

use crate::app::AppState;
use crate::app::health::{Health, health};
use errors::Code;

/// Routeur de production, SPA servie depuis `web/dist/` embarqué.
pub fn router(state: AppState) -> Router {
    router_with::<assets::WebDist>(state)
}

/// Routeur sur un dossier d'assets quelconque (les tests embarquent une SPA de démonstration).
pub fn router_with<A: RustEmbed + Send + Sync + 'static>(state: AppState) -> Router {
    let api = Router::new()
        .route("/health", get(get_health))
        .route("/reference", get(reference::get_reference))
        .fallback(api_not_found)
        .method_not_allowed_fallback(api_method_not_allowed);
    Router::new()
        .nest("/api", api)
        .fallback(assets::serve::<A>)
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
pub async fn serve(
    listener: tokio::net::TcpListener,
    router: Router,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> std::io::Result<()> {
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown)
        .await
}
