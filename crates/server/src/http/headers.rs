//! En-têtes posés par le binaire, pas par l'edge (AD-9). Aucun en-tête, URI ni corps de requête
//! n'est journalisé ici : les chemins de la SPA portent des secrets.

use axum::extract::Request;
use axum::http::header::{
    CACHE_CONTROL, CONTENT_SECURITY_POLICY, HeaderName, HeaderValue, REFERRER_POLICY,
    X_CONTENT_TYPE_OPTIONS,
};
use axum::middleware::Next;
use axum::response::Response;

/// Hash de build fixé à la compilation depuis `APP_BUILD`, `dev` sinon (voir `build.rs`).
pub const APP_BUILD: &str = env!("APP_BUILD_HASH");

pub const CSP: &str = "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; \
connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'";

pub const PERMISSIONS_POLICY: &str = "accelerometer=(), camera=(), geolocation=(), gyroscope=(), \
magnetometer=(), microphone=(), payment=(), usb=()";

pub const X_APP_BUILD: HeaderName = HeaderName::from_static("x-app-build");
pub const PERMISSIONS_POLICY_HEADER: HeaderName = HeaderName::from_static("permissions-policy");

pub const NO_STORE: &str = "no-store";
pub const NO_CACHE: &str = "no-cache";
pub const IMMUTABLE: &str = "public, max-age=31536000, immutable";

pub fn is_api(path: &str) -> bool {
    path == "/api" || path.starts_with("/api/")
}

pub async fn security_headers(req: Request, next: Next) -> Response {
    let api = is_api(req.uri().path());
    let mut res = next.run(req).await;
    let h = res.headers_mut();
    h.insert(CONTENT_SECURITY_POLICY, HeaderValue::from_static(CSP));
    h.insert(REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    h.insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    h.insert(
        PERMISSIONS_POLICY_HEADER,
        HeaderValue::from_static(PERMISSIONS_POLICY),
    );
    if api {
        h.insert(CACHE_CONTROL, HeaderValue::from_static(NO_STORE));
        h.insert(X_APP_BUILD, HeaderValue::from_static(APP_BUILD));
    } else if !h.contains_key(CACHE_CONTROL) {
        h.insert(CACHE_CONTROL, HeaderValue::from_static(NO_CACHE));
    }
    res
}
