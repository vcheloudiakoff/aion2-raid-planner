//! Matrice de la story 1-7 sur des routes de test, compilées sous `cfg(test)` seulement : le binaire
//! de production n'en a aucune. Adresses de documentation uniquement (192.0.2.0/24, 2001:db8::/32).

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::extract::{ConnectInfo, State};
use axum::http::{HeaderMap, Method, Request, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router, middleware};
use serde::Deserialize;
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::app::AppState;
use crate::config::test_config;
use crate::http::errors::Code;
use crate::http::form_token::{self, FormClass, FormEnvelope};
use crate::http::limits::{self, Guard, RouteClass, Trip};
use crate::infra::clock::FixedClock;
use crate::infra::ids::DeterministicIdGenerator;
use crate::infra::reference::ReferenceStore;
use crate::infra::sqlite::{Db, shared_memory_options};

#[derive(Debug, Deserialize)]
struct Probe {
    #[serde(default)]
    name: String,
}

/// Gabarit d'une commande réelle : jeton et honeypot, `check` du disjoncteur, écriture, `record`.
/// Dans une vraie commande, `check` et `record` se font dans la transaction d'écriture, que le pool
/// d'écriture à une seule connexion sérialise : la paire est alors atomique (voir `Breaker`).
async fn submit(
    state: &AppState,
    class: FormClass,
    trip: Trip,
    body: FormEnvelope<Probe>,
) -> Response {
    if let Err(code) = body.check(&state.form_tokens, class, state.clock.now()) {
        return code.into_response();
    }
    if let Err(problem) = state.limits.breaker.check(trip) {
        return problem.into_response();
    }
    // Écriture commise ici dans une vraie commande.
    state.limits.breaker.record(trip);
    (StatusCode::CREATED, body.data.name).into_response()
}

async fn create(State(state): State<AppState>, Json(body): Json<FormEnvelope<Probe>>) -> Response {
    submit(&state, FormClass::Create, Trip::Events, body).await
}

async fn candidature(
    State(state): State<AppState>,
    Json(body): Json<FormEnvelope<Probe>>,
) -> Response {
    submit(&state, FormClass::Candidature, Trip::Candidatures, body).await
}

async fn ok() -> &'static str {
    "ok"
}

fn router(state: AppState) -> Router {
    let keyed =
        |class| middleware::from_fn_with_state(Guard::new(&state.limits, class), limits::enforce);
    Router::new()
        .merge(
            Router::new()
                .route("/api/test/create", post(create))
                .route_layer(keyed(RouteClass::Create)),
        )
        .merge(
            Router::new()
                .route("/api/test/candidature", post(candidature))
                .route_layer(keyed(RouteClass::Candidature)),
        )
        .merge(
            Router::new()
                .route("/api/test/read", get(ok))
                .route("/api/form-token", get(form_token::get_form_token))
                .route_layer(keyed(RouteClass::Read)),
        )
        .route("/api/test/nokey", get(ok))
        .with_state(state)
}

struct Fixture {
    router: Router,
    clock: Arc<FixedClock>,
    state: AppState,
}

const T0: &str = "2026-11-06T10:00:00Z";

async fn fixture(name: &str, vars: &[(&str, &str)]) -> Fixture {
    let db = Db::open(shared_memory_options(&format!("http_abuse_{name}")))
        .await
        .unwrap();
    let clock = Arc::new(FixedClock::new(T0.parse().unwrap()));
    let state = AppState::new(
        db,
        clock.clone(),
        Arc::new(DeterministicIdGenerator::new()),
        ReferenceStore::new(),
        &test_config(vars),
    );
    Fixture {
        router: router(state.clone()),
        clock,
        state,
    }
}

impl Fixture {
    fn advance(&self, secs: i64) {
        let now = self.state.clock.now().unix_seconds();
        self.clock
            .set(domain::Timestamp::from_unix_seconds(now + secs).unwrap());
    }

    async fn call(
        &self,
        method: Method,
        uri: &str,
        headers: &[(&str, &str)],
        body: Option<Value>,
        peer: Option<&str>,
    ) -> (StatusCode, HeaderMap, String) {
        let mut req = Request::builder().method(method).uri(uri);
        for (name, value) in headers {
            req = req.header(*name, *value);
        }
        let body = match body {
            Some(v) => {
                req = req.header("content-type", "application/json");
                Body::from(v.to_string())
            }
            None => Body::empty(),
        };
        let mut req = req.body(body).unwrap();
        if let Some(p) = peer {
            req.extensions_mut()
                .insert(ConnectInfo::<SocketAddr>(p.parse().unwrap()));
        }
        let res = self.router.clone().oneshot(req).await.unwrap();
        let status = res.status();
        let headers = res.headers().clone();
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, headers, String::from_utf8(bytes.to_vec()).unwrap())
    }

    async fn read(&self, ip: &str) -> (StatusCode, HeaderMap, String) {
        self.call(
            Method::GET,
            "/api/test/read",
            &[("x-client-ip", ip)],
            None,
            None,
        )
        .await
    }

    async fn token(&self, class: &str) -> String {
        let (status, _, body) = self
            .call(
                Method::GET,
                &format!("/api/form-token?class={class}"),
                &[("x-client-ip", "192.0.2.200")],
                None,
                None,
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let v: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v.as_object().unwrap().len(), 1, "{body}");
        v["token"].as_str().unwrap().to_owned()
    }

    async fn post(&self, path: &str, ip: &str, body: Value) -> (StatusCode, HeaderMap, String) {
        self.call(Method::POST, path, &[("x-client-ip", ip)], Some(body), None)
            .await
    }
}

fn assert_code(res: &(StatusCode, HeaderMap, String), code: Code) {
    let (status, _, body) = res;
    assert_eq!(*status, code.status(), "{body}");
    let v: Value = serde_json::from_str(body).unwrap();
    assert_eq!(v["code"], code.as_str(), "{body}");
}

fn retry_after(res: &(StatusCode, HeaderMap, String)) -> u64 {
    res.1["retry-after"].to_str().unwrap().parse().unwrap()
}

#[tokio::test]
async fn missing_or_multiple_ip_is_400_on_keyed_routes() {
    let fx = fixture("ip_missing", &[]).await;
    let res = fx
        .call(
            Method::GET,
            "/api/test/read",
            &[],
            None,
            Some("192.0.2.9:4000"),
        )
        .await;
    assert_code(&res, Code::LimitClientIpMissing);
    for headers in [
        &[("x-client-ip", "192.0.2.1"), ("x-client-ip", "192.0.2.2")][..],
        &[("x-client-ip", "192.0.2.1, 192.0.2.2")][..],
        &[("x-forwarded-for", "192.0.2.1")][..],
    ] {
        let res = fx
            .call(Method::GET, "/api/test/read", headers, None, None)
            .await;
        assert_code(&res, Code::LimitClientIpMissing);
    }
    let token = fx.token("create").await;
    fx.advance(5);
    let res = fx
        .call(
            Method::POST,
            "/api/test/create",
            &[],
            Some(json!({ "formToken": token })),
            None,
        )
        .await;
    assert_code(&res, Code::LimitClientIpMissing);
}

#[tokio::test]
async fn nokey_route_needs_no_ip() {
    let fx = fixture("nokey", &[("RATE_READ", "1/h")]).await;
    for _ in 0..3 {
        let (status, _, _) = fx
            .call(Method::GET, "/api/test/nokey", &[], None, None)
            .await;
        assert_eq!(status, StatusCode::OK);
    }
}

#[tokio::test]
async fn untrusted_header_uses_peer_address() {
    let fx = fixture(
        "peer",
        &[("TRUST_CLIENT_IP_HEADER", "false"), ("RATE_READ", "1/h")],
    )
    .await;
    let get = |ip: &'static str, peer: &'static str| {
        let fx = &fx;
        async move {
            fx.call(
                Method::GET,
                "/api/test/read",
                &[("x-client-ip", ip)],
                None,
                Some(peer),
            )
            .await
            .0
        }
    };
    assert_eq!(get("192.0.2.7", "192.0.2.5:4000").await, StatusCode::OK);
    // Même pair, autre en-tête : l'en-tête est ignoré.
    assert_eq!(
        get("192.0.2.8", "192.0.2.5:4001").await,
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(get("192.0.2.5", "192.0.2.6:4000").await, StatusCode::OK);
    // Sans `ConnectInfo` : pas de clé.
    let res = fx
        .call(
            Method::GET,
            "/api/test/read",
            &[("x-client-ip", "192.0.2.5")],
            None,
            None,
        )
        .await;
    assert_code(&res, Code::LimitClientIpMissing);
}

#[tokio::test]
async fn create_quota_exceeded_is_429_with_retry_after() {
    let fx = fixture("quota_create", &[("RATE_CREATE", "2/h")]).await;
    let token = fx.token("create").await;
    fx.advance(5);
    for _ in 0..2 {
        let (status, _, body) = fx
            .post(
                "/api/test/create",
                "192.0.2.10",
                json!({ "formToken": token }),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
    }
    let res = fx
        .post(
            "/api/test/create",
            "192.0.2.10",
            json!({ "formToken": token }),
        )
        .await;
    assert_code(&res, Code::LimitRateLimited);
    let wait = retry_after(&res);
    assert!((1..=1800).contains(&wait), "{wait}");
    // Autre IP : son propre compteur.
    let (status, _, _) = fx
        .post(
            "/api/test/create",
            "192.0.2.11",
            json!({ "formToken": token }),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    // Les classes ont des compteurs distincts.
    let (status, _, _) = fx.read("192.0.2.10").await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn ipv6_is_limited_per_64() {
    let fx = fixture("ipv6", &[("RATE_READ", "1/h")]).await;
    assert_eq!(fx.read("2001:db8::1").await.0, StatusCode::OK);
    let res = fx.read("2001:db8::2").await;
    assert_code(&res, Code::LimitRateLimited);
    assert!(retry_after(&res) >= 1);
    assert_eq!(fx.read("2001:db8:0:1::1").await.0, StatusCode::OK);
    assert_eq!(fx.read("192.0.2.1").await.0, StatusCode::OK);
}

#[tokio::test]
async fn form_token_matrix() {
    let fx = fixture(
        "token",
        &[("RATE_CREATE", "100/h"), ("RATE_CANDIDATURE", "100/h")],
    )
    .await;
    let create = fx.token("create").await;
    let cand = fx.token("candidature").await;

    // Trop jeune (défaut 3000 ms).
    fx.advance(1);
    let res = fx
        .post(
            "/api/test/create",
            "192.0.2.20",
            json!({ "formToken": create }),
        )
        .await;
    assert_code(&res, Code::FormTooFast);

    // Émis il y a 5 s : accepté, champs métier lus à plat.
    fx.advance(4);
    let (status, _, body) = fx
        .post(
            "/api/test/create",
            "192.0.2.20",
            json!({ "formToken": create, "website": "", "name": "Zoe" }),
        )
        .await;
    assert_eq!((status, body.as_str()), (StatusCode::CREATED, "Zoe"));
    let (status, _, _) = fx
        .post(
            "/api/test/candidature",
            "192.0.2.20",
            json!({ "formToken": cand }),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);

    // Sans jeton.
    let res = fx
        .post("/api/test/create", "192.0.2.20", json!({ "name": "Zoe" }))
        .await;
    assert_code(&res, Code::FormTokenInvalid);
    let res = fx
        .post(
            "/api/test/create",
            "192.0.2.20",
            json!({ "formToken": null }),
        )
        .await;
    assert_code(&res, Code::FormTokenInvalid);

    // Mauvaise classe.
    let res = fx
        .post(
            "/api/test/create",
            "192.0.2.20",
            json!({ "formToken": cand }),
        )
        .await;
    assert_code(&res, Code::FormTokenInvalid);

    // MAC altéré : un caractère changé.
    let mut tampered = create.clone().into_bytes();
    let i = tampered.len() / 2;
    tampered[i] = if tampered[i] == b'A' { b'B' } else { b'A' };
    let tampered = String::from_utf8(tampered).unwrap();
    let res = fx
        .post(
            "/api/test/create",
            "192.0.2.20",
            json!({ "formToken": tampered }),
        )
        .await;
    assert_code(&res, Code::FormTokenInvalid);

    // Honeypot rempli, même avec un jeton valide.
    let res = fx
        .post(
            "/api/test/create",
            "192.0.2.20",
            json!({ "formToken": create, "website": "https://spam.example" }),
        )
        .await;
    assert_code(&res, Code::FormRejected);

    // Expiré : plus d'une heure.
    fx.advance(3600 - 5 + 1);
    let res = fx
        .post(
            "/api/test/create",
            "192.0.2.20",
            json!({ "formToken": create }),
        )
        .await;
    assert_code(&res, Code::FormTokenInvalid);
}

#[tokio::test]
async fn form_token_route_rejects_unknown_class() {
    let fx = fixture("token_route", &[]).await;
    for uri in [
        "/api/form-token",
        "/api/form-token?class=",
        "/api/form-token?class=write",
        "/api/form-token?class=CREATE",
    ] {
        let res = fx
            .call(
                Method::GET,
                uri,
                &[("x-client-ip", "192.0.2.30")],
                None,
                None,
            )
            .await;
        assert_code(&res, Code::FormTokenInvalid);
    }
    let res = fx
        .call(Method::GET, "/api/form-token?class=create", &[], None, None)
        .await;
    assert_code(&res, Code::LimitClientIpMissing);
}

#[tokio::test]
async fn breaker_pauses_then_reopens_when_the_window_slides() {
    let fx = fixture(
        "breaker",
        &[("MAX_EVENTS_PER_HOUR", "2"), ("RATE_CREATE", "100/h")],
    )
    .await;
    let token = fx.token("create").await;
    fx.advance(5);
    for ip in ["192.0.2.40", "192.0.2.41"] {
        let (status, _, _) = fx
            .post("/api/test/create", ip, json!({ "formToken": token }))
            .await;
        assert_eq!(status, StatusCode::CREATED);
    }
    fx.advance(600);
    let res = fx
        .post(
            "/api/test/create",
            "192.0.2.42",
            json!({ "formToken": token }),
        )
        .await;
    assert_code(&res, Code::LimitPaused);
    assert_eq!(retry_after(&res), 3000);

    // L'autre détente reste ouverte.
    let cand = fx.token("candidature").await;
    fx.advance(5);
    let (status, _, _) = fx
        .post(
            "/api/test/candidature",
            "192.0.2.42",
            json!({ "formToken": cand }),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);

    // Une heure après les deux écritures : de nouveau accepté.
    fx.advance(3000 - 5);
    let token = fx.token("create").await;
    fx.advance(5);
    let (status, _, body) = fx
        .post(
            "/api/test/create",
            "192.0.2.42",
            json!({ "formToken": token }),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
}

#[test]
fn breaker_refusal_does_not_count() {
    let clock = Arc::new(FixedClock::new(T0.parse().unwrap()));
    let breaker = limits::Breaker::new(clock.clone(), 1, 1);
    assert!(breaker.check(Trip::Events).is_ok());
    breaker.record(Trip::Events);
    assert!(breaker.check(Trip::Events).is_err());
    assert!(breaker.check(Trip::Events).is_err());
    let t = domain::Timestamp::from_unix_seconds(
        T0.parse::<domain::Timestamp>().unwrap().unix_seconds() + 3599,
    )
    .unwrap();
    clock.set(t);
    assert!(breaker.check(Trip::Events).is_err());
    clock.set(domain::Timestamp::from_unix_seconds(t.unix_seconds() + 1).unwrap());
    assert!(breaker.check(Trip::Events).is_ok());
}

#[test]
fn breaker_survives_a_clock_stepping_backward() {
    let clock = Arc::new(FixedClock::new(T0.parse().unwrap()));
    let breaker = limits::Breaker::new(clock.clone(), 1, 1);
    let t0 = T0.parse::<domain::Timestamp>().unwrap().unix_seconds();
    breaker.record(Trip::Events);
    // Horloge reculée de deux heures : l'écriture « future » ne compte plus.
    clock.set(domain::Timestamp::from_unix_seconds(t0 - 7200).unwrap());
    assert!(breaker.check(Trip::Events).is_ok());
    // Retour en avant : l'attente annoncée reste dans 1..=3600.
    breaker.record(Trip::Events);
    clock.set(domain::Timestamp::from_unix_seconds(t0 - 7200 + 10).unwrap());
    let problem = breaker.check(Trip::Events).unwrap_err();
    let res = axum::response::IntoResponse::into_response(problem);
    let wait: i64 = res.headers()["retry-after"]
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    assert!((1..=3600).contains(&wait), "{wait}");
    assert_eq!(wait, 3590);
}

#[tokio::test]
async fn retain_recent_forgets_replenished_keys() {
    let fx = fixture("retain", &[("RATE_READ", "1000/s")]).await;
    for ip in ["192.0.2.50", "192.0.2.51", "2001:db8::50"] {
        assert_eq!(fx.read(ip).await.0, StatusCode::OK);
    }
    assert_eq!(fx.state.limits.tracked_keys(), 3);
    let task = crate::app::tasks::limits::spawn(fx.state.limits.clone(), Duration::from_millis(5));
    tokio::time::sleep(Duration::from_millis(100)).await;
    task.abort();
    assert_eq!(fx.state.limits.tracked_keys(), 0);
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
async fn limited_and_refused_requests_log_no_ip() {
    let fx = fixture(
        "no_ip_in_logs",
        &[("RATE_READ", "1/h"), ("RATE_CREATE", "1/h")],
    )
    .await;
    let token = fx.token("create").await;
    // `RATE_READ` vaut 1/h : les autres jetons sont émis sans passer par la route.
    let issue = |class| {
        fx.state
            .form_tokens
            .issue(class, fx.state.clock.now(), [9; 16])
    };
    let cand = issue(FormClass::Candidature);
    fx.advance(5);
    // Les tests tournent en parallèle et `tracing` met en cache l'intérêt de chaque callsite : un
    // premier enregistrement fait sur un autre fil, sans collecteur, peut le figer à « jamais ». Les
    // callsites sont donc touchés d'abord, puis le cache est reconstruit une fois le collecteur posé.
    fx.read("192.0.2.250").await;
    assert_code(&fx.read("192.0.2.250").await, Code::LimitRateLimited);
    assert_code(
        &fx.call(Method::GET, "/api/test/read", &[], None, None)
            .await,
        Code::LimitClientIpMissing,
    );
    assert_code(
        &fx.post("/api/test/candidature", "192.0.2.250", json!({}))
            .await,
        Code::FormTokenInvalid,
    );
    tokio::time::sleep(Duration::from_millis(50)).await;
    let captured = Captured::default();
    let writer = captured.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_ansi(false)
        .with_writer(move || writer.clone())
        .finish();
    // `tokio::test` tourne sur un seul fil : le collecteur par défaut couvre les requêtes.
    let guard = tracing::subscriber::set_default(subscriber);
    tracing::callsite::rebuild_interest_cache();
    for ip in ["192.0.2.177", "2001:db8:77::1"] {
        assert_eq!(fx.read(ip).await.0, StatusCode::OK);
        assert_code(&fx.read(ip).await, Code::LimitRateLimited);
    }
    assert_eq!(
        fx.post(
            "/api/test/create",
            "192.0.2.177",
            json!({ "formToken": token })
        )
        .await
        .0,
        StatusCode::CREATED
    );
    assert_code(
        &fx.post(
            "/api/test/create",
            "192.0.2.177",
            json!({ "formToken": token }),
        )
        .await,
        Code::LimitRateLimited,
    );
    assert_code(
        &fx.call(
            Method::GET,
            "/api/test/read",
            &[("x-client-ip", "192.0.2.177, 192.0.2.178")],
            None,
            None,
        )
        .await,
        Code::LimitClientIpMissing,
    );
    // Refus de formulaire : honeypot, sans jeton, jeton d'une autre classe, trop jeune.
    for (body, code) in [
        (
            json!({ "formToken": cand, "website": "x" }),
            Code::FormRejected,
        ),
        (json!({}), Code::FormTokenInvalid),
        (json!({ "formToken": token }), Code::FormTokenInvalid),
    ] {
        assert_code(
            &fx.post("/api/test/candidature", "192.0.2.177", body).await,
            code,
        );
    }
    let fresh = issue(FormClass::Candidature);
    assert_code(
        &fx.post(
            "/api/test/candidature",
            "192.0.2.177",
            json!({ "formToken": fresh }),
        )
        .await,
        Code::FormTooFast,
    );
    drop(guard);
    let log = String::from_utf8(captured.0.lock().unwrap().clone()).unwrap();
    assert!(log.contains("rate limited"), "{log}");
    assert!(log.contains("client ip header missing"), "{log}");
    for code in ["form.rejected", "form.token_invalid", "form.too_fast"] {
        assert!(
            log.contains(&format!(
                "form refused class=\"candidature\" code=\"{code}\""
            )),
            "{code} in {log}"
        );
    }
    for leak in [
        token.as_str(),
        cand.as_str(),
        fresh.as_str(),
        "192.0.2",
        "2001:db8",
        "2001:0db8",
        "[192, 0, 2",
        "[32, 1, 13, 184",
    ] {
        assert!(!log.contains(leak), "{leak} in {log}");
    }
}
