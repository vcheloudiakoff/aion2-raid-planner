//! SPA embarquée par `rust-embed` (AD-1) : toute route GET sans extension renvoie `index.html`, un
//! asset absent renvoie 404.

use axum::body::Body;
use axum::http::header::{ALLOW, CACHE_CONTROL, CONTENT_LENGTH, CONTENT_TYPE, HeaderValue};
use axum::http::{Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

use super::headers::{IMMUTABLE, NO_CACHE, is_api};

/// Le build Vite. `web/dist/.gitkeep` garantit que le dossier existe sans build web.
#[derive(RustEmbed)]
#[folder = "../../web/dist/"]
#[exclude = ".gitkeep"]
pub struct WebDist;

const INDEX: &str = "index.html";

pub async fn serve<A: RustEmbed>(method: Method, uri: Uri) -> Response {
    // `/api/` (barre finale) échappe au routeur imbriqué : jamais `index.html` sous l'API, quelle
    // que soit la méthode.
    if is_api(uri.path()) {
        return StatusCode::NOT_FOUND.into_response();
    }
    if method != Method::GET && method != Method::HEAD {
        return (StatusCode::METHOD_NOT_ALLOWED, [(ALLOW, "GET, HEAD")]).into_response();
    }
    let path = uri.path().trim_start_matches('/');
    let last = path.rsplit('/').next().unwrap_or_default();
    let (name, cache) = if last.contains('.') {
        if !is_safe(path) {
            return StatusCode::NOT_FOUND.into_response();
        }
        let cache = if path.starts_with("assets/") {
            IMMUTABLE
        } else {
            NO_CACHE
        };
        (path, cache)
    } else {
        (INDEX, NO_CACHE)
    };
    let Some(file) = A::get(name) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let mime = HeaderValue::from_str(file.metadata.mimetype())
        .unwrap_or(HeaderValue::from_static("application/octet-stream"));
    let builder = Response::builder()
        .header(CONTENT_TYPE, mime)
        .header(CACHE_CONTROL, cache);
    let response = if method == Method::HEAD {
        // Même `Content-Length` qu'en GET, corps vide.
        builder
            .header(CONTENT_LENGTH, file.data.len())
            .body(Body::empty())
    } else {
        builder.body(Body::from(file.data.into_owned()))
    };
    response.expect("valid asset response")
}

/// Refuse les segments vides, cachés ou remontants, et les antislashs.
fn is_safe(path: &str) -> bool {
    !path.contains('\\')
        && path
            .split('/')
            .all(|s| !s.is_empty() && !s.starts_with('.'))
}
