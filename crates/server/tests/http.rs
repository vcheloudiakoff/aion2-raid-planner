//! Matrice HTTP de la story 1-1 : santé, SPA, assets, API inconnue, en-têtes.

mod common;

use axum::http::{Method, StatusCode};
use common::*;
use server::http::headers::{APP_BUILD, CSP, PERMISSIONS_POLICY};
use sqlx::migrate::Migrator;

fn assert_security_headers(res: &axum::http::Response<axum::body::Body>) {
    assert_eq!(header(res, "content-security-policy"), CSP);
    assert_eq!(
        CSP,
        "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'; \
         frame-ancestors 'none'; base-uri 'none'; form-action 'self'"
    );
    assert_eq!(header(res, "referrer-policy"), "no-referrer");
    assert_eq!(header(res, "x-content-type-options"), "nosniff");
    assert_eq!(header(res, "permissions-policy"), PERMISSIONS_POLICY);
    assert!(res.headers().get("access-control-allow-origin").is_none());
}

#[tokio::test]
async fn health_ok() {
    let router = fixture_router(memory_state("health_ok").await);
    let (res, body) = send(&router, Method::GET, "/api/health").await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(header(&res, "cache-control"), "no-store");
    assert_eq!(header(&res, "x-app-build"), APP_BUILD);
    assert!(!APP_BUILD.is_empty());
    assert_security_headers(&res);
    assert!(body.is_empty());
}

#[tokio::test]
async fn health_ko_when_pool_closed() {
    let state = memory_state("health_ko_closed").await;
    state.db.read.close().await;
    let router = fixture_router(state);
    let (res, body) = send(&router, Method::GET, "/api/health").await;
    assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_problem(&res, &body, "server.unavailable", "pool closed");
    assert_eq!(header(&res, "x-app-build"), APP_BUILD);
}

static FIXTURE_MIGRATOR: Migrator = sqlx::migrate!("tests/fixtures/migrations");

#[tokio::test]
async fn health_ko_when_embedded_migration_not_applied() {
    let mut state = memory_state("health_ko_migration").await;
    state.migrator = &FIXTURE_MIGRATOR;
    let router = fixture_router(state);
    let (res, body) = send(&router, Method::GET, "/api/health").await;
    assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_problem(&res, &body, "server.unavailable", "migration");
}

#[tokio::test]
async fn health_ko_when_applied_migration_mismatches() {
    for (case, corruption) in [
        ("checksum", "UPDATE _sqlx_migrations SET checksum = x'00'"),
        ("failed", "UPDATE _sqlx_migrations SET success = 0"),
    ] {
        let mut state = memory_state(&format!("health_ko_mismatch_{case}")).await;
        FIXTURE_MIGRATOR.run(&state.db.write).await.unwrap();
        sqlx::query(corruption)
            .execute(&state.db.write)
            .await
            .unwrap();
        state.migrator = &FIXTURE_MIGRATOR;
        let router = fixture_router(state);
        let (res, body) = send(&router, Method::GET, "/api/health").await;
        assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE, "{case}");
        assert_problem(&res, &body, "server.unavailable", case);
    }
}

#[tokio::test]
async fn health_ko_when_migrations_table_missing() {
    // Base ouverte mais jamais migrée.
    let db = server::infra::sqlite::Db::open(server::infra::sqlite::shared_memory_options(
        "health_ko_unmigrated",
    ))
    .await
    .unwrap();
    let router = fixture_router(state(db));
    let (res, _) = send(&router, Method::GET, "/api/health").await;
    assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn spa_routes_serve_index() {
    let router = fixture_router(memory_state("spa_routes").await);
    for uri in ["/", "/e/abc", "/o/Some-Secret_Value", "/deep/nested/path/"] {
        let (res, body) = send(&router, Method::GET, uri).await;
        assert_eq!(res.status(), StatusCode::OK, "{uri}");
        assert_eq!(body, FIXTURE_INDEX, "{uri}");
        assert_eq!(header(&res, "cache-control"), "no-cache", "{uri}");
        assert!(
            header(&res, "content-type").starts_with("text/html"),
            "{uri}"
        );
        assert!(res.headers().get("x-app-build").is_none(), "{uri}");
        assert_security_headers(&res);
    }
}

#[tokio::test]
async fn head_on_spa_route_has_no_body() {
    let router = fixture_router(memory_state("spa_head").await);
    let (res, body) = send(&router, Method::HEAD, "/e/abc").await;
    assert_eq!(res.status(), StatusCode::OK);
    assert!(body.is_empty());
    assert_eq!(
        header(&res, "content-length"),
        FIXTURE_INDEX.len().to_string(),
        "HEAD announces the GET length"
    );
}

#[tokio::test]
async fn non_get_on_spa_route_is_rejected() {
    let router = fixture_router(memory_state("spa_post").await);
    let (res, _) = send(&router, Method::POST, "/e/abc").await;
    assert_eq!(res.status(), StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn hashed_asset_is_immutable() {
    let router = fixture_router(memory_state("asset_present").await);
    let (res, body) = send(&router, Method::GET, "/assets/app-abc123.js").await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        header(&res, "cache-control"),
        "public, max-age=31536000, immutable"
    );
    assert!(header(&res, "content-type").contains("javascript"));
    assert!(body.contains("fixture"));
    assert_security_headers(&res);
}

#[tokio::test]
async fn unhashed_root_file_is_revalidated() {
    let router = fixture_router(memory_state("asset_root").await);
    let (res, _) = send(&router, Method::GET, "/robots.txt").await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(header(&res, "cache-control"), "no-cache");
}

#[tokio::test]
async fn missing_asset_is_404_not_index() {
    let router = fixture_router(memory_state("asset_absent").await);
    for uri in [
        "/assets/x-HASH.js",
        "/favicon.ico",
        "/.gitkeep",
        "/assets/.hidden.js",
        "/e/abc.def",
    ] {
        let (res, body) = send(&router, Method::GET, uri).await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND, "{uri}");
        assert_ne!(body, FIXTURE_INDEX, "{uri}");
    }
}

#[tokio::test]
async fn unknown_api_is_404_never_index() {
    let router = fixture_router(memory_state("api_unknown").await);
    for uri in [
        "/api/nope",
        "/api",
        "/api/",
        "/api/health/extra",
        "/api/nope/deeper",
    ] {
        let (res, body) = send(&router, Method::GET, uri).await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND, "{uri}");
        assert_problem(&res, &body, "server.not_found", uri);
        assert_eq!(header(&res, "cache-control"), "no-store", "{uri}");
        assert_eq!(header(&res, "x-app-build"), APP_BUILD, "{uri}");
    }
    // Barre finale : atteint le repli SPA, qui doit répondre 404 et non 405.
    let (res, body) = send(&router, Method::POST, "/api/").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_problem(&res, &body, "server.not_found", "POST /api/");
}

#[tokio::test]
async fn wrong_method_on_api_route_is_405_problem() {
    let router = fixture_router(memory_state("api_405").await);
    for (method, uri) in [
        (Method::POST, "/api/health"),
        (Method::DELETE, "/api/reference"),
    ] {
        let (res, body) = send(&router, method.clone(), uri).await;
        assert_eq!(
            res.status(),
            StatusCode::METHOD_NOT_ALLOWED,
            "{method} {uri}"
        );
        assert_problem(&res, &body, "server.method_not_allowed", uri);
        assert!(header(&res, "allow").contains("GET"), "{uri}: Allow");
        assert_eq!(header(&res, "x-app-build"), APP_BUILD, "{uri}");
        assert_eq!(header(&res, "cache-control"), "no-store", "{uri}");
    }
}

#[tokio::test]
async fn reference_absent_is_503_problem_with_retry_after() {
    let db = memory_db("ref_absent").await;
    let router = fixture_router(state_with(
        db,
        server::infra::reference::ReferenceStore::new(),
    ));
    let (res, body) = send(&router, Method::GET, "/api/reference").await;
    assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_problem(&res, &body, "reference.unavailable", "reference");
    assert_eq!(
        header(&res, "retry-after"),
        server::http::errors::DEFAULT_RETRY_AFTER_SECS.to_string()
    );
    assert_eq!(header(&res, "x-app-build"), APP_BUILD);
}

/// Avec `npm run build` fait avant `cargo build`, la vraie page embarquée est servie avec la CSP
/// exacte. Sans build web, seul le contrat « pas d'index = 404 » est vérifié.
#[tokio::test]
async fn embedded_web_dist() {
    let router = server::http::router(memory_state("embedded_dist").await);
    let (res, body) = send(&router, Method::GET, "/").await;
    assert_security_headers(&res);
    // En CI, après `vite build`, le repli 404 masquerait un `outDir` cassé : il est interdit.
    let required = std::env::var("REQUIRE_WEB_DIST").is_ok_and(|v| v == "1");
    let built = server::http::assets::WebDist::get("index.html").is_some();
    assert!(
        built || !required,
        "REQUIRE_WEB_DIST=1 but web/dist/index.html is not embedded"
    );
    if built {
        assert_eq!(res.status(), StatusCode::OK);
        assert!(body.contains("<div id=\"root\">"));
        assert_eq!(header(&res, "cache-control"), "no-cache");
    } else {
        eprintln!("web/dist not built: checking 404 contract only");
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }
}
