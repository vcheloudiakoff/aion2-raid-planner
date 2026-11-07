//! Matrice HTTP de la story 2-2, côté lecture : `GET /api/events/{id}`, `GET /api/join`, comptes
//! publics, aucune donnée nominative, `noindex` conditionnel de la page `/e/{id}`.

mod common;

use axum::http::{Method, StatusCode};
use common::*;
use domain::Timestamp;
use serde_json::{Value, json};
use server::app::AppState;
use server::http::form_token::FormClass;
use server::infra::reference::{self, ReferenceStore};

const NOW: &str = "2026-11-06T10:00:00Z";

/// Pseudos de test, fictifs.
const PSEUDOS: [&str; 3] = ["Nyraelle", "Kaelthorn", "Tomasquin"];

fn token(state: &AppState, class: FormClass) -> String {
    let issued =
        Timestamp::from_unix_seconds(NOW.parse::<Timestamp>().unwrap().unix_seconds() - 10)
            .unwrap();
    state.form_tokens.issue(class, issued, [3; 16])
}

struct Created {
    event_id: String,
    organizer: String,
    join: String,
}

/// Crée un événement Ludra par l'API, avec `patch` sur le corps nominal.
async fn create(state: &AppState, router: &axum::Router, patch: Value) -> Created {
    let mut body = json!({
        "formToken": token(state, FormClass::Create),
        "instance": "ludra",
        "region": "europe",
        "factionScope": "all",
        "note": "Vocal prêt",
        "discordUrl": "https://discord.gg/abcdef",
        "minCp": 1200,
        "slots": [
            { "startsAt": "2026-11-14T20:00:00Z" },
            { "startsAt": "2026-11-11T20:00:00Z", "durationMin": 90 },
        ],
    });
    for (k, v) in patch.as_object().unwrap() {
        body[k] = v.clone();
    }
    let (res, text) = post_json(router, "/api/events", &body).await;
    assert_eq!(res.status(), StatusCode::CREATED, "{text}");
    let v: Value = serde_json::from_str(&text).unwrap();
    Created {
        event_id: v["eventId"].as_str().unwrap().into(),
        organizer: v["organizerSecret"].as_str().unwrap().into(),
        join: v["joinSecret"].as_str().unwrap().into(),
    }
}

/// Candidature écrite directement en base, pour fixer un statut quelconque.
async fn insert_candidature(
    state: &AppState,
    event_id: &str,
    n: u8,
    pseudo: &str,
    class: &str,
    status: &str,
) {
    sqlx::query(
        "INSERT INTO candidatures (id, event_id, pseudo, server, class, cp, status, origin, \
         created_at, updated_at) VALUES (?, ?, ?, 'siel', ?, 1000, ?, 'public', ?, ?)",
    )
    .bind(format!("{}{n}", "c".repeat(21)))
    .bind(event_id)
    .bind(pseudo)
    .bind(class)
    .bind(status)
    .bind(NOW)
    .bind(NOW)
    .execute(&state.db.write)
    .await
    .unwrap();
}

async fn setup(name: &str) -> (AppState, axum::Router) {
    let state = state_with_config(
        memory_db(&format!("public_{name}")).await,
        ReferenceStore::with(reference::embedded().unwrap()),
        &test_config(&[("RATE_CREATE", "100/h")]),
    );
    let router = fixture_router(state.clone());
    (state, router)
}

async fn get(
    router: &axum::Router,
    uri: &str,
    headers: &[(&str, &str)],
) -> (StatusCode, Value, String) {
    let (res, body) = send_with(router, Method::GET, uri, headers).await;
    let status = res.status();
    assert_eq!(header(&res, "cache-control"), "no-store");
    let json = serde_json::from_str(&body).unwrap_or(Value::Null);
    (status, json, body)
}

/// Clés de tout le document, à toutes les profondeurs.
fn all_keys(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Object(m) => {
            for (k, x) in m {
                out.push(k.clone());
                all_keys(x, out);
            }
        }
        Value::Array(a) => a.iter().for_each(|x| all_keys(x, out)),
        _ => {}
    }
}

fn assert_no_nominative_data(body: &str, ctx: &str) {
    let v: Value = serde_json::from_str(body).unwrap();
    let mut keys = Vec::new();
    all_keys(&v, &mut keys);
    for forbidden in [
        "pseudo",
        "server",
        "candidatures",
        "candidatureId",
        "discordUrl",
        "adminUnlistedAt",
        "createdAt",
    ] {
        assert!(
            !keys.iter().any(|k| k == forbidden),
            "{ctx}: key {forbidden} in {body}"
        );
    }
    for p in PSEUDOS {
        assert!(!body.contains(p), "{ctx}: pseudo {p} in {body}");
    }
    assert!(!body.contains("discord.gg"), "{ctx}: {body}");
    assert!(!body.contains("siel"), "{ctx}: server in {body}");
}

#[tokio::test]
async fn public_read_counts_pending_and_accepted_only_and_names_nobody() {
    let (state, router) = setup("nominal").await;
    let c = create(
        &state,
        &router,
        json!({ "classBounds": { "cleric": { "min": 2, "max": 3 } } }),
    )
    .await;
    insert_candidature(&state, &c.event_id, 1, PSEUDOS[0], "cleric", "pending").await;
    insert_candidature(&state, &c.event_id, 2, PSEUDOS[1], "cleric", "rejected").await;
    insert_candidature(&state, &c.event_id, 3, PSEUDOS[2], "templar", "withdrawn").await;

    let (status, v, body) = get(&router, &format!("/api/events/{}", c.event_id), &[]).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_no_nominative_data(&body, "public");

    let mut keys: Vec<_> = v.as_object().unwrap().keys().cloned().collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "countsByClass",
            "countsByRole",
            "factionScope",
            "id",
            "instance",
            "minCp",
            "missingRoles",
            "note",
            "region",
            "slots",
            "status",
            "visibility"
        ]
    );
    assert_eq!(v["id"], c.event_id);
    assert_eq!(v["instance"], "ludra");
    assert_eq!(v["region"], "europe");
    assert_eq!(v["factionScope"], "all");
    assert_eq!(v["visibility"], "listed");
    assert_eq!(v["status"], "open");
    assert_eq!(v["minCp"], 1200);
    assert_eq!(v["note"], "Vocal prêt");
    // Triés par `startsAt`.
    let starts: Vec<_> = v["slots"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["startsAt"].clone())
        .collect();
    assert_eq!(
        starts,
        [json!("2026-11-11T20:00:00Z"), json!("2026-11-14T20:00:00Z")]
    );
    assert_eq!(v["slots"][0]["durationMin"], 90);
    assert_eq!(v["slots"][1]["durationMin"], 60);
    assert_eq!(v["slots"][0]["id"].as_str().unwrap().len(), 22);

    // Une entrée par classe bornée, zéros compris.
    let set = state.reference.get().unwrap();
    let by_class = v["countsByClass"].as_object().unwrap();
    assert_eq!(
        by_class.len(),
        set.classes.iter().filter(|c| c.enabled).count()
    );
    assert_eq!(v["countsByClass"]["cleric"], 1);
    assert_eq!(v["countsByClass"]["templar"], 0);
    assert_eq!(v["countsByRole"]["healer"], 1);
    assert_eq!(v["countsByRole"]["tank"], 0);
    assert_eq!(
        v["missingRoles"],
        json!([{ "role": "healer", "missing": 1 }])
    );
}

#[tokio::test]
async fn unknown_event_is_404_and_cancelled_stays_readable() {
    let (state, router) = setup("unknown").await;
    let (res, body) = send(
        &router,
        Method::GET,
        &format!("/api/events/{}", "x".repeat(22)),
    )
    .await;
    assert_problem(&res, &body, "event.not_found", "unknown");
    let (res, body) = send(&router, Method::GET, "/api/events/short").await;
    assert_problem(&res, &body, "event.not_found", "short");

    let c = create(&state, &router, json!({})).await;
    sqlx::query("UPDATE events SET status = 'cancelled', cancelled_at = ? WHERE id = ?")
        .bind(NOW)
        .bind(&c.event_id)
        .execute(&state.db.write)
        .await
        .unwrap();
    let (status, v, _) = get(&router, &format!("/api/events/{}", c.event_id), &[]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["status"], "cancelled");
}

#[tokio::test]
async fn join_read_gives_the_same_public_event() {
    let (state, router) = setup("join").await;
    let c = create(&state, &router, json!({})).await;
    insert_candidature(&state, &c.event_id, 1, PSEUDOS[0], "cleric", "accepted").await;
    let (_, public, _) = get(&router, &format!("/api/events/{}", c.event_id), &[]).await;
    let (status, joined, body) = get(&router, "/api/join", &[("x-link-secret", &c.join)]).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(joined, public);
    assert_no_nominative_data(&body, "join");

    let (res, body) = send_with(
        &router,
        Method::GET,
        "/api/join",
        &[("x-link-secret", &c.organizer)],
    )
    .await;
    assert_problem(&res, &body, "auth.wrong_right", "organizer");
    let unknown = "A".repeat(43);
    let (res, body) = send_with(
        &router,
        Method::GET,
        "/api/join",
        &[("x-link-secret", &unknown)],
    )
    .await;
    assert_problem(&res, &body, "auth.not_found", "unknown");
    let (res, body) = send(&router, Method::GET, "/api/join").await;
    assert_problem(&res, &body, "auth.missing", "absent");
    // Jamais de secret dans l'URL : la query string est ignorée.
    let (res, body) = send(
        &router,
        Method::GET,
        &format!("/api/join?secret={}", c.join),
    )
    .await;
    assert_problem(&res, &body, "auth.missing", "query");
}

#[tokio::test]
async fn read_routes_use_the_read_quota() {
    let state = state_with_config(
        memory_db("public_quota").await,
        ReferenceStore::with(reference::embedded().unwrap()),
        &test_config(&[("RATE_READ", "2/h")]),
    );
    let router = fixture_router(state.clone());
    let uri = format!("/api/events/{}", "x".repeat(22));
    for _ in 0..2 {
        let (res, _) = send(&router, Method::GET, &uri).await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }
    let (res, body) = send(&router, Method::GET, "/api/join").await;
    assert_problem(&res, &body, "limit.rate_limited", "quota");
    let (res, body) = send_bare(&router, Method::GET, &uri, &[]).await;
    assert_problem(&res, &body, "limit.client_ip_missing", "no ip");
}

#[tokio::test]
async fn event_page_is_noindex_unless_listed() {
    let (state, router) = setup("noindex").await;
    let listed = create(&state, &router, json!({})).await;
    let unlisted = create(&state, &router, json!({ "visibility": "unlisted" })).await;
    let hidden = create(&state, &router, json!({})).await;
    sqlx::query("UPDATE events SET admin_unlisted_at = ? WHERE id = ?")
        .bind(NOW)
        .bind(&hidden.event_id)
        .execute(&state.db.write)
        .await
        .unwrap();

    let (res, body) = send(&router, Method::GET, &format!("/e/{}", listed.event_id)).await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(body, FIXTURE_INDEX);
    assert!(res.headers().get("x-robots-tag").is_none());
    assert!(!body.contains("og:"));

    for id in [
        unlisted.event_id.clone(),
        hidden.event_id.clone(),
        "x".repeat(22),
        "abc".into(),
    ] {
        let (res, body) = send(&router, Method::GET, &format!("/e/{id}")).await;
        assert_eq!(res.status(), StatusCode::OK, "{id}");
        assert_eq!(body, FIXTURE_INDEX, "{id}");
        assert_eq!(header(&res, "x-robots-tag"), "noindex", "{id}");
        assert_eq!(header(&res, "cache-control"), "no-cache", "{id}");
    }
    // HEAD garde la même règle, et un asset sous `/e/` reste un 404.
    let (res, body) = send(&router, Method::HEAD, &format!("/e/{}", unlisted.event_id)).await;
    assert_eq!(res.status(), StatusCode::OK);
    assert!(body.is_empty());
    assert_eq!(header(&res, "x-robots-tag"), "noindex");
    let (res, _) = send(&router, Method::GET, "/e/abc.def").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    // Hors de la route exacte, tout chemin sous `/e/` est noindex, même pour un événement listé.
    for uri in [
        format!("/e/{}/", listed.event_id),
        format!("/e/{}/x", unlisted.event_id),
        "/e".into(),
        "/e/".into(),
    ] {
        let (res, body) = send(&router, Method::GET, &uri).await;
        assert_eq!(res.status(), StatusCode::OK, "{uri}");
        assert_eq!(body, FIXTURE_INDEX, "{uri}");
        assert_eq!(header(&res, "x-robots-tag"), "noindex", "{uri}");
    }
    // Id de 22 caractères hors base64url : noindex, sans requête.
    let (res, _) = send(&router, Method::GET, &format!("/e/{}", "%25".repeat(22))).await;
    assert_eq!(header(&res, "x-robots-tag"), "noindex");
}
