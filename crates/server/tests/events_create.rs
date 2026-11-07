//! Matrice HTTP de la story 2-1 : `POST /api/events`, lignes écrites, liens hachés, refus.

mod common;

use std::borrow::Cow;
use std::sync::{Arc, Mutex};

use axum::http::StatusCode;
use common::*;
use domain::Timestamp;
use serde_json::{Value, json};
use server::app::AppState;
use server::http::form_token::FormClass;
use server::infra::links::{self, Right};
use server::infra::reference::{self, ReferenceStore};
use server::infra::sqlite::{self, Db, MIGRATOR};
use sqlx::migrate::Migrator;

const EVENTS: &str = "/api/events";
/// Instant de l'horloge de test (`common::state_with_config`).
const NOW: &str = "2026-11-06T10:00:00Z";

fn ts(s: &str) -> Timestamp {
    s.parse().unwrap()
}

/// Jeton `create` émis dix secondes avant l'horloge de test : assez vieux pour passer.
fn token(state: &AppState) -> String {
    let issued = Timestamp::from_unix_seconds(ts(NOW).unix_seconds() - 10).unwrap();
    state.form_tokens.issue(FormClass::Create, issued, [3; 16])
}

/// Corps nominal : Ludra, Europe, toutes factions, trois créneaux futurs, sans bornes.
fn nominal(token: &str) -> Value {
    json!({
        "formToken": token,
        "website": "",
        "instance": "ludra",
        "region": "europe",
        "factionScope": "all",
        "slots": [
            { "startsAt": "2026-11-11T20:00:00Z" },
            { "startsAt": "2026-11-12T20:00:00Z" },
            { "startsAt": "2026-11-14T20:00:00Z" },
        ],
    })
}

fn with(mut body: Value, patch: Value) -> Value {
    for (k, v) in patch.as_object().unwrap() {
        body[k] = v.clone();
    }
    body
}

async fn count(db: &Db, table: &str) -> i64 {
    let (n,): (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {table}"))
        .fetch_one(&db.read)
        .await
        .unwrap();
    n
}

async fn assert_nothing_written(db: &Db, ctx: &str) {
    for table in ["events", "slots", "class_bounds", "links"] {
        assert_eq!(count(db, table).await, 0, "{ctx}: {table}");
    }
}

/// 400 problem+json avec `code` et exactement `fields`.
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
    assert_eq!(v["status"], 400);
    assert_eq!(v["type"], "about:blank");
    assert_eq!(v["fields"], fields, "{body}");
}

async fn setup(name: &str) -> (AppState, axum::Router) {
    let state = memory_state(&format!("events_{name}")).await;
    let router = fixture_router(state.clone());
    (state, router)
}

#[tokio::test]
async fn nominal_creation_writes_the_event_its_slots_bounds_and_hashed_links() {
    let (state, router) = setup("nominal").await;
    let (res, body) = post_json(&router, EVENTS, &nominal(&token(&state))).await;
    assert_eq!(res.status(), StatusCode::CREATED, "{body}");
    assert_eq!(header(&res, "cache-control"), "no-store");
    assert_eq!(header(&res, "content-type"), "application/json");

    let v: Value = serde_json::from_str(&body).unwrap();
    let mut keys: Vec<_> = v.as_object().unwrap().keys().cloned().collect();
    keys.sort_unstable();
    assert_eq!(keys, ["eventId", "joinSecret", "links", "organizerSecret"]);
    let event_id = v["eventId"].as_str().unwrap();
    let organizer = v["organizerSecret"].as_str().unwrap();
    let join = v["joinSecret"].as_str().unwrap();
    assert_eq!(event_id.len(), 22);
    assert_eq!((organizer.len(), join.len()), (43, 43));
    assert_ne!(organizer, join);
    assert_eq!(
        v["links"],
        json!({
            "organizer": format!("/o/{organizer}"),
            "join": format!("/j/{join}"),
            "public": format!("/e/{event_id}"),
        })
    );

    let db = &state.db;
    let row: Value = {
        let r: (String, String, String, String, String, String) = sqlx::query_as(
            "SELECT id, instance, region, faction_scope, visibility, status FROM events",
        )
        .fetch_one(&db.read)
        .await
        .unwrap();
        let o: (Option<i64>, Option<String>, String, String) =
            sqlx::query_as("SELECT min_cp, discord_url, note, created_at FROM events")
                .fetch_one(&db.read)
                .await
                .unwrap();
        json!([r.0, r.1, r.2, r.3, r.4, r.5, o.0, o.1, o.2, o.3])
    };
    assert_eq!(
        row,
        json!([
            event_id, "ludra", "europe", "all", "listed", "open", null, null, "", NOW
        ])
    );

    let slots: Vec<(String, Timestamp, i64)> =
        sqlx::query_as("SELECT event_id, starts_at, duration_min FROM slots ORDER BY starts_at")
            .fetch_all(&db.read)
            .await
            .unwrap();
    assert_eq!(
        slots,
        [
            "2026-11-11T20:00:00Z",
            "2026-11-12T20:00:00Z",
            "2026-11-14T20:00:00Z"
        ]
        .map(|s| (event_id.to_owned(), ts(s), 60))
    );

    // Une borne par classe active, aux valeurs par défaut de la référence.
    let set = state.reference.get().unwrap();
    let expected: Vec<(String, i64, i64)> = set
        .classes
        .iter()
        .filter(|c| c.enabled)
        .map(|c| {
            (
                c.id.clone(),
                i64::from(c.default_min),
                i64::from(c.default_max),
            )
        })
        .collect();
    assert!(!expected.is_empty());
    let mut bounds: Vec<(String, i64, i64)> = sqlx::query_as(
        "SELECT class, min_count, max_count FROM class_bounds WHERE event_id = ? ORDER BY class",
    )
    .bind(event_id)
    .fetch_all(&db.read)
    .await
    .unwrap();
    let mut expected = expected;
    expected.sort();
    bounds.sort();
    assert_eq!(bounds, expected);

    // Deux liens, trouvés par le hash, jamais stockés en clair.
    assert_eq!(count(db, "links").await, 2);
    for (secret, right) in [(organizer, Right::Organizer), (join, Right::Join)] {
        let found = links::find(&db.read, &links::hash(secret))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.right, right);
        assert_eq!(found.event_id, event_id);
        assert_eq!(found.candidature_id, None);
    }
    let dump: Vec<(String,)> = sqlx::query_as(
        "SELECT id || instance || region || note || created_at FROM events \
         UNION ALL SELECT id || event_id || starts_at FROM slots \
         UNION ALL SELECT hex(secret_hash) || link_right || event_id FROM links",
    )
    .fetch_all(&db.read)
    .await
    .unwrap();
    for (text,) in dump {
        assert!(!text.contains(organizer) && !text.contains(join));
    }
}

#[tokio::test]
async fn explicit_options_are_stored() {
    let (state, router) = setup("options").await;
    let body = with(
        nominal(&token(&state)),
        json!({
            "slots": [{ "startsAt": "2026-11-11T20:00:00Z", "durationMin": 90 }],
            "note": "Vocal prêt à 20 h 45",
            "discordUrl": "https://discord.gg/abc",
            "visibility": "unlisted",
            "minCp": 1500,
            "classBounds": { "cleric": { "min": 2, "max": 3 } },
        }),
    );
    let (res, text) = post_json(&router, EVENTS, &body).await;
    assert_eq!(res.status(), StatusCode::CREATED, "{text}");
    let row: (String, Option<i64>, Option<String>, String) =
        sqlx::query_as("SELECT visibility, min_cp, discord_url, note FROM events")
            .fetch_one(&state.db.read)
            .await
            .unwrap();
    assert_eq!(
        row,
        (
            "unlisted".into(),
            Some(1500),
            Some("https://discord.gg/abc".into()),
            "Vocal prêt à 20 h 45".into()
        )
    );
    let (d,): (i64,) = sqlx::query_as("SELECT duration_min FROM slots")
        .fetch_one(&state.db.read)
        .await
        .unwrap();
    assert_eq!(d, 90);
    let b: (i64, i64) =
        sqlx::query_as("SELECT min_count, max_count FROM class_bounds WHERE class = 'cleric'")
            .fetch_one(&state.db.read)
            .await
            .unwrap();
    assert_eq!(b, (2, 3));
}

#[tokio::test]
async fn slot_in_the_past_is_refused_with_its_field() {
    let (state, router) = setup("past").await;
    for past in [NOW, "2026-11-05T20:00:00Z"] {
        let body = with(
            nominal(&token(&state)),
            json!({ "slots": [
                { "startsAt": "2026-11-11T20:00:00Z" },
                { "startsAt": past },
            ] }),
        );
        let (res, text) = post_json(&router, EVENTS, &body).await;
        assert_fields(
            &res,
            &text,
            "slot.in_past",
            json!({ "slots.1.startsAt": "slot.in_past" }),
        );
    }
    assert_nothing_written(&state.db, "past").await;
}

#[tokio::test]
async fn every_field_error_is_reported_and_the_first_names_the_code() {
    let (state, router) = setup("several").await;
    let body = with(
        nominal(&token(&state)),
        json!({ "note": "voir https://example.org", "region": "atlantis" }),
    );
    let (res, text) = post_json(&router, EVENTS, &body).await;
    assert_fields(
        &res,
        &text,
        "reference.unknown",
        json!({ "region": "reference.unknown", "note": "event.note_invalid" }),
    );
    assert_nothing_written(&state.db, "several").await;
}

#[tokio::test]
async fn slot_count_and_duplicates() {
    let (state, router) = setup("slot_count").await;
    let many: Vec<Value> = (10..25)
        .map(|d| json!({ "startsAt": format!("2026-11-{d}T20:00:00Z") }))
        .collect();
    assert_eq!(many.len(), 15);
    for (slots, code, path) in [
        (json!([]), "slot.count_out_of_range", "slots"),
        (Value::Array(many), "slot.count_out_of_range", "slots"),
        (
            json!([
                { "startsAt": "2026-11-11T20:00:00Z" },
                { "startsAt": "2026-11-11T20:00:00Z" },
            ]),
            "slot.duplicate",
            "slots.1.startsAt",
        ),
        (
            json!([{ "startsAt": "2026-11-11T20:00:00Z", "durationMin": 721 }]),
            "slot.duration_out_of_range",
            "slots.0.durationMin",
        ),
    ] {
        let body = with(nominal(&token(&state)), json!({ "slots": slots }));
        let (res, text) = post_json(&router, EVENTS, &body).await;
        assert_fields(&res, &text, code, json!({ path: code }));
    }
    assert_nothing_written(&state.db, "slot count").await;
}

#[tokio::test]
async fn class_bounds_out_of_range() {
    let (state, router) = setup("bounds").await;
    for (bound, path) in [
        (json!({ "min": 3, "max": 2 }), "classBounds.cleric.min"),
        // Ludra : 2 groupes de 5.
        (json!({ "min": 0, "max": 11 }), "classBounds.cleric.max"),
    ] {
        let body = with(
            nominal(&token(&state)),
            json!({ "classBounds": { "cleric": bound } }),
        );
        let (res, text) = post_json(&router, EVENTS, &body).await;
        assert_fields(
            &res,
            &text,
            "event.class_bounds_invalid",
            json!({ path: "event.class_bounds_invalid" }),
        );
    }
    let body = with(
        nominal(&token(&state)),
        json!({ "classBounds": { "bard": { "min": 0, "max": 1 } } }),
    );
    let (res, text) = post_json(&router, EVENTS, &body).await;
    assert_fields(
        &res,
        &text,
        "reference.unknown",
        json!({ "classBounds.bard": "reference.unknown" }),
    );
    assert_nothing_written(&state.db, "bounds").await;
}

#[tokio::test]
async fn faction_scope_all_is_refused_on_a_single_faction_instance() {
    let mut set = reference::embedded().unwrap();
    let mut mono = set.instances[0].clone();
    mono.id = "mono_faction".into();
    mono.cross_faction = false;
    set.instances.push(mono);
    let db = memory_db("events_scope").await;
    let state = state_with(db, ReferenceStore::with(set));
    let router = fixture_router(state.clone());
    let body = with(
        nominal(&token(&state)),
        json!({ "instance": "mono_faction" }),
    );
    let (res, text) = post_json(&router, EVENTS, &body).await;
    assert_fields(
        &res,
        &text,
        "event.faction_scope_not_allowed",
        json!({ "factionScope": "event.faction_scope_not_allowed" }),
    );
    assert_nothing_written(&state.db, "scope").await;
    let body = with(body, json!({ "factionScope": "elyos" }));
    let (res, text) = post_json(&router, EVENTS, &body).await;
    assert_eq!(res.status(), StatusCode::CREATED, "{text}");
}

#[tokio::test]
async fn honeypot_and_form_token() {
    let (state, router) = setup("form").await;
    let fresh = state.form_tokens.issue(FormClass::Create, ts(NOW), [4; 16]);
    let candidature = state.form_tokens.issue(
        FormClass::Candidature,
        Timestamp::from_unix_seconds(ts(NOW).unix_seconds() - 10).unwrap(),
        [5; 16],
    );
    let mut no_token = nominal("");
    no_token.as_object_mut().unwrap().remove("formToken");
    for (body, code) in [
        (
            with(nominal(&token(&state)), json!({ "website": "x" })),
            "form.rejected",
        ),
        (no_token, "form.token_invalid"),
        (nominal(&candidature), "form.token_invalid"),
        (nominal(&fresh), "form.too_fast"),
    ] {
        let (res, text) = post_json(&router, EVENTS, &body).await;
        assert_eq!(res.status(), StatusCode::BAD_REQUEST, "{code}");
        assert_problem(&res, &text, code, code);
    }
    assert_nothing_written(&state.db, "form").await;
}

#[tokio::test]
async fn malformed_json_is_a_problem() {
    let state = state_with_config(
        memory_db("events_malformed").await,
        ReferenceStore::with(reference::embedded().unwrap()),
        &test_config(&[("RATE_CREATE", "100/h")]),
    );
    let router = fixture_router(state.clone());
    let t = token(&state);
    for raw in [
        "{".to_owned(),
        String::new(),
        "[]".to_owned(),
        with(nominal(&t), json!({ "slots": "tomorrow" })).to_string(),
        with(nominal(&t), json!({ "factionScope": "neutral" })).to_string(),
        with(
            nominal(&t),
            json!({ "slots": [{ "startsAt": "2026-11-11T20:00:00+01:00" }] }),
        )
        .to_string(),
        with(nominal(&t), json!({ "minCp": "1000" })).to_string(),
        {
            let mut b = nominal(&t);
            b.as_object_mut().unwrap().remove("instance");
            b.to_string()
        },
    ] {
        let (res, text) = post_raw(&router, EVENTS, &raw).await;
        assert_problem(&res, &text, "form.malformed", &raw);
        assert_eq!(header(&res, "cache-control"), "no-store");
    }
    assert_nothing_written(&state.db, "malformed").await;
}

#[tokio::test]
async fn breaker_pauses_creation_with_retry_after() {
    let state = state_with_config(
        memory_db("events_breaker").await,
        ReferenceStore::with(reference::embedded().unwrap()),
        &test_config(&[("MAX_EVENTS_PER_HOUR", "1")]),
    );
    let router = fixture_router(state.clone());
    let (res, _) = post_json(&router, EVENTS, &nominal(&token(&state))).await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let (res, text) = post_json(&router, EVENTS, &nominal(&token(&state))).await;
    assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_problem(&res, &text, "limit.paused", "breaker");
    assert_eq!(header(&res, "retry-after"), "3600");
    assert_eq!(count(&state.db, "events").await, 1);
    assert_eq!(count(&state.db, "links").await, 2);
}

/// La vraie route est sous `RouteClass::Create` : son quota `RATE_CREATE` s'applique.
#[tokio::test]
async fn create_route_uses_the_create_quota() {
    let state = state_with_config(
        memory_db("events_quota").await,
        ReferenceStore::with(reference::embedded().unwrap()),
        &test_config(&[("RATE_CREATE", "2/h")]),
    );
    let router = fixture_router(state.clone());
    for _ in 0..2 {
        let (res, text) = post_json(&router, EVENTS, &nominal(&token(&state))).await;
        assert_eq!(res.status(), StatusCode::CREATED, "{text}");
    }
    let (res, text) = post_json(&router, EVENTS, &nominal(&token(&state))).await;
    assert_eq!(res.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_problem(&res, &text, "limit.rate_limited", "quota");
    assert_eq!(count(&state.db, "events").await, 2);
}

#[tokio::test]
async fn reference_unavailable_is_503() {
    let db = memory_db("events_no_reference").await;
    let state = state_with(db, ReferenceStore::new());
    let router = fixture_router(state.clone());
    let (res, text) = post_json(&router, EVENTS, &nominal(&token(&state))).await;
    assert_problem(&res, &text, "reference.unavailable", "no reference");
    assert_nothing_written(&state.db, "no reference").await;
}

#[tokio::test]
async fn create_route_is_keyed_and_post_only() {
    let (_, router) = setup("route").await;
    let (res, body) = send_bare(&router, axum::http::Method::POST, EVENTS, &[]).await;
    assert_problem(&res, &body, "limit.client_ip_missing", "no ip");
    let (res, body) = send(&router, axum::http::Method::GET, EVENTS).await;
    assert_problem(&res, &body, "server.method_not_allowed", "GET");
}

/// `0002` sur une base de la story 1-7 : les liens restent, le modèle de l'epic 2 arrive sans les
/// tables de l'epic 3, et supprimer un événement emporte tout ce qui en dépend.
#[tokio::test]
async fn migration_0002_on_a_1_7_base_and_cascade() {
    let db = Db::open(sqlite::shared_memory_options("events_migration"))
        .await
        .unwrap();
    let first: Vec<_> = MIGRATOR
        .iter()
        .filter(|m| m.version == 1)
        .cloned()
        .collect();
    assert_eq!(first.len(), 1);
    let v1 = Migrator {
        migrations: Cow::Owned(first),
        ..Migrator::DEFAULT
    };
    v1.run(&db.write).await.unwrap();
    let old_event = "e".repeat(22);
    sqlx::query("INSERT INTO links VALUES (zeroblob(32), 'organizer', ?, NULL)")
        .bind(&old_event)
        .execute(&db.write)
        .await
        .unwrap();

    db.migrate().await.unwrap();
    assert_eq!(count(&db, "links").await, 1);
    for table in ["groups", "assignments", "confirmations"] {
        let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM sqlite_master WHERE name = ?")
            .bind(table)
            .fetch_one(&db.read)
            .await
            .unwrap();
        assert_eq!(n, 0, "{table}");
    }

    let state = state(db.clone());
    let router = fixture_router(state.clone());
    let (res, body) = post_json(&router, EVENTS, &nominal(&token(&state))).await;
    assert_eq!(res.status(), StatusCode::CREATED, "{body}");
    let event_id = serde_json::from_str::<Value>(&body).unwrap()["eventId"]
        .as_str()
        .unwrap()
        .to_owned();
    let (slot_id,): (String,) = sqlx::query_as("SELECT id FROM slots LIMIT 1")
        .fetch_one(&db.read)
        .await
        .unwrap();
    let cand = "c".repeat(22);
    sqlx::query(
        "INSERT INTO candidatures (id, event_id, pseudo, server, class, cp, status, origin, \
         created_at, updated_at) VALUES (?, ?, 'Kael', 'siel', 'cleric', 1000, 'pending', \
         'public', ?, ?)",
    )
    .bind(&cand)
    .bind(&event_id)
    .bind(ts(NOW))
    .bind(ts(NOW))
    .execute(&db.write)
    .await
    .unwrap();
    sqlx::query("INSERT INTO availabilities VALUES (?, ?, 1)")
        .bind(&cand)
        .bind(&slot_id)
        .execute(&db.write)
        .await
        .unwrap();
    // Clé étrangère active : une disponibilité vers un créneau inconnu est refusée.
    assert!(
        sqlx::query("INSERT INTO availabilities VALUES (?, ?, 0)")
            .bind(&cand)
            .bind("x".repeat(22))
            .execute(&db.write)
            .await
            .is_err()
    );

    sqlx::query("DELETE FROM events WHERE id = ?")
        .bind(&event_id)
        .execute(&db.write)
        .await
        .unwrap();
    for table in ["slots", "class_bounds", "candidatures", "availabilities"] {
        assert_eq!(count(&db, table).await, 0, "cascade {table}");
    }
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
async fn secrets_never_reach_the_logs() {
    let state = state_with_config(
        memory_db("events_logs").await,
        ReferenceStore::with(reference::embedded().unwrap()),
        &test_config(&[("RATE_CREATE", "100/h")]),
    );
    let router = fixture_router(state.clone());
    let t = token(&state);
    let requests = || async {
        let (res, body) = post_json(&router, EVENTS, &nominal(&t)).await;
        assert_eq!(res.status(), StatusCode::CREATED);
        let (res, _) = post_raw(&router, EVENTS, "{\"formToken\":").await;
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
        let (res, _) = post_json(
            &router,
            EVENTS,
            &with(nominal(&t), json!({ "website": "x" })),
        )
        .await;
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
        body
    };
    // Les tests tournent en parallèle et `tracing` met en cache l'intérêt de chaque callsite : les
    // callsites sont touchés d'abord, puis le cache est reconstruit une fois le collecteur posé (même
    // gabarit que `http::abuse_tests`).
    // Premier passage sous un abonné muet : sans abonné, un callsite enregistré depuis un autre fil
    // peut voir son intérêt figé à « jamais » (même gabarit que `candidature_submit`).
    {
        let mute = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .with_writer(std::io::sink)
            .finish();
        let _mute = tracing::subscriber::set_default(mute);
        requests().await;
    }
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
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
    let body = requests().await;
    drop(guard);
    let log = String::from_utf8(captured.0.lock().unwrap().clone()).unwrap();
    let v: Value = serde_json::from_str(&body).unwrap();
    let event_id = v["eventId"].as_str().unwrap();
    assert!(log.contains("event created"), "{log}");
    assert!(log.contains(event_id), "{log}");
    assert!(log.contains("form.malformed"), "{log}");
    for leak in [
        v["organizerSecret"].as_str().unwrap(),
        v["joinSecret"].as_str().unwrap(),
        t.as_str(),
        TEST_CLIENT_IP,
    ] {
        assert!(!log.contains(leak), "{leak} in {log}");
    }
}
