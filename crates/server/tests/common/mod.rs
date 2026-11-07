#![allow(dead_code)]

pub mod candidatures;

use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, Response};
use rust_embed::RustEmbed;
use server::app::AppState;
use server::config::Config;
use server::infra::clock::FixedClock;
use server::infra::ids::DeterministicIdGenerator;
use server::infra::reference::{self, ReferenceStore};
use server::infra::sqlite::{self, Db};
use tower::ServiceExt;

/// SPA de démonstration, indépendante du build Vite.
#[derive(RustEmbed)]
#[folder = "tests/fixtures/dist/"]
pub struct FixtureDist;

pub const FIXTURE_INDEX: &str = include_str!("../fixtures/dist/index.html");

/// État de test, référence embarquée chargée.
pub fn state(db: Db) -> AppState {
    state_with(
        db,
        ReferenceStore::with(reference::embedded().expect("embedded reference is valid")),
    )
}

pub fn state_with(db: Db, reference: ReferenceStore) -> AppState {
    state_with_config(db, reference, &test_config(&[]))
}

/// Comme `state_with`, avec une configuration donnée.
pub fn state_with_config(db: Db, reference: ReferenceStore, config: &Config) -> AppState {
    let clock = FixedClock::new("2026-11-06T10:00:00Z".parse().unwrap());
    AppState::new(
        db,
        Arc::new(clock),
        Arc::new(DeterministicIdGenerator::new()),
        reference,
        config,
    )
}

/// Configuration de test : production par défaut (`TRUST_CLIENT_IP_HEADER=true`), `overrides`
/// par-dessus.
pub fn test_config(overrides: &[(&str, &str)]) -> Config {
    Config::from_source(|k| {
        overrides
            .iter()
            .rev()
            .find(|(name, _)| *name == k)
            .map(|(_, v)| (*v).to_owned())
            .or_else(|| match k {
                "PUBLIC_BASE_URL" => Some("https://raid.example.org".to_owned()),
                "FORM_TOKEN_SECRET" => Some("0123456789abcdef0123456789abcdef".to_owned()),
                _ => None,
            })
    })
    .expect("valid test configuration")
}

/// IP visiteur posée par défaut sur les requêtes de test (plage de documentation 192.0.2.0/24).
pub const TEST_CLIENT_IP: &str = "192.0.2.1";

/// Base mémoire partagée propre au test, migrations appliquées.
pub async fn memory_db(name: &str) -> Db {
    let db = Db::open(sqlite::shared_memory_options(name)).await.unwrap();
    db.migrate().await.unwrap();
    db
}

pub async fn memory_state(name: &str) -> AppState {
    state(memory_db(name).await)
}

pub fn fixture_router(state: AppState) -> Router {
    server::http::router_with::<FixtureDist>(state)
}

pub async fn send(router: &Router, method: Method, uri: &str) -> (Response<Body>, String) {
    send_with(router, method, uri, &[]).await
}

/// Comme `send`, avec des en-têtes de requête. `X-Client-IP` vaut [`TEST_CLIENT_IP`] sauf si
/// `headers` le fournit.
pub async fn send_with(
    router: &Router,
    method: Method,
    uri: &str,
    headers: &[(&str, &str)],
) -> (Response<Body>, String) {
    let mut all = headers.to_vec();
    if !headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("x-client-ip"))
    {
        all.push(("x-client-ip", TEST_CLIENT_IP));
    }
    send_bare(router, method, uri, &all).await
}

/// Envoie exactement `headers`, sans IP visiteur par défaut.
pub async fn send_bare(
    router: &Router,
    method: Method,
    uri: &str,
    headers: &[(&str, &str)],
) -> (Response<Body>, String) {
    let mut req = Request::builder().method(method).uri(uri);
    for (name, value) in headers {
        req = req.header(*name, *value);
    }
    let req = req.body(Body::empty()).unwrap();
    let res = router.clone().oneshot(req).await.unwrap();
    let (parts, body) = res.into_parts();
    let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
    (
        Response::from_parts(parts, Body::empty()),
        String::from_utf8_lossy(&bytes).into_owned(),
    )
}

/// POST JSON avec l'IP visiteur de test ; `body` est envoyé tel quel (texte brut, JSON ou non).
pub async fn post_raw(router: &Router, uri: &str, body: &str) -> (Response<Body>, String) {
    post_raw_with(router, uri, body, &[]).await
}

/// Comme `post_raw`, avec des en-têtes de requête en plus (`X-Link-Secret`…).
pub async fn post_raw_with(
    router: &Router,
    uri: &str,
    body: &str,
    headers: &[(&str, &str)],
) -> (Response<Body>, String) {
    let mut req = Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header("content-type", "application/json")
        .header("x-client-ip", TEST_CLIENT_IP);
    for (name, value) in headers {
        req = req.header(*name, *value);
    }
    let req = req.body(Body::from(body.to_owned())).unwrap();
    let res = router.clone().oneshot(req).await.unwrap();
    let (parts, body) = res.into_parts();
    let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
    (
        Response::from_parts(parts, Body::empty()),
        String::from_utf8_lossy(&bytes).into_owned(),
    )
}

/// POST d'une valeur JSON.
pub async fn post_json(
    router: &Router,
    uri: &str,
    body: &serde_json::Value,
) -> (Response<Body>, String) {
    post_raw(router, uri, &body.to_string()).await
}

/// POST d'une valeur JSON avec des en-têtes en plus.
pub async fn post_json_with(
    router: &Router,
    uri: &str,
    body: &serde_json::Value,
    headers: &[(&str, &str)],
) -> (Response<Body>, String) {
    post_raw_with(router, uri, &body.to_string(), headers).await
}

pub fn header<'a>(res: &'a Response<Body>, name: &str) -> &'a str {
    res.headers()
        .get(name)
        .map(|v| v.to_str().unwrap())
        .unwrap_or("")
}

/// Dossier temporaire unique, jamais sous `/data`.
pub struct TempDir(pub PathBuf);

impl TempDir {
    pub fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("aion2-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Corps `application/problem+json` exact : `{type, title, status, code}` et rien d'autre ; 503 et
/// 429 portent `Retry-After`.
pub fn assert_problem(res: &Response<Body>, body: &str, code: &str, ctx: &str) {
    let status = res.status();
    assert_eq!(
        header(res, "content-type"),
        "application/problem+json",
        "{ctx}"
    );
    let v: serde_json::Value = serde_json::from_str(body).unwrap_or_else(|e| panic!("{ctx}: {e}"));
    let obj = v.as_object().unwrap();
    let mut keys: Vec<_> = obj.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, ["code", "status", "title", "type"], "{ctx}");
    assert_eq!(v["type"], "about:blank", "{ctx}");
    assert_eq!(v["title"], status.canonical_reason().unwrap(), "{ctx}");
    assert_eq!(v["status"], status.as_u16(), "{ctx}");
    assert_eq!(v["code"], code, "{ctx}");
    let retry = header(res, "retry-after");
    if status.as_u16() == 503 || status.as_u16() == 429 {
        assert!(
            retry.parse::<u64>().is_ok_and(|s| s > 0),
            "{ctx}: Retry-After"
        );
    } else {
        assert!(retry.is_empty(), "{ctx}: no Retry-After");
    }
}
