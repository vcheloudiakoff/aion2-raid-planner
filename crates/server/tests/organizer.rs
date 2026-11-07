//! Matrice HTTP de la story 2-4 : `GET /api/organizer` et `POST /api/organizer/candidature-status`,
//! sous le seul secret organisateur. Ordre de création, `Pseudo[SRV]`, doublons, transitions d'AD-4,
//! candidature hors de l'événement, statut invalide, quotas `read` et `write`, journaux muets.

mod common;

use std::sync::{Arc, Mutex};

use axum::http::{Method, StatusCode};
use common::candidatures::*;
use common::*;
use serde_json::{Value, json};
use server::app::AppState;
use server::config::Config;

const VIEW: &str = "/api/organizer";
const DECIDE: &str = "/api/organizer/candidature-status";

async fn setup_with(name: &str, config: &Config) -> (AppState, axum::Router) {
    candidature_setup(&format!("organizer_{name}"), config).await
}

async fn setup(name: &str) -> (AppState, axum::Router) {
    setup_with(
        name,
        &test_config(&[
            ("RATE_CREATE", "100/h"),
            ("RATE_CANDIDATURE", "1000/h"),
            ("RATE_WRITE", "1000/h"),
        ]),
    )
    .await
}

fn organizer(e: &Event) -> [(&'static str, &str); 1] {
    [("x-link-secret", e.organizer.as_str())]
}

async fn read(router: &axum::Router, e: &Event) -> (StatusCode, Value, String) {
    let (res, body) = send_with(router, Method::GET, VIEW, &organizer(e)).await;
    assert_eq!(header(&res, "cache-control"), "no-store");
    let status = res.status();
    (
        status,
        serde_json::from_str(&body).unwrap_or(Value::Null),
        body,
    )
}

/// Décision de l'organisateur sur `id`.
async fn decide(
    router: &axum::Router,
    e: &Event,
    id: &str,
    status: &str,
) -> (axum::http::Response<axum::body::Body>, String) {
    post_json_with(
        router,
        DECIDE,
        &json!({ "candidatureId": id, "status": status }),
        &organizer(e),
    )
    .await
}

/// Statut et `updated_at` en base.
async fn stored(state: &AppState, c: &Cand) -> (String, String) {
    sqlx::query_as("SELECT status, updated_at FROM candidatures WHERE id = ?")
        .bind(&c.id)
        .fetch_one(&state.db.read)
        .await
        .unwrap()
}

/// Toutes les lignes de candidature de la base : rien ne doit changer sur un refus.
async fn all_rows(state: &AppState) -> Vec<(String, String, String)> {
    sqlx::query_as("SELECT id, status, updated_at FROM candidatures ORDER BY id")
        .fetch_all(&state.db.read)
        .await
        .unwrap()
}

fn sorted_keys(v: &Value) -> Vec<String> {
    let mut keys: Vec<_> = v.as_object().unwrap().keys().cloned().collect();
    keys.sort_unstable();
    keys
}

fn ids(v: &Value) -> Vec<String> {
    v["candidatures"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap().to_owned())
        .collect()
}

fn find<'a>(v: &'a Value, c: &Cand) -> &'a Value {
    v["candidatures"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["id"] == c.id.as_str())
        .unwrap_or_else(|| panic!("{} missing", c.id))
}

async fn set_created_at(state: &AppState, c: &Cand, at: &str) {
    sql(
        state,
        "UPDATE candidatures SET created_at = ? WHERE id = ?",
        &[at, &c.id],
    )
    .await;
}

#[tokio::test]
async fn read_lists_every_candidature_in_creation_order_with_pseudo_srv() {
    let (state, router) = setup("read").await;
    let e = create_with(
        &state,
        &router,
        "all",
        json!({ "discordUrl": "https://discord.gg/abcdef" }),
    )
    .await;
    let a = candidate(&state, &router, &e, "Nyraelle", false).await;
    let b = candidate(&state, &router, &e, "Kaelthorn", true).await;
    let c = candidate_with(
        &state,
        &router,
        &e,
        json!({
            "pseudo": "Tomasquin",
            "server": "siel",
            "class": "templar",
            "availabilities": [{ "slotId": e.slots[1], "preferred": false }],
        }),
        false,
    )
    .await;
    // `c` est la plus ancienne ; `a` et `b` à égalité se départagent par id.
    set_created_at(&state, &c, "2026-11-06T09:00:00Z").await;
    set_created_at(&state, &a, "2026-11-06T09:30:00Z").await;
    set_created_at(&state, &b, "2026-11-06T09:30:00Z").await;
    let mut tie = [a.id.clone(), b.id.clone()];
    tie.sort();

    let (status, v, body) = read(&router, &e).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        sorted_keys(&v),
        ["candidatures", "classBounds", "discordUrl", "event"]
    );
    assert_eq!(v["discordUrl"], "https://discord.gg/abcdef");
    assert_eq!(ids(&v), [c.id.clone(), tie[0].clone(), tie[1].clone()]);

    let first = &v["candidatures"][0];
    assert_eq!(
        sorted_keys(first),
        [
            "availabilities",
            "class",
            "cp",
            "duplicate",
            "id",
            "origin",
            "pseudo",
            "pseudoSrv",
            "role",
            "server",
            "status"
        ]
    );
    assert_eq!(
        *first,
        json!({
            "id": c.id,
            "pseudo": "Tomasquin",
            "server": "siel",
            "pseudoSrv": "Tomasquin[Siel]",
            "class": "templar",
            "role": "tank",
            "cp": 1500,
            "status": "pending",
            "origin": "public",
            "availabilities": [{ "slotId": e.slots[1], "preferred": false }],
            "duplicate": false,
        })
    );
    let nyra = find(&v, &a);
    assert_eq!(nyra["pseudoSrv"], "Nyraelle[Israphel]");
    assert_eq!(nyra["role"], "healer");
    assert_eq!(nyra["origin"], "public");
    assert_eq!(nyra["status"], "pending");
    assert_eq!(
        nyra["availabilities"],
        json!([
            { "slotId": e.slots[0], "preferred": true },
            { "slotId": e.slots[1], "preferred": true },
        ])
    );
    let kael = find(&v, &b);
    assert_eq!(kael["origin"], "direct");
    assert_eq!(kael["status"], "accepted");

    // L'événement imbriqué est le `PublicEvent` : comptes publics, sans `hasGroups` (epic 3).
    assert_eq!(v["event"]["id"], e.id);
    assert_eq!(v["event"]["countsByClass"]["cleric"], 2);
    assert_eq!(v["event"]["countsByClass"]["templar"], 1);
    assert!(v["event"].get("candidatures").is_none());
    assert!(v["event"].get("discordUrl").is_none());
    for slot in v["event"]["slots"].as_array().unwrap() {
        assert_eq!(sorted_keys(slot), ["durationMin", "id", "startsAt"]);
    }
    // Une borne par classe active à la création.
    let bounds = v["classBounds"].as_object().unwrap();
    assert!(!bounds.is_empty());
    assert_eq!(
        bounds.len(),
        v["event"]["countsByClass"].as_object().unwrap().len()
    );
    assert_eq!(bounds["cleric"], json!({ "min": 0, "max": 10 }));

    // Sans lien Discord : la clé n'existe pas.
    let bare = create(&state, &router, "all").await;
    let (status, v, body) = read(&router, &bare).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(v.get("discordUrl").is_none(), "{body}");
    assert_eq!(v["candidatures"], json!([]));
}

#[tokio::test]
async fn a_read_tolerates_a_server_and_a_class_missing_from_the_reference() {
    let (state, router) = setup("retired").await;
    let e = create(&state, &router, "all").await;
    candidate(&state, &router, &e, "Nyraelle", false).await;
    let mut set = reference_set();
    set.servers.retain(|s| s.id != "israphel");
    set.classes.retain(|c| c.id != "cleric");
    state.reference.replace(std::sync::Arc::new(set));
    let (status, v, body) = read(&router, &e).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(v["candidatures"][0]["pseudoSrv"], "Nyraelle[israphel]");
    assert_eq!(v["candidatures"][0]["role"], "cleric");
}

#[tokio::test]
async fn only_the_organizer_secret_opens_both_routes() {
    let (state, router) = setup("rights").await;
    let e = create(&state, &router, "all").await;
    let c = candidate(&state, &router, &e, "Nyraelle", false).await;
    let unknown = "A".repeat(43);
    let before = all_rows(&state).await;
    let body = json!({ "candidatureId": c.id, "status": "accepted" });
    for (headers, code) in [
        (vec![], "auth.missing"),
        (vec![("x-link-secret", unknown.as_str())], "auth.not_found"),
        (vec![("x-link-secret", e.join.as_str())], "auth.wrong_right"),
        (
            vec![("x-link-secret", c.secret.as_str())],
            "auth.wrong_right",
        ),
    ] {
        let (res, text) = send_with(&router, Method::GET, VIEW, &headers).await;
        assert_problem(&res, &text, code, &format!("GET {code}"));
        let (res, text) = post_json_with(&router, DECIDE, &body, &headers).await;
        assert_problem(&res, &text, code, &format!("POST {code}"));
    }
    assert_eq!(all_rows(&state).await, before);
    // Ni l'id ni le secret dans l'URL : une query string ne vaut rien.
    let (res, text) = send(
        &router,
        Method::GET,
        &format!("{VIEW}?secret={}", e.organizer),
    )
    .await;
    assert_problem(&res, &text, "auth.missing", "query");
    // Route de commande : POST seulement.
    let (res, text) = send_with(&router, Method::GET, DECIDE, &organizer(&e)).await;
    assert_problem(&res, &text, "server.method_not_allowed", "GET on POST");
}

#[tokio::test]
async fn a_link_whose_event_is_gone_leads_nowhere() {
    let (state, router) = setup("gone").await;
    let e = create(&state, &router, "all").await;
    let c = candidate(&state, &router, &e, "Nyraelle", false).await;
    // Purge simulée : l'événement et ses candidatures partent, la ligne du lien reste.
    sql(&state, "DELETE FROM events WHERE id = ?", &[&e.id]).await;
    let (res, text) = send_with(&router, Method::GET, VIEW, &organizer(&e)).await;
    assert_problem(&res, &text, "auth.not_found", "GET");
    let (res, text) = decide(&router, &e, &c.id, "accepted").await;
    assert_problem(&res, &text, "auth.not_found", "POST");
}

#[tokio::test]
async fn duplicates_are_flagged_on_pending_and_accepted_only() {
    let (state, router) = setup("duplicates").await;
    let e = create(&state, &router, "all").await;
    let pending = candidate(&state, &router, &e, "Abc", false).await;
    let accepted = candidate(&state, &router, &e, "abc", true).await;
    let rejected = candidate(&state, &router, &e, "Abc", false).await;
    set_status(&state, &rejected, "rejected").await;
    let other_server = candidate_with(
        &state,
        &router,
        &e,
        json!({ "pseudo": "Abc", "server": "siel" }),
        false,
    )
    .await;
    let (_, v, body) = read(&router, &e).await;
    assert_eq!(find(&v, &pending)["duplicate"], true, "{body}");
    assert_eq!(find(&v, &accepted)["duplicate"], true, "{body}");
    assert_eq!(find(&v, &rejected)["duplicate"], false, "{body}");
    assert_eq!(find(&v, &other_server)["duplicate"], false, "{body}");
    // Rien n'est dédoublonné.
    assert_eq!(v["candidatures"].as_array().unwrap().len(), 4);

    // L'organisateur refuse l'un des deux : le signal disparaît de l'autre.
    let (res, text) = decide(&router, &e, &pending.id, "rejected").await;
    assert_eq!(res.status(), StatusCode::OK, "{text}");
    let v: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(find(&v, &pending)["duplicate"], false);
    assert_eq!(find(&v, &accepted)["duplicate"], false);
}

#[tokio::test]
async fn accept_or_reject_a_pending_candidature_in_open_or_locked() {
    let (state, router) = setup("accept").await;
    for event_status in ["open", "locked"] {
        for (decision, expected) in [("accepted", "accepted"), ("rejected", "rejected")] {
            let ctx = format!("{event_status} {decision}");
            let e = create(&state, &router, "all").await;
            let first = candidate(&state, &router, &e, "Kaelthorn", false).await;
            let c = candidate(&state, &router, &e, "Nyraelle", false).await;
            set_event_status(&state, &e, event_status).await;
            let (_, before, _) = read(&router, &e).await;
            let (res, text) = decide(&router, &e, &c.id, decision).await;
            assert_eq!(res.status(), StatusCode::OK, "{ctx}: {text}");
            assert_eq!(header(&res, "cache-control"), "no-store");
            let v: Value = serde_json::from_str(&text).unwrap();
            assert_eq!(find(&v, &c)["status"], expected, "{ctx}");
            // La réponse est l'agrégat à jour, sans rien réordonner.
            assert_eq!(ids(&v), ids(&before), "{ctx}");
            assert_eq!(find(&v, &first)["status"], "pending", "{ctx}");
            assert_eq!(
                stored(&state, &c).await,
                (expected.to_owned(), NOW.to_owned()),
                "{ctx}"
            );
            assert_eq!(
                stored(&state, &first).await,
                ("pending".to_owned(), BEFORE.to_owned()),
                "{ctx}"
            );
        }
    }
}

#[tokio::test]
async fn accepted_and_rejected_toggle_both_ways_in_open_or_locked() {
    let (state, router) = setup("toggle").await;
    let e = create(&state, &router, "all").await;
    let c = candidate(&state, &router, &e, "Nyraelle", true).await;
    for event_status in ["open", "locked"] {
        set_event_status(&state, &e, event_status).await;
        for (decision, at) in [("rejected", "accepted"), ("accepted", "rejected")] {
            let ctx = format!("{event_status} {at} -> {decision}");
            set_status(&state, &c, at).await;
            sql(
                &state,
                "UPDATE candidatures SET updated_at = ? WHERE id = ?",
                &[BEFORE, &c.id],
            )
            .await;
            let (res, text) = decide(&router, &e, &c.id, decision).await;
            assert_eq!(res.status(), StatusCode::OK, "{ctx}: {text}");
            let v: Value = serde_json::from_str(&text).unwrap();
            assert_eq!(find(&v, &c)["status"], decision, "{ctx}");
            assert_eq!(v["event"]["status"], event_status, "{ctx}");
            assert_eq!(
                stored(&state, &c).await,
                (decision.to_owned(), NOW.to_owned()),
                "{ctx}"
            );
        }
    }
}

#[tokio::test]
async fn availabilities_are_sorted_by_slot_start_not_by_slot_id() {
    let (state, router) = setup("slot_order").await;
    let e = create(&state, &router, "all").await;
    let c = candidate(&state, &router, &e, "Nyraelle", false).await;
    // Créneau plus tôt que les deux autres, dont l'id se trie après les leurs.
    let early = "z".repeat(22);
    assert!(e.slots.iter().all(|s| *s < early));
    sql(
        &state,
        "INSERT INTO slots (id, event_id, starts_at, duration_min) VALUES (?, ?, ?, 60)",
        &[&early, &e.id, "2026-11-10T20:00:00Z"],
    )
    .await;
    sql(
        &state,
        "INSERT INTO availabilities (candidature_id, slot_id, preferred) VALUES (?, ?, 0)",
        &[&c.id, &early],
    )
    .await;
    let (status, v, body) = read(&router, &e).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        find(&v, &c)["availabilities"],
        json!([
            { "slotId": early, "preferred": false },
            { "slotId": e.slots[0], "preferred": true },
            { "slotId": e.slots[1], "preferred": true },
        ])
    );
}

#[tokio::test]
async fn the_same_status_again_writes_nothing_and_returns_the_view() {
    let (state, router) = setup("same").await;
    let e = create(&state, &router, "all").await;
    let c = candidate(&state, &router, &e, "Nyraelle", true).await;
    for status in ["accepted", "rejected"] {
        set_status(&state, &c, status).await;
        let (res, text) = decide(&router, &e, &c.id, status).await;
        assert_eq!(res.status(), StatusCode::OK, "{status}: {text}");
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(find(&v, &c)["status"], status);
        assert_eq!(
            stored(&state, &c).await,
            (status.to_owned(), BEFORE.to_owned())
        );
    }
}

#[tokio::test]
async fn a_withdrawn_candidature_never_moves() {
    let (state, router) = setup("withdrawn").await;
    let e = create(&state, &router, "all").await;
    let c = candidate(&state, &router, &e, "Nyraelle", false).await;
    set_status(&state, &c, "withdrawn").await;
    for event_status in ["open", "locked"] {
        set_event_status(&state, &e, event_status).await;
        for decision in ["accepted", "rejected"] {
            let before = all_rows(&state).await;
            let (res, text) = decide(&router, &e, &c.id, decision).await;
            assert_problem(
                &res,
                &text,
                "candidature.transition_forbidden",
                &format!("{event_status} {decision}"),
            );
            assert_eq!(all_rows(&state).await, before);
        }
    }
}

#[tokio::test]
async fn a_cancelled_event_refuses_every_decision_first() {
    let (state, router) = setup("cancelled").await;
    let e = create(&state, &router, "all").await;
    let c = candidate(&state, &router, &e, "Nyraelle", false).await;
    set_event_status(&state, &e, "cancelled").await;
    // L'annulation passe avant le statut de la candidature.
    for status in ["pending", "accepted", "withdrawn"] {
        set_status(&state, &c, status).await;
        let before = all_rows(&state).await;
        for decision in ["accepted", "rejected"] {
            let (res, text) = decide(&router, &e, &c.id, decision).await;
            assert_problem(&res, &text, "event.closed", &format!("{status} {decision}"));
        }
        assert_eq!(all_rows(&state).await, before, "{status}");
    }
    // La lecture reste possible.
    let (status, v, _) = read(&router, &e).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["event"]["status"], "cancelled");
}

#[tokio::test]
async fn a_candidature_of_another_event_or_unknown_is_not_found() {
    let (state, router) = setup("foreign").await;
    let e = create(&state, &router, "all").await;
    let other = create(&state, &router, "all").await;
    let mine = candidate(&state, &router, &e, "Nyraelle", false).await;
    let theirs = candidate(&state, &router, &other, "Kaelthorn", false).await;
    let before = all_rows(&state).await;
    for id in [theirs.id.as_str(), &"x".repeat(22), "", "short"] {
        let (res, text) = decide(&router, &e, id, "accepted").await;
        assert_problem(&res, &text, "candidature.not_found", id);
    }
    assert_eq!(all_rows(&state).await, before);
    // Le bon lien sur sa propre candidature passe.
    let (res, text) = decide(&router, &e, &mine.id, "accepted").await;
    assert_eq!(res.status(), StatusCode::OK, "{text}");
}

#[tokio::test]
async fn an_invalid_status_or_body_is_malformed() {
    let (state, router) = setup("malformed").await;
    let e = create(&state, &router, "all").await;
    let c = candidate(&state, &router, &e, "Nyraelle", false).await;
    let before = all_rows(&state).await;
    for status in ["withdrawn", "pending", "ACCEPTED", "accept", ""] {
        let (res, text) = decide(&router, &e, &c.id, status).await;
        assert_problem(&res, &text, "form.malformed", status);
    }
    for raw in [
        "{".to_owned(),
        "{}".to_owned(),
        json!({ "candidatureId": c.id }).to_string(),
        json!({ "status": "accepted" }).to_string(),
        json!({ "candidatureId": 3, "status": "accepted" }).to_string(),
    ] {
        let (res, text) = post_raw_with(&router, DECIDE, &raw, &organizer(&e)).await;
        assert_problem(&res, &text, "form.malformed", &raw);
    }
    assert_eq!(all_rows(&state).await, before);
}

#[tokio::test]
async fn an_accepted_candidate_reads_accepted_with_the_discord_link() {
    let (state, router) = setup("acceptance").await;
    let e = create_with(
        &state,
        &router,
        "all",
        json!({ "discordUrl": "https://discord.gg/abcdef" }),
    )
    .await;
    let c = candidate(&state, &router, &e, "Nyraelle", false).await;
    let participant = [("x-link-secret", c.secret.as_str())];
    let (res, text) = send_with(&router, Method::GET, "/api/participant", &participant).await;
    assert_eq!(res.status(), StatusCode::OK, "{text}");
    let v: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["status"], "pending");
    assert!(v.get("discordUrl").is_none(), "{text}");

    let (res, text) = decide(&router, &e, &c.id, "accepted").await;
    assert_eq!(res.status(), StatusCode::OK, "{text}");

    let (res, text) = send_with(&router, Method::GET, "/api/participant", &participant).await;
    assert_eq!(res.status(), StatusCode::OK, "{text}");
    let v: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["status"], "accepted");
    assert_eq!(v["discordUrl"], "https://discord.gg/abcdef");
}

#[tokio::test]
async fn the_routes_use_the_read_and_write_quotas() {
    let (state, router) = setup_with(
        "quota",
        &test_config(&[
            ("RATE_CREATE", "100/h"),
            ("RATE_CANDIDATURE", "100/h"),
            ("RATE_READ", "1/h"),
            ("RATE_WRITE", "1/h"),
        ]),
    )
    .await;
    let e = create(&state, &router, "all").await;
    let c = candidate(&state, &router, &e, "Nyraelle", false).await;
    let (res, text) = send_with(&router, Method::GET, VIEW, &organizer(&e)).await;
    assert_eq!(res.status(), StatusCode::OK, "{text}");
    let (res, text) = send_with(&router, Method::GET, VIEW, &organizer(&e)).await;
    assert_problem(&res, &text, "limit.rate_limited", "read");
    // La lecture épuisée ne touche pas la classe `write`.
    let (res, text) = decide(&router, &e, &c.id, "accepted").await;
    assert_eq!(res.status(), StatusCode::OK, "{text}");
    let (res, text) = decide(&router, &e, &c.id, "rejected").await;
    assert_problem(&res, &text, "limit.rate_limited", "write");
    assert_eq!(stored(&state, &c).await.0, "accepted");
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
    let c = candidate(&state, &router, &e, "Nyraelle", false).await;
    let requests = |decision: &'static str| {
        let (router, e, id) = (&router, &e, c.id.clone());
        async move {
            let (status, _, _) = read(router, e).await;
            assert_eq!(status, StatusCode::OK);
            let (res, _) = decide(router, e, &id, decision).await;
            assert_eq!(res.status(), StatusCode::OK);
            let (res, _) = decide(router, e, &"x".repeat(22), decision).await;
            assert_eq!(res.status(), StatusCode::NOT_FOUND);
            let (res, _) = post_raw_with(router, DECIDE, "{", &organizer(e)).await;
            assert_eq!(res.status(), StatusCode::BAD_REQUEST);
        }
    };
    // Même gabarit que `participant::no_secret_nor_pseudo_in_the_logs`.
    requests("accepted").await;
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
    requests("rejected").await;
    drop(guard);
    let log = String::from_utf8(captured.0.lock().unwrap().clone()).unwrap();
    assert!(log.contains("candidature status set"), "{log}");
    assert!(log.contains(&c.id), "{log}");
    for leak in [
        e.organizer.as_str(),
        c.secret.as_str(),
        "Nyraelle",
        "Nyraelle[Israphel]",
        TEST_CLIENT_IP,
    ] {
        assert!(!log.contains(leak), "{leak} in {log}");
    }
}
