//! Matrice HTTP de la story 2-2, côté écriture : `POST /api/events/{id}/candidatures` (`pending`,
//! `public`) et `POST /api/join/candidatures` (`accepted`, `direct`), lignes écrites, lien participant
//! haché, refus.

mod common;

use std::sync::{Arc, Mutex};

use axum::http::{Method, StatusCode};
use common::*;
use domain::Timestamp;
use domain::reference::{Faction, Label, Server};
use serde_json::{Value, json};
use server::app::AppState;
use server::config::Config;
use server::http::form_token::FormClass;
use server::infra::links::{self, Right};
use server::infra::reference::{self, ReferenceStore};

const NOW: &str = "2026-11-06T10:00:00Z";
const JOIN_CANDIDATURES: &str = "/api/join/candidatures";

fn ago(secs: i64) -> Timestamp {
    Timestamp::from_unix_seconds(NOW.parse::<Timestamp>().unwrap().unix_seconds() - secs).unwrap()
}

fn token(state: &AppState, class: FormClass) -> String {
    state.form_tokens.issue(class, ago(10), [3; 16])
}

fn public_uri(event_id: &str) -> String {
    format!("/api/events/{event_id}/candidatures")
}

/// Référence embarquée, plus un serveur d'une autre région (valeurs fictives).
fn reference_set() -> domain::reference::ReferenceSet {
    let mut set = reference::embedded().unwrap();
    set.servers.push(Server {
        id: "far_away".into(),
        region: "na_west".into(),
        faction: Faction::Elyos,
        tag: "Far".into(),
        label: Label {
            en: "Far".into(),
            fr: None,
        },
        enabled: true,
    });
    set
}

async fn setup_with(name: &str, config: &Config) -> (AppState, axum::Router) {
    let state = state_with_config(
        memory_db(&format!("cand_{name}")).await,
        ReferenceStore::with(reference_set()),
        config,
    );
    let router = fixture_router(state.clone());
    (state, router)
}

async fn setup(name: &str) -> (AppState, axum::Router) {
    setup_with(
        name,
        &test_config(&[("RATE_CREATE", "100/h"), ("RATE_CANDIDATURE", "1000/h")]),
    )
    .await
}

struct Event {
    id: String,
    organizer: String,
    join: String,
    /// Ids des créneaux, triés par début.
    slots: Vec<String>,
}

async fn create(state: &AppState, router: &axum::Router, scope: &str) -> Event {
    let body = json!({
        "formToken": token(state, FormClass::Create),
        "instance": "ludra",
        "region": "europe",
        "factionScope": scope,
        "slots": [
            { "startsAt": "2026-11-11T20:00:00Z" },
            { "startsAt": "2026-11-12T20:00:00Z" },
        ],
    });
    let (res, text) = post_json(router, "/api/events", &body).await;
    assert_eq!(res.status(), StatusCode::CREATED, "{text}");
    let v: Value = serde_json::from_str(&text).unwrap();
    let id = v["eventId"].as_str().unwrap().to_owned();
    let slots: Vec<(String,)> =
        sqlx::query_as("SELECT id FROM slots WHERE event_id = ? ORDER BY starts_at")
            .bind(&id)
            .fetch_all(&state.db.read)
            .await
            .unwrap();
    Event {
        id,
        organizer: v["organizerSecret"].as_str().unwrap().into(),
        join: v["joinSecret"].as_str().unwrap().into(),
        slots: slots.into_iter().map(|(s,)| s).collect(),
    }
}

/// Corps nominal : Asmodienne d'Europe, Clerc, les deux créneaux, tous deux préférés.
fn nominal(state: &AppState, e: &Event) -> Value {
    json!({
        "formToken": token(state, FormClass::Candidature),
        "website": "",
        "pseudo": "Nyraelle",
        "server": "israphel",
        "class": "cleric",
        "cp": 1500,
        "availabilities": [
            { "slotId": e.slots[0], "preferred": true },
            { "slotId": e.slots[1], "preferred": true },
        ],
    })
}

fn with(mut body: Value, patch: Value) -> Value {
    for (k, v) in patch.as_object().unwrap() {
        body[k] = v.clone();
    }
    body
}

async fn count(state: &AppState, table: &str) -> i64 {
    let (n,): (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {table}"))
        .fetch_one(&state.db.read)
        .await
        .unwrap();
    n
}

/// Rien de la candidature n'est écrit : seuls les deux liens de l'événement existent.
async fn assert_nothing_written(state: &AppState, ctx: &str) {
    assert_eq!(count(state, "candidatures").await, 0, "{ctx}");
    assert_eq!(count(state, "availabilities").await, 0, "{ctx}");
    let (n,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM links WHERE link_right = 'participant'")
            .fetch_one(&state.db.read)
            .await
            .unwrap();
    assert_eq!(n, 0, "{ctx}");
}

fn assert_fields(
    res: &axum::http::Response<axum::body::Body>,
    body: &str,
    code: &str,
    fields: Value,
) {
    assert_eq!(res.status(), StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(header(res, "content-type"), "application/problem+json");
    let v: Value = serde_json::from_str(body).unwrap();
    assert_eq!(v["code"], code, "{body}");
    assert_eq!(v["fields"], fields, "{body}");
}

/// Vérifie la réponse 201 et les lignes écrites ; rend l'id de candidature et le secret.
async fn assert_created(
    state: &AppState,
    e: &Event,
    body: &str,
    status: &str,
    origin: &str,
) -> (String, String) {
    let v: Value = serde_json::from_str(body).unwrap();
    let mut keys: Vec<_> = v.as_object().unwrap().keys().cloned().collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "candidatureId",
            "eventId",
            "links",
            "participantSecret",
            "status"
        ]
    );
    let cid = v["candidatureId"].as_str().unwrap().to_owned();
    let secret = v["participantSecret"].as_str().unwrap().to_owned();
    assert_eq!(v["eventId"], e.id);
    assert_eq!(v["status"], status);
    assert_eq!((cid.len(), secret.len()), (22, 43));
    assert_eq!(v["links"], json!({ "participant": format!("/p/{secret}") }));

    let row: (
        String,
        String,
        String,
        String,
        i64,
        String,
        String,
        String,
        String,
    ) = sqlx::query_as(
        "SELECT event_id, pseudo, server, class, cp, status, origin, created_at, updated_at \
             FROM candidatures WHERE id = ?",
    )
    .bind(&cid)
    .fetch_one(&state.db.read)
    .await
    .unwrap();
    assert_eq!(
        row,
        (
            e.id.clone(),
            "Nyraelle".into(),
            "israphel".into(),
            "cleric".into(),
            1500,
            status.into(),
            origin.into(),
            NOW.into(),
            NOW.into()
        )
    );
    let avail: Vec<(String, i64)> = sqlx::query_as(
        "SELECT slot_id, preferred FROM availabilities WHERE candidature_id = ? ORDER BY slot_id",
    )
    .bind(&cid)
    .fetch_all(&state.db.read)
    .await
    .unwrap();
    let mut expected: Vec<(String, i64)> = e.slots.iter().map(|s| (s.clone(), 1)).collect();
    expected.sort();
    assert_eq!(avail, expected);

    // Le lien participant se résout par son hash ; le secret n'est stocké nulle part en clair.
    let found = links::find(&state.db.read, &links::hash(&secret))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found.right, Right::Participant);
    assert_eq!(found.event_id, e.id);
    assert_eq!(found.candidature_id.as_deref(), Some(cid.as_str()));
    let dump: Vec<(String,)> = sqlx::query_as(
        "SELECT id || event_id || pseudo || server || class || status || origin FROM candidatures \
         UNION ALL SELECT hex(secret_hash) || link_right || event_id || \
         coalesce(candidature_id, '') FROM links",
    )
    .fetch_all(&state.db.read)
    .await
    .unwrap();
    for (text,) in dump {
        assert!(!text.contains(&secret));
    }
    (cid, secret)
}

#[tokio::test]
async fn public_candidature_is_pending() {
    let (state, router) = setup("public").await;
    let e = create(&state, &router, "all").await;
    let (res, body) = post_json(&router, &public_uri(&e.id), &nominal(&state, &e)).await;
    assert_eq!(res.status(), StatusCode::CREATED, "{body}");
    assert_eq!(header(&res, "cache-control"), "no-store");
    assert_created(&state, &e, &body, "pending", "public").await;

    // La page publique compte la candidature, sans la nommer.
    let (_, text) = send(&router, Method::GET, &format!("/api/events/{}", e.id)).await;
    let v: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["countsByClass"]["cleric"], 1);
    assert!(!text.contains("Nyraelle"));
}

#[tokio::test]
async fn direct_candidature_is_accepted() {
    let (state, router) = setup("direct").await;
    let e = create(&state, &router, "all").await;
    let (res, body) = post_json_with(
        &router,
        JOIN_CANDIDATURES,
        &nominal(&state, &e),
        &[("x-link-secret", &e.join)],
    )
    .await;
    assert_eq!(res.status(), StatusCode::CREATED, "{body}");
    assert_created(&state, &e, &body, "accepted", "direct").await;
}

#[tokio::test]
async fn direct_candidature_needs_a_join_secret() {
    let (state, router) = setup("direct_auth").await;
    let e = create(&state, &router, "all").await;
    let unknown = "A".repeat(43);
    for (headers, code) in [
        (vec![], "auth.missing"),
        (vec![("x-link-secret", unknown.as_str())], "auth.not_found"),
        (
            vec![("x-link-secret", e.organizer.as_str())],
            "auth.wrong_right",
        ),
    ] {
        let (res, body) =
            post_json_with(&router, JOIN_CANDIDATURES, &nominal(&state, &e), &headers).await;
        assert_problem(&res, &body, code, code);
    }
    assert_nothing_written(&state, "direct auth").await;
}

#[tokio::test]
async fn server_out_of_region_or_faction_out_of_scope() {
    let (state, router) = setup("server").await;
    let e = create(&state, &router, "all").await;
    let body = with(nominal(&state, &e), json!({ "server": "far_away" }));
    let (res, text) = post_json(&router, &public_uri(&e.id), &body).await;
    assert_fields(
        &res,
        &text,
        "candidature.server_out_of_region",
        json!({ "server": "candidature.server_out_of_region" }),
    );
    let elyos = create(&state, &router, "elyos").await;
    let (res, text) = post_json(&router, &public_uri(&elyos.id), &nominal(&state, &elyos)).await;
    assert_fields(
        &res,
        &text,
        "candidature.faction_out_of_scope",
        json!({ "server": "candidature.faction_out_of_scope" }),
    );
    let body = with(
        nominal(&state, &e),
        json!({ "server": "atlantis", "class": "bard" }),
    );
    let (res, text) = post_json(&router, &public_uri(&e.id), &body).await;
    assert_fields(
        &res,
        &text,
        "reference.unknown",
        json!({ "server": "reference.unknown", "class": "reference.unknown" }),
    );
    assert_nothing_written(&state, "server").await;
}

#[tokio::test]
async fn availabilities_are_checked() {
    let (state, router) = setup("slots").await;
    let e = create(&state, &router, "all").await;
    let other = create(&state, &router, "all").await;
    // Créneau déjà commencé, posé directement en base.
    let past = "p".repeat(22);
    sqlx::query("INSERT INTO slots (id, event_id, starts_at, duration_min) VALUES (?, ?, ?, 60)")
        .bind(&past)
        .bind(&e.id)
        .bind(ago(600))
        .execute(&state.db.write)
        .await
        .unwrap();
    for (availabilities, code, path) in [
        (json!([]), "candidature.no_availability", "availabilities"),
        (
            json!([{ "slotId": other.slots[0] }]),
            "candidature.slot_unknown",
            "availabilities.0.slotId",
        ),
        (
            json!([{ "slotId": e.slots[0] }, { "slotId": past, "preferred": true }]),
            "slot.in_past",
            "availabilities.1.slotId",
        ),
        (
            json!([{ "slotId": e.slots[0] }, { "slotId": e.slots[0] }]),
            "candidature.slot_unknown",
            "availabilities.1.slotId",
        ),
    ] {
        let body = with(
            nominal(&state, &e),
            json!({ "availabilities": availabilities }),
        );
        let (res, text) = post_json(&router, &public_uri(&e.id), &body).await;
        assert_fields(&res, &text, code, json!({ path: code }));
    }
    assert_nothing_written(&state, "slots").await;
}

#[tokio::test]
async fn pseudo_and_cp_bounds() {
    let (state, router) = setup("pseudo_cp").await;
    let e = create(&state, &router, "all").await;
    let body = with(
        nominal(&state, &e),
        json!({ "pseudo": "a".repeat(17), "cp": 1_000_000 }),
    );
    let (res, text) = post_json(&router, &public_uri(&e.id), &body).await;
    assert_fields(
        &res,
        &text,
        "candidature.pseudo_invalid",
        json!({ "pseudo": "candidature.pseudo_invalid", "cp": "candidature.cp_out_of_range" }),
    );
    assert_nothing_written(&state, "pseudo cp").await;
}

#[tokio::test]
async fn a_cp_under_min_cp_is_only_a_signal() {
    let (state, router) = setup("min_cp").await;
    let e = create(&state, &router, "all").await;
    sqlx::query("UPDATE events SET min_cp = 5000 WHERE id = ?")
        .bind(&e.id)
        .execute(&state.db.write)
        .await
        .unwrap();
    let (res, body) = post_json(&router, &public_uri(&e.id), &nominal(&state, &e)).await;
    assert_eq!(res.status(), StatusCode::CREATED, "{body}");
}

#[tokio::test]
async fn cancelled_event_is_closed_but_locked_still_accepts() {
    let (state, router) = setup("closed").await;
    let e = create(&state, &router, "all").await;
    for (status, expected) in [
        ("cancelled", StatusCode::CONFLICT),
        ("locked", StatusCode::CREATED),
    ] {
        sqlx::query("UPDATE events SET status = ? WHERE id = ?")
            .bind(status)
            .bind(&e.id)
            .execute(&state.db.write)
            .await
            .unwrap();
        let (res, body) = post_json(&router, &public_uri(&e.id), &nominal(&state, &e)).await;
        assert_eq!(res.status(), expected, "{status}: {body}");
        if expected == StatusCode::CONFLICT {
            assert_problem(&res, &body, "event.closed", status);
            assert_nothing_written(&state, status).await;
        }
    }
}

#[tokio::test]
async fn cancelled_event_is_closed_before_any_field_rule() {
    let (state, router) = setup("closed_first").await;
    let e = create(&state, &router, "all").await;
    let past = "p".repeat(22);
    sqlx::query("INSERT INTO slots (id, event_id, starts_at, duration_min) VALUES (?, ?, ?, 60)")
        .bind(&past)
        .bind(&e.id)
        .bind(ago(600))
        .execute(&state.db.write)
        .await
        .unwrap();
    sqlx::query("UPDATE events SET status = 'cancelled' WHERE id = ?")
        .bind(&e.id)
        .execute(&state.db.write)
        .await
        .unwrap();
    let body = with(
        nominal(&state, &e),
        json!({ "availabilities": [{ "slotId": past }] }),
    );
    let (res, text) = post_json(&router, &public_uri(&e.id), &body).await;
    assert_problem(&res, &text, "event.closed", "cancelled + past slot");
    assert_nothing_written(&state, "closed first").await;
}

#[tokio::test]
async fn unknown_event_is_404() {
    let (state, router) = setup("unknown").await;
    let e = create(&state, &router, "all").await;
    let (res, body) = post_json(&router, &public_uri(&"x".repeat(22)), &nominal(&state, &e)).await;
    assert_problem(&res, &body, "event.not_found", "unknown");
    assert_nothing_written(&state, "unknown").await;
}

#[tokio::test]
async fn two_hundred_candidatures_is_the_ceiling() {
    let (state, router) = setup("ceiling").await;
    let e = create(&state, &router, "all").await;
    let insert = |n: usize| {
        let (db, id) = (state.db.write.clone(), e.id.clone());
        async move {
            sqlx::query(
                "INSERT INTO candidatures (id, event_id, pseudo, server, class, cp, status, \
                 origin, created_at, updated_at) VALUES (?, ?, 'Filler', 'siel', 'cleric', 1, ?, \
                 'public', ?, ?)",
            )
            .bind(format!("{:0>22}", n))
            .bind(id)
            // Tous statuts confondus.
            .bind(if n.is_multiple_of(2) {
                "withdrawn"
            } else {
                "rejected"
            })
            .bind(NOW)
            .bind(NOW)
            .execute(&db)
            .await
            .unwrap();
        }
    };
    for n in 0..199 {
        insert(n).await;
    }
    // 199 lignes : la 200e passe, la suivante est refusée.
    let (res, body) = post_json(&router, &public_uri(&e.id), &nominal(&state, &e)).await;
    assert_eq!(res.status(), StatusCode::CREATED, "{body}");
    assert_eq!(count(&state, "candidatures").await, 200);
    let (res, body) = post_json(&router, &public_uri(&e.id), &nominal(&state, &e)).await;
    assert_problem(&res, &body, "candidature.limit_reached", "ceiling");
    assert_eq!(count(&state, "candidatures").await, 200);
    assert_eq!(count(&state, "availabilities").await, 2);
}

#[tokio::test]
async fn breaker_pauses_candidatures() {
    let (state, router) = setup_with(
        "breaker",
        &test_config(&[
            ("MAX_CANDIDATURES_PER_HOUR", "1"),
            ("RATE_CANDIDATURE", "100/h"),
        ]),
    )
    .await;
    let e = create(&state, &router, "all").await;
    let (res, _) = post_json(&router, &public_uri(&e.id), &nominal(&state, &e)).await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let (res, body) = post_json(&router, &public_uri(&e.id), &nominal(&state, &e)).await;
    assert_problem(&res, &body, "limit.paused", "breaker");
    assert_eq!(header(&res, "retry-after"), "3600");
    assert_eq!(count(&state, "candidatures").await, 1);
}

#[tokio::test]
async fn honeypot_and_form_token() {
    let (state, router) = setup("form").await;
    let e = create(&state, &router, "all").await;
    let create_token = token(&state, FormClass::Create);
    let fresh = state
        .form_tokens
        .issue(FormClass::Candidature, ago(0), [4; 16]);
    for (patch, code) in [
        (json!({ "website": "x" }), "form.rejected"),
        (json!({ "formToken": create_token }), "form.token_invalid"),
        (json!({ "formToken": fresh }), "form.too_fast"),
    ] {
        let body = with(nominal(&state, &e), patch);
        let (res, text) = post_json(&router, &public_uri(&e.id), &body).await;
        assert_problem(&res, &text, code, code);
    }
    for raw in [
        "{".to_owned(),
        with(nominal(&state, &e), json!({ "cp": "1500" })).to_string(),
        with(
            nominal(&state, &e),
            json!({ "availabilities": [{ "preferred": true }] }),
        )
        .to_string(),
    ] {
        let (res, text) = post_raw(&router, &public_uri(&e.id), &raw).await;
        assert_problem(&res, &text, "form.malformed", &raw);
    }
    assert_nothing_written(&state, "form").await;
}

#[tokio::test]
async fn candidature_routes_use_the_candidature_quota() {
    let (state, router) = setup_with(
        "quota",
        &test_config(&[("RATE_CANDIDATURE", "1/h"), ("RATE_CREATE", "10/h")]),
    )
    .await;
    let e = create(&state, &router, "all").await;
    let (res, _) = post_json(&router, &public_uri(&e.id), &nominal(&state, &e)).await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let (res, body) = post_json_with(
        &router,
        JOIN_CANDIDATURES,
        &nominal(&state, &e),
        &[("x-link-secret", &e.join)],
    )
    .await;
    assert_problem(&res, &body, "limit.rate_limited", "quota");
    let (res, body) = send(&router, Method::GET, JOIN_CANDIDATURES).await;
    assert_problem(&res, &body, "server.method_not_allowed", "GET");
}

/// Journal capturé en mémoire.
#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

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
async fn no_secret_nor_pseudo_in_the_logs() {
    let (state, router) = setup("logs").await;
    let e = create(&state, &router, "all").await;
    let requests = || async {
        let (res, body) = post_json_with(
            &router,
            JOIN_CANDIDATURES,
            &nominal(&state, &e),
            &[("x-link-secret", &e.join)],
        )
        .await;
        assert_eq!(res.status(), StatusCode::CREATED);
        let bad = with(nominal(&state, &e), json!({ "server": "far_away" }));
        let (res, _) = post_json(&router, &public_uri(&e.id), &bad).await;
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
        body
    };
    // Même gabarit que `events_create::secrets_never_reach_the_logs`.
    requests().await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let captured = Captured::default();
    let writer = captured.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_ansi(false)
        .with_writer(move || writer.clone())
        .finish();
    let guard = tracing::subscriber::set_default(subscriber);
    tracing::callsite::rebuild_interest_cache();
    let body = requests().await;
    drop(guard);
    let log = String::from_utf8(captured.0.lock().unwrap().clone()).unwrap();
    let v: Value = serde_json::from_str(&body).unwrap();
    assert!(log.contains("candidature submitted"), "{log}");
    assert!(log.contains(v["candidatureId"].as_str().unwrap()), "{log}");
    for leak in [
        v["participantSecret"].as_str().unwrap(),
        e.join.as_str(),
        "Nyraelle",
        TEST_CLIENT_IP,
    ] {
        assert!(!log.contains(leak), "{leak} in {log}");
    }
}
