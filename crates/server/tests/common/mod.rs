#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, Response};
use rust_embed::RustEmbed;
use server::app::AppState;
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
    let clock = FixedClock::new("2026-11-06T10:00:00Z".parse().unwrap());
    AppState::new(
        db,
        Arc::new(clock),
        Arc::new(DeterministicIdGenerator::new()),
        reference,
    )
}

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

/// Comme `send`, avec des en-têtes de requête.
pub async fn send_with(
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
