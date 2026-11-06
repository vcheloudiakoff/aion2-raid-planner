//! Registre unique des codes d'erreur (AD-5) et leur rendu `application/problem+json` (RFC 9457).
//!
//! Le serveur n'émet aucune phrase destinée à l'utilisateur : `title` est le libellé HTTP standard
//! du statut, le client traduit `errors.<code>` depuis ses locales (AD-12). La sous-commande
//! `list-error-codes` imprime les codes ; la CI les vérifie contre `fr.json` et `en.json`.

use axum::http::StatusCode;
use axum::http::header::{CONTENT_TYPE, HeaderValue, RETRY_AFTER};
use axum::response::{IntoResponse, Response};
use serde::Serialize;

pub const PROBLEM_JSON: &str = "application/problem+json";

/// `type` RFC 9457 quand le problème n'a pas d'autre sémantique que son statut et son `code`.
pub const PROBLEM_TYPE: &str = "about:blank";

/// `Retry-After` par défaut d'un 503 ou d'un 429, en secondes : un tick du rechargement (AD-6).
pub const DEFAULT_RETRY_AFTER_SECS: u64 = 60;

/// Une seule liste source : l'enum, [`Code::ALL`], [`Code::as_str`] et [`Code::status`] sont générés
/// ensemble, un code ne peut donc manquer à aucun d'eux.
macro_rules! codes {
    ($($variant:ident => $code:literal, $status:ident;)+) => {
        /// Code d'erreur `<entité>.<cas>`.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum Code {
            $($variant,)+
        }

        impl Code {
            /// Tous les codes, dans l'ordre de déclaration.
            pub const ALL: &[Code] = &[$(Code::$variant,)+];

            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Code::$variant => $code,)+
                }
            }

            pub const fn status(self) -> StatusCode {
                match self {
                    $(Code::$variant => StatusCode::$status,)+
                }
            }
        }
    };
}

codes! {
    AuthMissing => "auth.missing", UNAUTHORIZED;
    AuthNotFound => "auth.not_found", NOT_FOUND;
    AuthWrongRight => "auth.wrong_right", FORBIDDEN;
    ReferenceUnavailable => "reference.unavailable", SERVICE_UNAVAILABLE;
    ServerNotFound => "server.not_found", NOT_FOUND;
    ServerMethodNotAllowed => "server.method_not_allowed", METHOD_NOT_ALLOWED;
    ServerInternal => "server.internal", INTERNAL_SERVER_ERROR;
    ServerUnavailable => "server.unavailable", SERVICE_UNAVAILABLE;
}

/// Entités admises pour un code (AD-5, plus `server` décidé pour la story 1-5).
pub const ENTITIES: &[&str] = &[
    "event",
    "slot",
    "candidature",
    "placement",
    "confirmation",
    "reference",
    "auth",
    "form",
    "limit",
    "admin",
    "server",
];

impl Code {
    /// Codes triés : sortie de `list-error-codes`, un par ligne.
    pub fn sorted() -> Vec<&'static str> {
        let mut all: Vec<_> = Self::ALL.iter().map(|c| c.as_str()).collect();
        all.sort_unstable();
        all
    }
}

impl std::fmt::Display for Code {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Réponse d'erreur `/api/*`. Rien d'autre que le statut et le code : la cause d'une erreur interne
/// part dans `tracing`, jamais dans le corps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Problem {
    pub code: Code,
    retry_after: Option<u64>,
}

#[derive(Serialize)]
struct Body {
    #[serde(rename = "type")]
    kind: &'static str,
    title: &'static str,
    status: u16,
    code: &'static str,
}

impl Problem {
    /// `Retry-After` par défaut pour un 503 ou un 429, aucun sinon.
    pub fn new(code: Code) -> Self {
        let status = code.status();
        let retry_after = (status == StatusCode::SERVICE_UNAVAILABLE
            || status == StatusCode::TOO_MANY_REQUESTS)
            .then_some(DEFAULT_RETRY_AFTER_SECS);
        Self { code, retry_after }
    }

    /// Remplace le délai annoncé (sans effet hors 503 et 429).
    pub fn retry_after(mut self, secs: u64) -> Self {
        if self.retry_after.is_some() {
            self.retry_after = Some(secs);
        }
        self
    }
}

impl From<Code> for Problem {
    fn from(code: Code) -> Self {
        Self::new(code)
    }
}

impl IntoResponse for Problem {
    fn into_response(self) -> Response {
        let status = self.code.status();
        let body = Body {
            kind: PROBLEM_TYPE,
            title: status.canonical_reason().unwrap_or(""),
            status: status.as_u16(),
            code: self.code.as_str(),
        };
        // Quatre champs statiques : la sérialisation ne peut pas échouer.
        let json = serde_json::to_vec(&body).expect("problem body serializes");
        let mut res = (
            status,
            [(CONTENT_TYPE, HeaderValue::from_static(PROBLEM_JSON))],
            json,
        )
            .into_response();
        if let Some(secs) = self.retry_after {
            res.headers_mut()
                .insert(RETRY_AFTER, HeaderValue::from(secs));
        }
        res
    }
}

impl IntoResponse for Code {
    fn into_response(self) -> Response {
        Problem::new(self).into_response()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    fn is_snake(s: &str) -> bool {
        !s.is_empty()
            && !s.starts_with('_')
            && !s.ends_with('_')
            && s.chars().all(|c| c.is_ascii_lowercase() || c == '_')
    }

    #[test]
    fn codes_are_entity_dot_case() {
        for code in Code::ALL {
            let s = code.as_str();
            let (entity, case) = s.split_once('.').unwrap_or_else(|| panic!("{s}"));
            assert!(ENTITIES.contains(&entity), "{s}: unknown entity");
            assert!(is_snake(case), "{s}: case must be snake_case");
        }
    }

    #[test]
    fn codes_are_unique() {
        let strs: HashSet<_> = Code::ALL.iter().map(|c| c.as_str()).collect();
        assert_eq!(strs.len(), Code::ALL.len());
        let variants: HashSet<_> = Code::ALL.iter().collect();
        assert_eq!(variants.len(), Code::ALL.len());
    }

    #[test]
    fn statuses_are_errors() {
        for code in Code::ALL {
            let s = code.status();
            assert!(s.is_client_error() || s.is_server_error(), "{code}");
        }
    }

    #[test]
    fn sorted_is_sorted_and_complete() {
        let sorted = Code::sorted();
        assert_eq!(sorted.len(), Code::ALL.len());
        assert!(sorted.windows(2).all(|w| w[0] < w[1]));
    }

    async fn render(p: Problem) -> (Response, serde_json::Value) {
        let res = p.into_response();
        let (parts, body) = res.into_parts();
        let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        (
            Response::from_parts(parts, axum::body::Body::empty()),
            serde_json::from_slice(&bytes).unwrap(),
        )
    }

    #[tokio::test]
    async fn problem_shape() {
        for &code in Code::ALL {
            let (res, json) = render(Problem::new(code)).await;
            assert_eq!(res.status(), code.status());
            assert_eq!(res.headers()[CONTENT_TYPE], PROBLEM_JSON);
            let obj = json.as_object().unwrap();
            let mut keys: Vec<_> = obj.keys().map(String::as_str).collect();
            keys.sort_unstable();
            assert_eq!(keys, ["code", "status", "title", "type"], "{code}");
            assert_eq!(json["type"], PROBLEM_TYPE);
            assert_eq!(json["title"], code.status().canonical_reason().unwrap());
            assert_eq!(json["status"], code.status().as_u16());
            assert_eq!(json["code"], code.as_str());
            let retry = res.headers().get(RETRY_AFTER);
            if code.status() == StatusCode::SERVICE_UNAVAILABLE
                || code.status() == StatusCode::TOO_MANY_REQUESTS
            {
                assert_eq!(
                    retry.unwrap(),
                    DEFAULT_RETRY_AFTER_SECS.to_string().as_str()
                );
            } else {
                assert!(retry.is_none(), "{code}");
            }
        }
    }

    #[tokio::test]
    async fn retry_after_override_only_where_allowed() {
        let (res, _) = render(Problem::new(Code::ServerUnavailable).retry_after(5)).await;
        assert_eq!(res.headers()[RETRY_AFTER], "5");
        let (res, _) = render(Problem::new(Code::ServerNotFound).retry_after(5)).await;
        assert!(res.headers().get(RETRY_AFTER).is_none());
    }
}
