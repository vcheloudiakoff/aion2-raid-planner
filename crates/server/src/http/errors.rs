//! Registre unique des codes d'erreur (AD-5) et leur rendu `application/problem+json` (RFC 9457).
//!
//! Le serveur n'émet aucune phrase destinée à l'utilisateur : `title` est le libellé HTTP standard
//! du statut, le client traduit `errors.<code>` depuis ses locales (AD-12). La sous-commande
//! `list-error-codes` imprime les codes ; la CI les vérifie contre `fr.json` et `en.json`.

use std::collections::BTreeMap;

use axum::http::StatusCode;
use axum::http::header::{CONTENT_TYPE, HeaderValue, RETRY_AFTER};
use axum::response::{IntoResponse, Response};
use domain::candidature::{CandidatureError, OrganizerRefusal, ParticipantRefusal};
use domain::event::{EventError, EventRefusal};
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
    CandidatureCpOutOfRange => "candidature.cp_out_of_range", BAD_REQUEST;
    CandidatureFactionOutOfScope => "candidature.faction_out_of_scope", BAD_REQUEST;
    CandidatureLimitReached => "candidature.limit_reached", CONFLICT;
    CandidatureNoAvailability => "candidature.no_availability", BAD_REQUEST;
    CandidatureNotFound => "candidature.not_found", NOT_FOUND;
    CandidaturePreferenceNotAvailable => "candidature.preference_not_available", BAD_REQUEST;
    CandidaturePseudoInvalid => "candidature.pseudo_invalid", BAD_REQUEST;
    CandidatureServerOutOfRegion => "candidature.server_out_of_region", BAD_REQUEST;
    CandidatureSlotUnknown => "candidature.slot_unknown", BAD_REQUEST;
    CandidatureTransitionForbidden => "candidature.transition_forbidden", CONFLICT;
    CandidatureWithdrawn => "candidature.withdrawn", CONFLICT;
    EventClassBoundsInvalid => "event.class_bounds_invalid", BAD_REQUEST;
    EventClosed => "event.closed", CONFLICT;
    EventDiscordUrlInvalid => "event.discord_url_invalid", BAD_REQUEST;
    EventFactionScopeNotAllowed => "event.faction_scope_not_allowed", BAD_REQUEST;
    EventFieldImmutable => "event.field_immutable", CONFLICT;
    EventGroupCountOutOfRange => "event.group_count_out_of_range", BAD_REQUEST;
    EventLocked => "event.locked", CONFLICT;
    EventMinCpOutOfRange => "event.min_cp_out_of_range", BAD_REQUEST;
    EventNoUpcomingSlot => "event.no_upcoming_slot", CONFLICT;
    EventNotFound => "event.not_found", NOT_FOUND;
    EventNotLocked => "event.not_locked", CONFLICT;
    EventNoteInvalid => "event.note_invalid", BAD_REQUEST;
    FormMalformed => "form.malformed", BAD_REQUEST;
    FormRejected => "form.rejected", BAD_REQUEST;
    FormTokenInvalid => "form.token_invalid", BAD_REQUEST;
    FormTooFast => "form.too_fast", BAD_REQUEST;
    LimitClientIpMissing => "limit.client_ip_missing", BAD_REQUEST;
    LimitPaused => "limit.paused", SERVICE_UNAVAILABLE;
    LimitRateLimited => "limit.rate_limited", TOO_MANY_REQUESTS;
    ReferenceUnavailable => "reference.unavailable", SERVICE_UNAVAILABLE;
    ReferenceUnknown => "reference.unknown", BAD_REQUEST;
    ServerNotFound => "server.not_found", NOT_FOUND;
    ServerMethodNotAllowed => "server.method_not_allowed", METHOD_NOT_ALLOWED;
    ServerInternal => "server.internal", INTERNAL_SERVER_ERROR;
    ServerUnavailable => "server.unavailable", SERVICE_UNAVAILABLE;
    SlotCountOutOfRange => "slot.count_out_of_range", BAD_REQUEST;
    SlotDuplicate => "slot.duplicate", BAD_REQUEST;
    SlotDurationOutOfRange => "slot.duration_out_of_range", BAD_REQUEST;
    SlotInPast => "slot.in_past", BAD_REQUEST;
    SlotNotFound => "slot.not_found", NOT_FOUND;
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

impl From<EventError> for Code {
    fn from(e: EventError) -> Self {
        match e {
            EventError::ReferenceUnknown => Code::ReferenceUnknown,
            EventError::FactionScopeNotAllowed => Code::EventFactionScopeNotAllowed,
            EventError::SlotInPast => Code::SlotInPast,
            EventError::SlotCountOutOfRange => Code::SlotCountOutOfRange,
            EventError::SlotDuplicate => Code::SlotDuplicate,
            EventError::SlotDurationOutOfRange => Code::SlotDurationOutOfRange,
            EventError::NoteInvalid => Code::EventNoteInvalid,
            EventError::DiscordUrlInvalid => Code::EventDiscordUrlInvalid,
            EventError::MinCpOutOfRange => Code::EventMinCpOutOfRange,
            EventError::ClassBoundsInvalid => Code::EventClassBoundsInvalid,
            EventError::FieldImmutable => Code::EventFieldImmutable,
        }
    }
}

impl From<EventRefusal> for Code {
    fn from(r: EventRefusal) -> Self {
        match r {
            EventRefusal::Closed => Code::EventClosed,
            EventRefusal::Locked => Code::EventLocked,
            EventRefusal::NotLocked => Code::EventNotLocked,
        }
    }
}

impl From<CandidatureError> for Code {
    fn from(e: CandidatureError) -> Self {
        match e {
            CandidatureError::PseudoInvalid => Code::CandidaturePseudoInvalid,
            CandidatureError::ReferenceUnknown => Code::ReferenceUnknown,
            CandidatureError::ServerOutOfRegion => Code::CandidatureServerOutOfRegion,
            CandidatureError::FactionOutOfScope => Code::CandidatureFactionOutOfScope,
            CandidatureError::CpOutOfRange => Code::CandidatureCpOutOfRange,
            CandidatureError::NoAvailability => Code::CandidatureNoAvailability,
            CandidatureError::SlotUnknown => Code::CandidatureSlotUnknown,
            CandidatureError::SlotInPast => Code::SlotInPast,
        }
    }
}

impl From<ParticipantRefusal> for Code {
    fn from(r: ParticipantRefusal) -> Self {
        match r {
            ParticipantRefusal::EventClosed => Code::EventClosed,
            ParticipantRefusal::Withdrawn => Code::CandidatureWithdrawn,
            ParticipantRefusal::EventLocked => Code::EventLocked,
            ParticipantRefusal::TransitionForbidden => Code::CandidatureTransitionForbidden,
        }
    }
}

impl From<OrganizerRefusal> for Code {
    fn from(r: OrganizerRefusal) -> Self {
        match r {
            OrganizerRefusal::EventClosed => Code::EventClosed,
            OrganizerRefusal::TransitionForbidden => Code::CandidatureTransitionForbidden,
        }
    }
}

/// Réponse d'erreur `/api/*`. Le statut, le code et, pour une erreur de validation, les codes par
/// champ : la cause d'une erreur interne part dans `tracing`, jamais dans le corps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub code: Code,
    retry_after: Option<u64>,
    /// Chemin JSON camelCase (`slots.2.startsAt`) → code complet ; sérialisé seulement si présent.
    fields: Option<BTreeMap<String, &'static str>>,
}

#[derive(Serialize)]
struct Body<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    title: &'static str,
    status: u16,
    code: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    fields: Option<&'a BTreeMap<String, &'static str>>,
}

impl Problem {
    /// `Retry-After` par défaut pour un 503 ou un 429, aucun sinon.
    pub fn new(code: Code) -> Self {
        let status = code.status();
        let retry_after = (status == StatusCode::SERVICE_UNAVAILABLE
            || status == StatusCode::TOO_MANY_REQUESTS)
            .then_some(DEFAULT_RETRY_AFTER_SECS);
        Self {
            code,
            retry_after,
            fields: None,
        }
    }

    /// Erreurs de champ : `code` est celui de la première, `fields` les porte toutes. Liste vide =
    /// erreur interne (aucun appelant ne devrait la produire).
    pub fn fields<E: Into<Code>>(errors: Vec<(String, E)>) -> Self {
        let mut first = None;
        let mut fields = BTreeMap::new();
        for (path, e) in errors {
            let code = e.into();
            first.get_or_insert(code);
            // Un chemin vu deux fois garde sa première erreur.
            fields.entry(path).or_insert(code.as_str());
        }
        let Some(first) = first else {
            return Self::new(Code::ServerInternal);
        };
        let mut problem = Self::new(first);
        problem.fields = Some(fields);
        problem
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
            fields: self.fields.as_ref(),
        };
        // Champs statiques et chaînes : la sérialisation ne peut pas échouer.
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
            // Sans erreur de champ, `fields` est absent, jamais vide (AD-5).
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
    async fn field_errors_carry_every_path_and_the_first_code() {
        let (res, json) = render(Problem::fields(vec![
            ("region".into(), EventError::ReferenceUnknown),
            ("note".into(), EventError::NoteInvalid),
            ("slots.2.startsAt".into(), EventError::SlotInPast),
        ]))
        .await;
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
        assert_eq!(res.headers()[CONTENT_TYPE], PROBLEM_JSON);
        let mut keys: Vec<_> = json.as_object().unwrap().keys().cloned().collect();
        keys.sort_unstable();
        assert_eq!(keys, ["code", "fields", "status", "title", "type"]);
        assert_eq!(json["code"], "reference.unknown");
        assert_eq!(
            json["fields"],
            serde_json::json!({
                "region": "reference.unknown",
                "note": "event.note_invalid",
                "slots.2.startsAt": "slot.in_past",
            })
        );
    }

    #[test]
    fn every_event_error_but_a_frozen_field_maps_to_a_bad_request() {
        for e in [
            EventError::ReferenceUnknown,
            EventError::FactionScopeNotAllowed,
            EventError::SlotInPast,
            EventError::SlotCountOutOfRange,
            EventError::SlotDuplicate,
            EventError::SlotDurationOutOfRange,
            EventError::NoteInvalid,
            EventError::DiscordUrlInvalid,
            EventError::MinCpOutOfRange,
            EventError::ClassBoundsInvalid,
        ] {
            assert_eq!(Code::from(e).status(), StatusCode::BAD_REQUEST, "{e:?}");
        }
        assert_eq!(
            Code::from(EventError::FieldImmutable),
            Code::EventFieldImmutable
        );
        assert_eq!(Code::EventFieldImmutable.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn a_frozen_field_is_a_409_that_names_its_path() {
        let (res, json) = render(Problem::fields(vec![(
            "region".into(),
            EventError::FieldImmutable,
        )]))
        .await;
        assert_eq!(res.status(), StatusCode::CONFLICT);
        assert_eq!(json["code"], "event.field_immutable");
        assert_eq!(
            json["fields"],
            serde_json::json!({ "region": "event.field_immutable" })
        );
    }

    #[test]
    fn every_event_refusal_is_a_conflict_and_an_unknown_slot_is_404() {
        for (r, code) in [
            (EventRefusal::Closed, Code::EventClosed),
            (EventRefusal::Locked, Code::EventLocked),
            (EventRefusal::NotLocked, Code::EventNotLocked),
        ] {
            assert_eq!(Code::from(r), code);
            assert_eq!(code.status(), StatusCode::CONFLICT, "{code}");
        }
        assert_eq!(Code::EventNoUpcomingSlot.status(), StatusCode::CONFLICT);
        assert_eq!(
            Code::EventGroupCountOutOfRange.status(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(Code::SlotNotFound.as_str(), "slot.not_found");
        assert_eq!(Code::SlotNotFound.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn every_candidature_error_maps_to_a_bad_request() {
        for e in [
            CandidatureError::PseudoInvalid,
            CandidatureError::ReferenceUnknown,
            CandidatureError::ServerOutOfRegion,
            CandidatureError::FactionOutOfScope,
            CandidatureError::CpOutOfRange,
            CandidatureError::NoAvailability,
            CandidatureError::SlotUnknown,
            CandidatureError::SlotInPast,
        ] {
            assert_eq!(Code::from(e).status(), StatusCode::BAD_REQUEST, "{e:?}");
        }
    }

    #[tokio::test]
    async fn candidature_field_errors_use_the_same_shape() {
        let (res, json) = render(Problem::fields(vec![
            ("pseudo".into(), CandidatureError::PseudoInvalid),
            ("cp".into(), CandidatureError::CpOutOfRange),
        ]))
        .await;
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
        assert_eq!(json["code"], "candidature.pseudo_invalid");
        assert_eq!(
            json["fields"],
            serde_json::json!({
                "pseudo": "candidature.pseudo_invalid",
                "cp": "candidature.cp_out_of_range",
            })
        );
        let (res, _) = render(Problem::fields(Vec::<(String, CandidatureError)>::new())).await;
        assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn state_conflicts_are_409_and_absent_event_is_404() {
        for code in [
            Code::EventClosed,
            Code::CandidatureLimitReached,
            Code::EventLocked,
            Code::CandidatureWithdrawn,
            Code::CandidatureTransitionForbidden,
        ] {
            assert_eq!(code.status(), StatusCode::CONFLICT, "{code}");
        }
        assert_eq!(Code::EventNotFound.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            Code::CandidaturePreferenceNotAvailable.status(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn every_participant_refusal_is_a_conflict() {
        for (r, code) in [
            (ParticipantRefusal::EventClosed, Code::EventClosed),
            (ParticipantRefusal::Withdrawn, Code::CandidatureWithdrawn),
            (ParticipantRefusal::EventLocked, Code::EventLocked),
            (
                ParticipantRefusal::TransitionForbidden,
                Code::CandidatureTransitionForbidden,
            ),
        ] {
            assert_eq!(Code::from(r), code);
            assert_eq!(code.status(), StatusCode::CONFLICT, "{code}");
        }
    }

    #[test]
    fn every_organizer_refusal_is_a_conflict_and_a_foreign_candidature_is_404() {
        for (r, code) in [
            (OrganizerRefusal::EventClosed, Code::EventClosed),
            (
                OrganizerRefusal::TransitionForbidden,
                Code::CandidatureTransitionForbidden,
            ),
        ] {
            assert_eq!(Code::from(r), code);
            assert_eq!(code.status(), StatusCode::CONFLICT, "{code}");
        }
        assert_eq!(Code::CandidatureNotFound.as_str(), "candidature.not_found");
        assert_eq!(Code::CandidatureNotFound.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn retry_after_override_only_where_allowed() {
        let (res, _) = render(Problem::new(Code::ServerUnavailable).retry_after(5)).await;
        assert_eq!(res.headers()[RETRY_AFTER], "5");
        let (res, _) = render(Problem::new(Code::ServerNotFound).retry_after(5)).await;
        assert!(res.headers().get(RETRY_AFTER).is_none());
    }
}
