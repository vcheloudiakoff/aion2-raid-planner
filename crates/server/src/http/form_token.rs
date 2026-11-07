//! Jeton de formulaire sans état et honeypot (AD-7).
//!
//! Jeton : `base64url(ts ‖ nonce ‖ classe ‖ HMAC-SHA256(FORM_TOKEN_SECRET, ts ‖ nonce ‖ classe))`,
//! `ts` en secondes Unix (8 octets, gros-boutiste) selon la `Clock` injectée, nonce de 16 octets,
//! classe sur un octet. Accepté de `MIN_SUBMIT_DELAY_MS` à une heure d'âge, rejouable dans cette
//! fenêtre (le limiteur couvre le rejeu) ; aucune table. Ni le jeton ni le secret ne sont journalisés.

use std::time::Duration;

use axum::Json;
use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::extract::{FromRequest, Query, Request, State};
use axum::response::{IntoResponse, Response};
use data_encoding::BASE64URL_NOPAD;
use domain::Timestamp;
use hmac::{Hmac, KeyInit, Mac};
use serde::Deserialize;
use sha2::Sha256;

use crate::app::AppState;
use crate::config::FormTokenSecret;
use crate::http::dto;
use crate::http::errors::Code;

type HmacSha256 = Hmac<Sha256>;

pub use crate::infra::ids::NONCE_BYTES;
const TS_BYTES: usize = 8;
const MAC_BYTES: usize = 32;
const SIGNED_BYTES: usize = TS_BYTES + NONCE_BYTES + 1;
const TOKEN_BYTES: usize = SIGNED_BYTES + MAC_BYTES;

/// Âge maximal d'un jeton, en secondes.
pub const MAX_AGE_SECS: i64 = 3600;

/// Classe de route qu'un jeton autorise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormClass {
    Create,
    Candidature,
}

impl FormClass {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "create" => Some(Self::Create),
            "candidature" => Some(Self::Candidature),
            _ => None,
        }
    }

    /// Nom de la classe de route (`create` ou `candidature`).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Candidature => "candidature",
        }
    }

    const fn tag(self) -> u8 {
        match self {
            Self::Create => 1,
            Self::Candidature => 2,
        }
    }
}

/// Émetteur et vérificateur, porteur du secret HMAC.
#[derive(Debug, Clone)]
pub struct FormTokens {
    secret: FormTokenSecret,
    min_delay: Duration,
}

impl FormTokens {
    pub fn new(secret: FormTokenSecret, min_delay: Duration) -> Self {
        Self { secret, min_delay }
    }

    pub fn min_delay(&self) -> Duration {
        self.min_delay
    }

    fn mac(&self, signed: &[u8]) -> HmacSha256 {
        let mut mac =
            HmacSha256::new_from_slice(self.secret.expose()).expect("HMAC accepts any key length");
        mac.update(signed);
        mac
    }

    pub fn issue(&self, class: FormClass, now: Timestamp, nonce: [u8; NONCE_BYTES]) -> String {
        let mut bytes = Vec::with_capacity(TOKEN_BYTES);
        bytes.extend_from_slice(&now.unix_seconds().to_be_bytes());
        bytes.extend_from_slice(&nonce);
        bytes.push(class.tag());
        let tag = self.mac(&bytes).finalize().into_bytes();
        bytes.extend_from_slice(&tag);
        BASE64URL_NOPAD.encode(&bytes)
    }

    /// `form.token_invalid` : mal formé, MAC faux, mauvaise classe, daté du futur ou de plus d'une
    /// heure ; `form.too_fast` : plus jeune que `MIN_SUBMIT_DELAY_MS`.
    pub fn verify(&self, token: &str, class: FormClass, now: Timestamp) -> Result<(), Code> {
        let bytes = BASE64URL_NOPAD
            .decode(token.as_bytes())
            .map_err(|_| Code::FormTokenInvalid)?;
        if bytes.len() != TOKEN_BYTES {
            return Err(Code::FormTokenInvalid);
        }
        let (signed, tag) = bytes.split_at(SIGNED_BYTES);
        self.mac(signed)
            .verify_slice(tag)
            .map_err(|_| Code::FormTokenInvalid)?;
        if signed[SIGNED_BYTES - 1] != class.tag() {
            return Err(Code::FormTokenInvalid);
        }
        let ts = i64::from_be_bytes(signed[..TS_BYTES].try_into().expect("8 bytes"));
        let age = now.unix_seconds().saturating_sub(ts);
        if !(0..=MAX_AGE_SECS).contains(&age) {
            return Err(Code::FormTokenInvalid);
        }
        // Horloge à la seconde : âge entier, arrondi assumé (Design Notes de la story 1-7).
        if (age as u128) * 1000 < self.min_delay.as_millis() {
            return Err(Code::FormTooFast);
        }
        Ok(())
    }
}

/// Corps JSON d'un POST `create` ou `candidature` : jeton et honeypot à plat à côté des champs métier.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormEnvelope<T> {
    #[serde(default)]
    pub form_token: Option<String>,
    /// Honeypot : champ caché, laissé vide par un humain.
    #[serde(default)]
    pub website: Option<String>,
    #[serde(flatten)]
    pub data: T,
}

impl<T> FormEnvelope<T> {
    /// Honeypot rempli = `form.rejected` ; puis le jeton. Un refus est journalisé avec la classe et
    /// le code seulement : ni jeton, ni corps, ni IP.
    pub fn check(&self, tokens: &FormTokens, class: FormClass, now: Timestamp) -> Result<(), Code> {
        self.verdict(tokens, class, now).inspect_err(|code| {
            tracing::info!(class = class.as_str(), code = code.as_str(), "form refused");
        })
    }

    fn verdict(&self, tokens: &FormTokens, class: FormClass, now: Timestamp) -> Result<(), Code> {
        if self.website.as_deref().is_some_and(|w| !w.is_empty()) {
            return Err(Code::FormRejected);
        }
        let token = self.form_token.as_deref().ok_or(Code::FormTokenInvalid)?;
        tokens.verify(token, class, now)
    }
}

/// Corps JSON d'une route de commande. Tout rejet de `Json` (corps illisible, champ manquant ou de
/// mauvais type, `Content-Type` absent) devient 400 `form.malformed` en problem+json ; ni le corps ni
/// le détail du rejet ne sont journalisés, car ils peuvent contenir un secret ou une donnée de joueur.
#[derive(Debug)]
pub struct FormJson<T>(pub T);

impl<S, T> FromRequest<S> for FormJson<T>
where
    Json<T>: FromRequest<S, Rejection = JsonRejection>,
    S: Send + Sync,
{
    type Rejection = Code;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match Json::<T>::from_request(req, state).await {
            Ok(Json(value)) => Ok(Self(value)),
            Err(rejection) => {
                // Le statut seul distingue 413, 415 et corps illisible ; jamais le détail ni le corps.
                tracing::info!(
                    code = Code::FormMalformed.as_str(),
                    rejection_status = rejection.status().as_u16(),
                    "form refused"
                );
                Err(Code::FormMalformed)
            }
        }
    }
}

#[derive(Deserialize)]
pub struct FormTokenQuery {
    class: String,
}

/// `GET /api/form-token?class=create|candidature` → `{ "token": … }`.
pub async fn get_form_token(
    State(state): State<AppState>,
    query: Result<Query<FormTokenQuery>, QueryRejection>,
) -> Response {
    let Some(class) = query.ok().and_then(|Query(q)| FormClass::parse(&q.class)) else {
        return Code::FormTokenInvalid.into_response();
    };
    let token = state
        .form_tokens
        .issue(class, state.clock.now(), state.ids.nonce());
    Json(dto::FormToken { token }).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens(min_delay_ms: u64) -> FormTokens {
        let config = crate::config::test_config(&[]);
        FormTokens::new(
            config.form_token_secret,
            Duration::from_millis(min_delay_ms),
        )
    }

    fn at(secs: i64) -> Timestamp {
        Timestamp::from_unix_seconds(1_800_000_000 + secs).unwrap()
    }

    #[test]
    fn age_window() {
        let t = tokens(3000);
        let token = t.issue(FormClass::Create, at(0), [7; NONCE_BYTES]);
        assert_eq!(
            t.verify(&token, FormClass::Create, at(0)),
            Err(Code::FormTooFast)
        );
        assert_eq!(
            t.verify(&token, FormClass::Create, at(2)),
            Err(Code::FormTooFast)
        );
        assert_eq!(t.verify(&token, FormClass::Create, at(3)), Ok(()));
        assert_eq!(t.verify(&token, FormClass::Create, at(3600)), Ok(()));
        assert_eq!(
            t.verify(&token, FormClass::Create, at(3601)),
            Err(Code::FormTokenInvalid)
        );
        assert_eq!(
            t.verify(&token, FormClass::Create, at(-1)),
            Err(Code::FormTokenInvalid)
        );
        // Rejouable dans sa fenêtre.
        assert_eq!(t.verify(&token, FormClass::Create, at(10)), Ok(()));
    }

    #[test]
    fn wrong_class_tamper_or_garbage_is_invalid() {
        let t = tokens(0);
        let token = t.issue(FormClass::Candidature, at(0), [1; NONCE_BYTES]);
        assert_eq!(t.verify(&token, FormClass::Candidature, at(5)), Ok(()));
        assert_eq!(
            t.verify(&token, FormClass::Create, at(5)),
            Err(Code::FormTokenInvalid)
        );
        let mut bytes = BASE64URL_NOPAD.decode(token.as_bytes()).unwrap();
        for i in [0, TS_BYTES, SIGNED_BYTES - 1, TOKEN_BYTES - 1] {
            bytes[i] ^= 1;
            let tampered = BASE64URL_NOPAD.encode(&bytes);
            assert_eq!(
                t.verify(&tampered, FormClass::Candidature, at(5)),
                Err(Code::FormTokenInvalid),
                "byte {i}"
            );
            bytes[i] ^= 1;
        }
        for garbage in ["", "!!!", "AAAA", &token[..token.len() - 2]] {
            assert_eq!(
                t.verify(garbage, FormClass::Candidature, at(5)),
                Err(Code::FormTokenInvalid)
            );
        }
        // Autre secret : MAC faux.
        let other = FormTokens::new(
            crate::config::test_config(&[(
                "FORM_TOKEN_SECRET",
                "another-secret-of-at-least-32-bytes",
            )])
            .form_token_secret,
            Duration::ZERO,
        );
        assert_eq!(
            other.verify(&token, FormClass::Candidature, at(5)),
            Err(Code::FormTokenInvalid)
        );
    }

    #[test]
    fn nonce_changes_the_token() {
        let t = tokens(0);
        assert_ne!(
            t.issue(FormClass::Create, at(0), [1; NONCE_BYTES]),
            t.issue(FormClass::Create, at(0), [2; NONCE_BYTES])
        );
    }
}
