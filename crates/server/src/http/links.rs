//! Extracteur unique des droits par lien secret (AD-3).
//!
//! Une route déclare le droit qu'elle exige par son type, `LinkSecret<Organizer>`, `LinkSecret<Join>`
//! ou `LinkSecret<Participant>`, et sa portée par ses paramètres de chemin : `event_id` toujours,
//! `candidature_id` en plus pour une route participant qui vise une candidature. Le secret se lit dans
//! l'en-tête `X-Link-Secret` et nulle part ailleurs : ni query string, ni cookie.
//!
//! Ordre des réponses : en-tête absent = 401 `auth.missing` ; valeur mal formée, en double ou
//! inconnue = 404 `auth.not_found` ; secret d'un autre droit ou d'une autre portée = 403
//! `auth.wrong_right`. Ni la valeur ni son hash ne sont journalisés (AD-15).
//!
//! Portée garantie, et ce qu'elle ne couvre pas :
//! - `Organizer` et `Join` : seul `event_id` est vérifié. Un paramètre `{candidature_id}` de la route
//!   n'est PAS contrôlé : la commande doit charger la candidature à l'intérieur de l'événement.
//! - `Participant` sur une route avec `{candidature_id}` : l'événement et la candidature sont vérifiés.
//! - `Participant` sur une route sans `{candidature_id}` : seul l'événement est vérifié ; le handler
//!   prend la candidature dans `link.candidature_id()`, jamais un id venu du corps de la requête.
//!
//! Trois exceptions à la portée par chemin : [`JoinLink`], pour les routes `/api/join*`,
//! [`ParticipantLink`], pour les routes `/api/participant*`, et [`OrganizerLink`], pour les routes
//! `/api/organizer*`. Les pages `/j/<secret>`, `/p/<secret>` et `/o/<secret>` ne connaissent que
//! leur secret, qui n'entre jamais dans une URL d'API : la portée vient donc de la ligne du lien
//! elle-même (l'événement, et la candidature pour `participant`). Seuls ces trois droits se
//! résolvent ainsi ; les mêmes en-tête, ordre de réponses et silence de journal s'appliquent. Un id
//! de candidature venu du corps d'une route organisateur n'est PAS contrôlé ici : la commande le
//! cherche dans l'événement du lien. Un lien dont l'événement a disparu se résout encore : la
//! requête ou la commande répond alors `auth.not_found`.

use std::marker::PhantomData;

use axum::extract::rejection::RawPathParamsRejection;
use axum::extract::{FromRequestParts, RawPathParams};
use axum::http::HeaderName;
use axum::http::request::Parts;

use crate::app::AppState;
use crate::http::errors::Code;
use crate::infra::ids::SECRET_LEN;
use crate::infra::links::{self, LinkRow, Right};

pub const X_LINK_SECRET: HeaderName = HeaderName::from_static("x-link-secret");

/// Paramètres de chemin qui portent la portée d'un lien.
pub const EVENT_ID_PARAM: &str = "event_id";
pub const CANDIDATURE_ID_PARAM: &str = "candidature_id";

/// Droit qu'une route exige.
pub trait RequiredRight: Send + Sync + 'static {
    const RIGHT: Right;
}

/// Lien organisateur `/o/`.
pub struct Organizer;
/// Lien de candidature directe `/j/`.
pub struct Join;
/// Lien participant `/p/`.
pub struct Participant;

impl RequiredRight for Organizer {
    const RIGHT: Right = Right::Organizer;
}
impl RequiredRight for Join {
    const RIGHT: Right = Right::Join;
}
impl RequiredRight for Participant {
    const RIGHT: Right = Right::Participant;
}

/// Droit `R` vérifié sur l'objet visé par la route. Ne contient pas le secret.
#[derive(Debug)]
pub struct LinkSecret<R> {
    event_id: String,
    candidature_id: Option<String>,
    right: PhantomData<fn() -> R>,
}

impl<R> LinkSecret<R> {
    /// Événement dans la portée du secret, égal à celui du chemin.
    pub fn event_id(&self) -> &str {
        &self.event_id
    }

    /// Candidature du secret participant ; `None` pour les autres droits.
    pub fn candidature_id(&self) -> Option<&str> {
        self.candidature_id.as_deref()
    }
}

/// Portée lue dans le chemin.
struct Scope {
    event_id: String,
    candidature_id: Option<String>,
}

/// Forme d'un secret : 43 caractères base64url sans padding.
fn well_formed(value: &[u8]) -> bool {
    value.len() == SECRET_LEN
        && value
            .iter()
            .all(|c| c.is_ascii_alphanumeric() || *c == b'-' || *c == b'_')
}

/// Erreur de programmation : la route n'a pas déclaré `event_id`.
fn undeclared_scope<R: RequiredRight>() -> Code {
    tracing::error!(
        right = R::RIGHT.as_str(),
        "link route declares no {EVENT_ID_PARAM} path parameter"
    );
    debug_assert!(
        false,
        "a LinkSecret route must declare a {{{EVENT_ID_PARAM}}} path parameter"
    );
    undeclared_scope_code()
}

/// Réponse à une route sans `event_id`, séparée du `debug_assert` pour être testée en debug.
const fn undeclared_scope_code() -> Code {
    Code::ServerInternal
}

/// `Ok(None)` : un paramètre de chemin n'est pas de l'UTF-8 valide, aucun secret ne peut y
/// correspondre.
async fn scope<R: RequiredRight>(
    parts: &mut Parts,
    state: &AppState,
) -> Result<Option<Scope>, Code> {
    let params = match RawPathParams::from_request_parts(parts, state).await {
        Ok(params) => params,
        Err(RawPathParamsRejection::InvalidUtf8InPathParam(_)) => return Ok(None),
        Err(_) => return Err(undeclared_scope::<R>()),
    };
    let mut event_id = None;
    let mut candidature_id = None;
    for (key, value) in &params {
        match key {
            EVENT_ID_PARAM => event_id = Some(value.to_owned()),
            CANDIDATURE_ID_PARAM => candidature_id = Some(value.to_owned()),
            _ => {}
        }
    }
    match event_id {
        Some(event_id) => Ok(Some(Scope {
            event_id,
            candidature_id,
        })),
        None => Err(undeclared_scope::<R>()),
    }
}

/// Lit `X-Link-Secret` et résout le lien, quel que soit son droit. `right` ne sert qu'au journal.
async fn lookup(parts: &Parts, state: &AppState, right: Right) -> Result<LinkRow, Code> {
    let mut values = parts.headers.get_all(X_LINK_SECRET).iter();
    let value = match (values.next(), values.next()) {
        (None, _) => return Err(Code::AuthMissing),
        (Some(_), Some(_)) => return Err(Code::AuthNotFound),
        (Some(value), None) => value.as_bytes(),
    };
    // Mal formé : indiscernable d'un inconnu pour le porteur, et aucune requête inutile.
    if !well_formed(value) {
        return Err(Code::AuthNotFound);
    }
    // `well_formed` garantit de l'ASCII.
    let secret = std::str::from_utf8(value).map_err(|_| Code::AuthNotFound)?;

    match links::find(&state.db.read, &links::hash(secret)).await {
        Ok(Some(row)) => Ok(row),
        Ok(None) => Err(Code::AuthNotFound),
        Err(e) => {
            // L'erreur sqlx ne porte pas les valeurs liées : ni secret ni hash dans le journal.
            tracing::error!(error = %e, right = right.as_str(), "link lookup failed");
            Err(Code::ServerInternal)
        }
    }
}

/// Lien `/j/` résolu sans `event_id` de chemin, pour les routes `/api/join*` : l'événement vient de
/// la ligne du lien. Un secret d'un autre droit = 403 `auth.wrong_right`. Ne contient pas le secret.
#[derive(Debug)]
pub struct JoinLink {
    event_id: String,
}

impl JoinLink {
    /// Événement du lien direct.
    pub fn event_id(&self) -> &str {
        &self.event_id
    }
}

impl FromRequestParts<AppState> for JoinLink {
    type Rejection = Code;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Code> {
        let row = lookup(parts, state, Right::Join).await?;
        if row.right != Right::Join {
            return Err(Code::AuthWrongRight);
        }
        Ok(Self {
            event_id: row.event_id,
        })
    }
}

/// Lien `/p/` résolu sans `event_id` de chemin, pour les routes `/api/participant*` : l'événement et
/// la candidature viennent de la ligne du lien, jamais du chemin ni du corps. Un secret d'un autre
/// droit = 403 `auth.wrong_right`. Ne contient pas le secret.
#[derive(Debug)]
pub struct ParticipantLink {
    event_id: String,
    candidature_id: String,
}

impl ParticipantLink {
    /// Événement de la candidature.
    pub fn event_id(&self) -> &str {
        &self.event_id
    }

    /// Candidature du lien personnel.
    pub fn candidature_id(&self) -> &str {
        &self.candidature_id
    }
}

impl FromRequestParts<AppState> for ParticipantLink {
    type Rejection = Code;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Code> {
        let row = lookup(parts, state, Right::Participant).await?;
        if row.right != Right::Participant {
            return Err(Code::AuthWrongRight);
        }
        // La table impose `candidature_id` pour un lien participant : son absence est une
        // corruption, jamais un droit.
        let Some(candidature_id) = row.candidature_id else {
            tracing::error!(event = %row.event_id, "participant link without candidature");
            return Err(Code::ServerInternal);
        };
        Ok(Self {
            event_id: row.event_id,
            candidature_id,
        })
    }
}

/// Lien `/o/` résolu sans `event_id` de chemin, pour les routes `/api/organizer*` : l'événement
/// vient de la ligne du lien, jamais du chemin ni du corps. Un secret d'un autre droit = 403
/// `auth.wrong_right`. Ne contient pas le secret.
#[derive(Debug)]
pub struct OrganizerLink {
    event_id: String,
}

impl OrganizerLink {
    /// Événement organisé.
    pub fn event_id(&self) -> &str {
        &self.event_id
    }
}

impl FromRequestParts<AppState> for OrganizerLink {
    type Rejection = Code;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Code> {
        let row = lookup(parts, state, Right::Organizer).await?;
        if row.right != Right::Organizer {
            return Err(Code::AuthWrongRight);
        }
        Ok(Self {
            event_id: row.event_id,
        })
    }
}

impl<R: RequiredRight> FromRequestParts<AppState> for LinkSecret<R> {
    type Rejection = Code;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Code> {
        let scope = scope::<R>(parts, state).await?;
        let row = lookup(parts, state, R::RIGHT).await?;

        let Some(scope) = scope else {
            return Err(Code::AuthWrongRight);
        };
        if row.right != R::RIGHT || row.event_id != scope.event_id {
            return Err(Code::AuthWrongRight);
        }
        if R::RIGHT == Right::Participant
            && let Some(wanted) = &scope.candidature_id
            && row.candidature_id.as_ref() != Some(wanted)
        {
            return Err(Code::AuthWrongRight);
        }
        Ok(Self {
            event_id: row.event_id,
            candidature_id: row.candidature_id,
            right: PhantomData,
        })
    }
}

#[cfg(test)]
mod tests {
    //! Routes de test, compilées sous `cfg(test)` seulement : le binaire de production n'en a aucune.

    use std::sync::Arc;

    use axum::Router;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::routing::get;
    use tower::ServiceExt;

    use super::*;
    use crate::infra::clock::FixedClock;
    use crate::infra::ids::{DeterministicIdGenerator, IdGenerator, Secret};
    use crate::infra::reference::ReferenceStore;
    use crate::infra::sqlite::{Db, shared_memory_options};

    async fn organizer(link: LinkSecret<Organizer>) -> String {
        link.event_id().to_owned()
    }

    async fn join(link: LinkSecret<Join>) -> String {
        link.event_id().to_owned()
    }

    async fn participant(link: LinkSecret<Participant>) -> String {
        format!(
            "{}/{}",
            link.event_id(),
            link.candidature_id().unwrap_or("-")
        )
    }

    async fn no_scope(_: LinkSecret<Organizer>) -> &'static str {
        "unreachable"
    }

    async fn join_link(link: JoinLink) -> String {
        link.event_id().to_owned()
    }

    async fn participant_link(link: ParticipantLink) -> String {
        format!("{}/{}", link.event_id(), link.candidature_id())
    }

    async fn organizer_link(link: OrganizerLink) -> String {
        link.event_id().to_owned()
    }

    fn router(state: AppState) -> Router {
        Router::new()
            .route("/api/test/events/{event_id}/organizer", get(organizer))
            .route("/api/test/events/{event_id}/join", get(join))
            .route("/api/test/events/{event_id}/participant", get(participant))
            .route(
                "/api/test/events/{event_id}/candidatures/{candidature_id}",
                get(participant),
            )
            .route("/api/test/no-scope", get(no_scope))
            .route("/api/test/join", get(join_link))
            .route("/api/test/participant", get(participant_link))
            .route("/api/test/organizer", get(organizer_link))
            .with_state(state)
    }

    struct Fixture {
        router: Router,
        state: AppState,
        e: String,
        f: String,
        c: String,
        d: String,
        organizer: Secret,
        join: Secret,
        participant: Secret,
    }

    async fn fixture(name: &str) -> Fixture {
        let db = Db::open(shared_memory_options(&format!("http_links_{name}")))
            .await
            .unwrap();
        db.migrate().await.unwrap();
        let ids = Arc::new(DeterministicIdGenerator::new());
        let state = AppState::new(
            db,
            Arc::new(FixedClock::new("2026-11-06T10:00:00Z".parse().unwrap())),
            ids.clone(),
            ReferenceStore::new(),
            &crate::config::test_config(&[]),
        );
        let [e, f, c, d] = [(); 4].map(|_| ids.public_id().to_string());
        let (organizer, join, participant) = (ids.secret(), ids.secret(), ids.secret());
        let mut conn = state.db.write.acquire().await.unwrap();
        for (secret, right, candidature_id) in [
            (&organizer, Right::Organizer, None),
            (&join, Right::Join, None),
            (&participant, Right::Participant, Some(c.clone())),
        ] {
            let row = LinkRow {
                right,
                event_id: e.clone(),
                candidature_id,
            };
            links::insert(&mut conn, secret, &row).await.unwrap();
        }
        drop(conn);
        Fixture {
            router: router(state.clone()),
            state,
            e,
            f,
            c,
            d,
            organizer,
            join,
            participant,
        }
    }

    async fn call(router: &Router, uri: &str, headers: &[(&str, &str)]) -> (StatusCode, String) {
        let mut req = Request::builder().uri(uri);
        for (name, value) in headers {
            req = req.header(*name, *value);
        }
        let res = router
            .clone()
            .oneshot(req.body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = res.status();
        let body = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, String::from_utf8(body.to_vec()).unwrap())
    }

    /// Statut et `code` problem+json attendus.
    async fn expect_code(router: &Router, uri: &str, headers: &[(&str, &str)], code: Code) {
        let (status, body) = call(router, uri, headers).await;
        assert_eq!(status, code.status(), "{uri} {body}");
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["code"], code.as_str(), "{uri}");
    }

    fn with(secret: &Secret) -> [(&'static str, &str); 1] {
        [("x-link-secret", secret.expose())]
    }

    #[tokio::test]
    async fn right_and_scope_match() {
        let fx = fixture("ok").await;
        let (status, body) = call(
            &fx.router,
            &format!("/api/test/events/{}/organizer", fx.e),
            &with(&fx.organizer),
        )
        .await;
        assert_eq!((status, body), (StatusCode::OK, fx.e.clone()));

        let (status, body) = call(
            &fx.router,
            &format!("/api/test/events/{}/join", fx.e),
            &with(&fx.join),
        )
        .await;
        assert_eq!((status, body), (StatusCode::OK, fx.e.clone()));

        let expected = format!("{}/{}", fx.e, fx.c);
        for uri in [
            format!("/api/test/events/{}/candidatures/{}", fx.e, fx.c),
            format!("/api/test/events/{}/participant", fx.e),
        ] {
            let (status, body) = call(&fx.router, &uri, &with(&fx.participant)).await;
            assert_eq!((status, body.as_str()), (StatusCode::OK, expected.as_str()));
        }
    }

    #[tokio::test]
    async fn absent_header_is_401_even_with_query_or_cookie() {
        let fx = fixture("absent").await;
        let secret = fx.organizer.expose();
        let base = format!("/api/test/events/{}/organizer", fx.e);
        expect_code(&fx.router, &base, &[], Code::AuthMissing).await;
        expect_code(
            &fx.router,
            &format!("{base}?secret={secret}&x-link-secret={secret}"),
            &[],
            Code::AuthMissing,
        )
        .await;
        let cookie = format!("X-Link-Secret={secret}; secret={secret}");
        expect_code(&fx.router, &base, &[("cookie", &cookie)], Code::AuthMissing).await;
    }

    #[tokio::test]
    async fn malformed_doubled_or_unknown_is_404() {
        let fx = fixture("not_found").await;
        let uri = format!("/api/test/events/{}/organizer", fx.e);
        let good = fx.organizer.expose();
        let short = &good[..SECRET_LEN - 1];
        let long = format!("{good}A");
        let bad_char = format!("{}+", &good[..SECRET_LEN - 1]);
        let padded = format!("{}=", &good[..SECRET_LEN - 1]);
        let unknown = "A".repeat(SECRET_LEN);
        for value in [short, &long, &bad_char, &padded, "", &unknown] {
            expect_code(
                &fx.router,
                &uri,
                &[("x-link-secret", value)],
                Code::AuthNotFound,
            )
            .await;
        }
        expect_code(
            &fx.router,
            &uri,
            &[("x-link-secret", good), ("x-link-secret", good)],
            Code::AuthNotFound,
        )
        .await;
    }

    #[tokio::test]
    async fn malformed_value_runs_no_query() {
        // Pool fermé : une recherche rendrait 500, une valeur mal formée doit rester un 404.
        let fx = fixture("malformed_no_sql").await;
        fx.state.db.read.close().await;
        let uri = format!("/api/test/events/{}/organizer", fx.e);
        let good = fx.organizer.expose();
        let bad_char = format!("{}+", &good[..SECRET_LEN - 1]);
        for headers in [
            vec![("x-link-secret", "too-short")],
            vec![("x-link-secret", bad_char.as_str())],
            vec![("x-link-secret", good), ("x-link-secret", good)],
        ] {
            expect_code(&fx.router, &uri, &headers, Code::AuthNotFound).await;
        }
    }

    #[tokio::test]
    async fn join_link_takes_its_scope_from_the_link_row() {
        let fx = fixture("join_link").await;
        let (status, body) = call(&fx.router, "/api/test/join", &with(&fx.join)).await;
        assert_eq!((status, body), (StatusCode::OK, fx.e.clone()));
        for secret in [&fx.organizer, &fx.participant] {
            expect_code(
                &fx.router,
                "/api/test/join",
                &with(secret),
                Code::AuthWrongRight,
            )
            .await;
        }
        expect_code(&fx.router, "/api/test/join", &[], Code::AuthMissing).await;
        let unknown = "A".repeat(SECRET_LEN);
        for value in ["short", unknown.as_str()] {
            expect_code(
                &fx.router,
                "/api/test/join",
                &[("x-link-secret", value)],
                Code::AuthNotFound,
            )
            .await;
        }
        let good = fx.join.expose();
        expect_code(
            &fx.router,
            "/api/test/join",
            &[("x-link-secret", good), ("x-link-secret", good)],
            Code::AuthNotFound,
        )
        .await;
    }

    #[tokio::test]
    async fn participant_link_takes_its_scope_from_the_link_row() {
        let fx = fixture("participant_link").await;
        let uri = "/api/test/participant";
        let (status, body) = call(&fx.router, uri, &with(&fx.participant)).await;
        assert_eq!(
            (status, body),
            (StatusCode::OK, format!("{}/{}", fx.e, fx.c))
        );
        for secret in [&fx.organizer, &fx.join] {
            expect_code(&fx.router, uri, &with(secret), Code::AuthWrongRight).await;
        }
        expect_code(&fx.router, uri, &[], Code::AuthMissing).await;
        let unknown = "A".repeat(SECRET_LEN);
        for value in ["short", unknown.as_str()] {
            expect_code(
                &fx.router,
                uri,
                &[("x-link-secret", value)],
                Code::AuthNotFound,
            )
            .await;
        }
        let good = fx.participant.expose();
        expect_code(
            &fx.router,
            uri,
            &[("x-link-secret", good), ("x-link-secret", good)],
            Code::AuthNotFound,
        )
        .await;
        // Ni query string ni cookie : seul l'en-tête compte.
        expect_code(
            &fx.router,
            &format!("{uri}?x-link-secret={good}"),
            &[("cookie", &format!("X-Link-Secret={good}"))],
            Code::AuthMissing,
        )
        .await;
    }

    #[tokio::test]
    async fn organizer_link_takes_its_scope_from_the_link_row() {
        let fx = fixture("organizer_link").await;
        let uri = "/api/test/organizer";
        let (status, body) = call(&fx.router, uri, &with(&fx.organizer)).await;
        assert_eq!((status, body), (StatusCode::OK, fx.e.clone()));
        for secret in [&fx.join, &fx.participant] {
            expect_code(&fx.router, uri, &with(secret), Code::AuthWrongRight).await;
        }
        expect_code(&fx.router, uri, &[], Code::AuthMissing).await;
        let unknown = "A".repeat(SECRET_LEN);
        for value in ["short", unknown.as_str()] {
            expect_code(
                &fx.router,
                uri,
                &[("x-link-secret", value)],
                Code::AuthNotFound,
            )
            .await;
        }
        let good = fx.organizer.expose();
        expect_code(
            &fx.router,
            uri,
            &[("x-link-secret", good), ("x-link-secret", good)],
            Code::AuthNotFound,
        )
        .await;
        // Ni query string ni cookie : seul l'en-tête compte.
        expect_code(
            &fx.router,
            &format!("{uri}?x-link-secret={good}"),
            &[("cookie", &format!("X-Link-Secret={good}"))],
            Code::AuthMissing,
        )
        .await;
    }

    #[test]
    fn undeclared_scope_answers_500() {
        assert_eq!(undeclared_scope_code(), Code::ServerInternal);
    }

    #[tokio::test]
    async fn other_right_is_403() {
        let fx = fixture("wrong_right").await;
        let e = &fx.e;
        for (uri, secret) in [
            (format!("/api/test/events/{e}/organizer"), &fx.join),
            (format!("/api/test/events/{e}/organizer"), &fx.participant),
            (format!("/api/test/events/{e}/join"), &fx.organizer),
            (format!("/api/test/events/{e}/participant"), &fx.organizer),
            (
                format!("/api/test/events/{e}/candidatures/{}", fx.c),
                &fx.join,
            ),
        ] {
            expect_code(&fx.router, &uri, &with(secret), Code::AuthWrongRight).await;
        }
    }

    #[tokio::test]
    async fn other_scope_is_403() {
        let fx = fixture("wrong_scope").await;
        expect_code(
            &fx.router,
            &format!("/api/test/events/{}/organizer", fx.f),
            &with(&fx.organizer),
            Code::AuthWrongRight,
        )
        .await;
        expect_code(
            &fx.router,
            &format!("/api/test/events/{}/candidatures/{}", fx.e, fx.d),
            &with(&fx.participant),
            Code::AuthWrongRight,
        )
        .await;
        expect_code(
            &fx.router,
            &format!("/api/test/events/{}/candidatures/{}", fx.f, fx.c),
            &with(&fx.participant),
            Code::AuthWrongRight,
        )
        .await;
        // Paramètre de chemin non UTF-8 : rien ne peut correspondre.
        expect_code(
            &fx.router,
            "/api/test/events/%FF/organizer",
            &with(&fx.organizer),
            Code::AuthWrongRight,
        )
        .await;
    }

    /// Journal capturé en mémoire, pour vérifier ce qui en sort.
    #[derive(Clone, Default)]
    struct Captured(Arc<std::sync::Mutex<Vec<u8>>>);

    impl std::io::Write for Captured {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn database_error_is_500_and_logged_without_the_secret() {
        let fx = fixture("db_error").await;
        fx.state.db.read.close().await;
        let captured = Captured::default();
        let writer = captured.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .with_ansi(false)
            .with_writer(move || writer.clone())
            .finish();
        // `tokio::test` tourne sur un seul fil : le collecteur par défaut couvre la requête.
        let guard = tracing::subscriber::set_default(subscriber);
        expect_code(
            &fx.router,
            &format!("/api/test/events/{}/organizer", fx.e),
            &with(&fx.organizer),
            Code::ServerInternal,
        )
        .await;
        drop(guard);
        let log = String::from_utf8(captured.0.lock().unwrap().clone()).unwrap();
        assert!(log.contains("link lookup failed"), "{log}");
        let hash = links::hash(fx.organizer.expose());
        for leak in [
            fx.organizer.expose().to_owned(),
            data_encoding::HEXLOWER.encode(&hash),
            data_encoding::HEXUPPER.encode(&hash),
            data_encoding::BASE64URL_NOPAD.encode(&hash),
            data_encoding::BASE64.encode(&hash),
            format!("{hash:?}"),
        ] {
            assert!(!log.contains(&leak), "{log}");
        }
    }

    #[tokio::test]
    #[cfg_attr(debug_assertions, should_panic(expected = "event_id"))]
    async fn route_without_event_id_is_a_programming_error() {
        let fx = fixture("no_scope").await;
        expect_code(
            &fx.router,
            "/api/test/no-scope",
            &with(&fx.organizer),
            Code::ServerInternal,
        )
        .await;
    }
}
