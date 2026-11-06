//! `GET /api/reference` (AD-6) : le jeu courant en JSON, avec `ETag` fort et `If-None-Match`.
//!
//! Corps et `ETag` recalculés à chaque requête (quelques ko) : les garder en cache dans `infra` ferait
//! dépendre `infra` du DTO. L'API reste `no-store` (posé par `headers::security_headers`) ; le client
//! fait le conditionnel à la main (AD-5).

use axum::extract::State;
use axum::http::header::{CONTENT_TYPE, ETAG, IF_NONE_MATCH};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use data_encoding::HEXLOWER;
use sha2::{Digest, Sha256};

use super::dto;
use super::errors::Code;
use crate::app::AppState;

/// Octets de SHA-256 gardés dans l'`ETag` (128 bits, 32 caractères hexadécimaux).
const ETAG_BYTES: usize = 16;

pub async fn get_reference(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(set) = state.reference.get() else {
        return Code::ReferenceUnavailable.into_response();
    };
    let body = match serde_json::to_vec(&dto::ReferenceSet::from(set.as_ref())) {
        Ok(b) => b,
        Err(e) => {
            tracing::error!(error = %e, "reference serialization failed");
            return Code::ServerInternal.into_response();
        }
    };
    let tag = etag(&body);
    let etag_value = HeaderValue::from_str(&tag).expect("hex etag is a valid header value");
    if if_none_match(&headers, &tag) {
        return (StatusCode::NOT_MODIFIED, [(ETAG, etag_value)]).into_response();
    }
    (
        StatusCode::OK,
        [
            (CONTENT_TYPE, HeaderValue::from_static("application/json")),
            (ETAG, etag_value),
        ],
        body,
    )
        .into_response()
}

/// `ETag` fort : `"<hex de SHA-256 tronqué>"`.
pub fn etag(body: &[u8]) -> String {
    let digest = Sha256::digest(body);
    format!("\"{}\"", HEXLOWER.encode(&digest[..ETAG_BYTES]))
}

/// Vrai si un `If-None-Match` vaut `*` ou contient `tag`, en comparaison faible (`W/` ignoré, RFC 9110
/// §13.1.2). Plusieurs en-têtes et listes séparées par des virgules acceptés.
fn if_none_match(headers: &HeaderMap, tag: &str) -> bool {
    headers
        .get_all(IF_NONE_MATCH)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .map(str::trim)
        .any(|candidate| {
            candidate == "*" || candidate.strip_prefix("W/").unwrap_or(candidate) == tag
        })
}
