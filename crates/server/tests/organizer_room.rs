//! Matrice HTTP de la story 3-4 : `regenerate_room_name` sous le seul secret organisateur. Nouveau
//! nom toujours différent et unique (collisions par un générateur de codes scripté, le nom actuel
//! compte comme pris), groupe inconnu, d'un autre événement ou en sommeil refusé, état `locked`
//! seulement, quota `write`, journaux sans nom de salle ni secret. Valeurs fictives.

mod common;

use std::sync::{Arc, Mutex};

use axum::http::{Method, StatusCode};
use common::candidatures::*;
use common::*;
use serde_json::{Value, json};
use server::app::AppState;
use server::config::Config;
use server::infra::clock::FixedClock;
use server::infra::ids::{DeterministicIdGenerator, IdGenerator};
use server::infra::reference::ReferenceStore;

const VIEW: &str = "/api/organizer";
const LOCK: &str = "/api/organizer/lock";
const UNLOCK: &str = "/api/organizer/unlock";
const ROOM_NAME: &str = "/api/organizer/room-name";

fn config(overrides: &[(&str, &str)]) -> Config {
    let mut all = vec![
        ("RATE_CREATE", "100/h"),
        ("RATE_CANDIDATURE", "1000/h"),
        ("RATE_WRITE", "1000/h"),
    ];
    all.extend_from_slice(overrides);
    test_config(&all)
}

async fn setup_full(
    name: &str,
    config: &Config,
    ids: impl IdGenerator + 'static,
) -> (AppState, axum::Router) {
    let state = AppState::new(
        memory_db(&format!("organizer_room_{name}")).await,
        Arc::new(FixedClock::new(NOW.parse().unwrap())),
        Arc::new(ids),
        ReferenceStore::with(reference_set()),
        config,
    );
    let router = fixture_router(state.clone());
    (state, router)
}

async fn setup(name: &str) -> (AppState, axum::Router) {
    setup_full(name, &config(&[]), DeterministicIdGenerator::new()).await
}

fn organizer(e: &Event) -> [(&'static str, &str); 1] {
    [("x-link-secret", e.organizer.as_str())]
}

async fn post(
    router: &axum::Router,
    e: &Event,
    uri: &str,
    body: Value,
) -> (axum::http::Response<axum::body::Body>, String) {
    post_json_with(router, uri, &body, &organizer(e)).await
}

/// 200 et la vue organisateur rendue.
fn ok(res: &axum::http::Response<axum::body::Body>, text: &str, ctx: &str) -> Value {
    assert_eq!(res.status(), StatusCode::OK, "{ctx}: {text}");
    serde_json::from_str(text).unwrap()
}

async fn lock(router: &axum::Router, e: &Event, slots: &[&str]) -> Value {
    let (res, text) = post(router, e, LOCK, json!({ "slotIds": slots })).await;
    ok(&res, &text, "lock")
}

async fn regenerate(router: &axum::Router, e: &Event, group: &str) -> Value {
    let (res, text) = post(router, e, ROOM_NAME, json!({ "groupId": group })).await;
    ok(&res, &text, "room-name")
}

/// Id du groupe retenu `n` (0 = G1) de la vue.
fn group(v: &Value, n: usize) -> String {
    v["groups"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|g| g["retained"] == true)
        .nth(n)
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// Nom de salle du groupe `id` dans la vue.
fn room_of(v: &Value, id: &str) -> String {
    v["groups"]
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["id"] == id)
        .unwrap()["roomName"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// Tous les noms de salle en base, tous événements confondus, triés.
async fn all_names(state: &AppState) -> Vec<String> {
    let rows: Vec<(String,)> = sqlx::query_as("SELECT room_name FROM groups ORDER BY room_name")
        .fetch_all(&state.db.read)
        .await
        .unwrap();
    rows.into_iter().map(|(n,)| n).collect()
}

/// Toutes les lignes de l'agrégat : rien ne doit changer sur un refus.
async fn dump(state: &AppState) -> Vec<String> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT 'e|' || id || '|' || status FROM events \
         UNION ALL SELECT 'g|' || id || '|' || slot_id || '|' || occurrence || '|' || retained \
         || '|' || parties || '|' || party_size || '|' || room_label || '|' || room_name \
         FROM groups \
         UNION ALL SELECT 'p|' || candidature_id || '|' || group_id FROM assignments ORDER BY 1",
    )
    .fetch_all(&state.db.read)
    .await
    .unwrap();
    rows.into_iter().map(|(r,)| r).collect()
}

fn is_ludra_name(name: &str) -> bool {
    name.strip_prefix("LUDRA-")
        .is_some_and(domain::room::is_room_code)
}

#[tokio::test]
async fn a_retained_group_gets_a_new_unique_name_and_nothing_else_changes() {
    let (state, router) = setup("nominal").await;
    let e = create(&state, &router, "all").await;
    let c = candidate(&state, &router, &e, "Nyraelle", true).await;
    let v = lock(&router, &e, &[&e.slots[0], &e.slots[1]]).await;
    let (g1, g2) = (group(&v, 0), group(&v, 1));
    let (old1, old2) = (room_of(&v, &g1), room_of(&v, &g2));
    // Un placement existant n'est pas touché.
    sql(
        &state,
        "INSERT INTO assignments (event_id, candidature_id, group_id, party, party_index, \
         bench_index) VALUES (?, ?, ?, 1, 1, NULL)",
        &[&e.id, &c.id, &g1],
    )
    .await;

    let v = regenerate(&router, &e, &g1).await;
    let new1 = room_of(&v, &g1);
    assert_ne!(new1, old1);
    assert!(is_ludra_name(&new1), "{new1}");
    assert_eq!(room_of(&v, &g2), old2);
    assert_eq!(v["event"]["status"], "locked");
    assert_eq!(v["groups"][0]["starters"][0]["candidatureId"], c.id);
    let names = all_names(&state).await;
    assert_eq!(names.len(), 2);
    assert!(names.contains(&new1) && names.contains(&old2));
    // La vue relue porte le nouveau nom.
    let (res, text) = send_with(&router, Method::GET, VIEW, &organizer(&e)).await;
    assert_eq!(room_of(&ok(&res, &text, "read"), &g1), new1);
}

#[tokio::test]
async fn the_current_name_and_a_taken_name_draw_a_new_code() {
    // G1 = AAAA, G2 = BBBB ; puis, pour G1 : AAAA (le nom actuel), BBBB (pris par G2), CCCC.
    let (state, router) = setup_full(
        "collision",
        &config(&[]),
        Scripted::with(&["AAAA", "BBBB", "AAAA", "BBBB", "CCCC"]),
    )
    .await;
    let e = create(&state, &router, "all").await;
    let v = lock(&router, &e, &[&e.slots[0], &e.slots[1]]).await;
    let g1 = group(&v, 0);
    assert_eq!(room_of(&v, &g1), "LUDRA-AAAA");
    let v = regenerate(&router, &e, &g1).await;
    assert_eq!(room_of(&v, &g1), "LUDRA-CCCC");
    assert_eq!(all_names(&state).await, ["LUDRA-BBBB", "LUDRA-CCCC"]);
}

#[tokio::test]
async fn five_collisions_are_a_500_that_writes_nothing() {
    // G1 = AAAA, G2 = BBBB ; puis cinq essais tous pris (le nom actuel ou celui de G2).
    let (state, router) = setup_full(
        "collision_5",
        &config(&[]),
        Scripted::with(&[
            "AAAA", "BBBB", "AAAA", "BBBB", "AAAA", "BBBB", "AAAA", "DDDD",
        ]),
    )
    .await;
    let e = create(&state, &router, "all").await;
    let v = lock(&router, &e, &[&e.slots[0], &e.slots[1]]).await;
    let g1 = group(&v, 0);
    let before = dump(&state).await;
    let (res, text) = post(&router, &e, ROOM_NAME, json!({ "groupId": g1 })).await;
    assert_problem(&res, &text, "server.internal", "5 collisions");
    assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(dump(&state).await, before);
}

#[tokio::test]
async fn four_collisions_then_a_free_code_pass_on_the_fifth_attempt() {
    let (state, router) = setup_full(
        "collision_4",
        &config(&[]),
        Scripted::with(&["AAAA", "BBBB", "AAAA", "BBBB", "AAAA", "BBBB", "EEEE"]),
    )
    .await;
    let e = create(&state, &router, "all").await;
    let v = lock(&router, &e, &[&e.slots[0], &e.slots[1]]).await;
    let g1 = group(&v, 0);
    let v = regenerate(&router, &e, &g1).await;
    assert_eq!(room_of(&v, &g1), "LUDRA-EEEE");
}

#[tokio::test]
async fn the_new_name_keeps_the_frozen_room_label_after_the_reference_changes() {
    let (state, router) = setup("frozen_label").await;
    let e = create(&state, &router, "all").await;
    let v = lock(&router, &e, &[&e.slots[0]]).await;
    let g = group(&v, 0);
    // Même idée que l'instance `retired` d'`organizer_lock` : la référence change après le
    // verrouillage, ici le libellé de salle de l'instance de l'événement.
    let mut set = reference_set();
    set.instances
        .iter_mut()
        .find(|i| i.id == "ludra")
        .unwrap()
        .room_label = "NEWLBL".into();
    state.reference.replace(Arc::new(set));
    let v = regenerate(&router, &e, &g).await;
    let name = room_of(&v, &g);
    assert!(is_ludra_name(&name), "{name}");
    let (label,): (String,) = sqlx::query_as("SELECT room_label FROM groups WHERE id = ?")
        .bind(&g)
        .fetch_one(&state.db.read)
        .await
        .unwrap();
    assert_eq!(label, "LUDRA");
}

#[tokio::test]
async fn an_unknown_foreign_or_sleeping_group_is_a_400_that_writes_nothing() {
    let (state, router) = setup("invalid_group").await;
    let e = create(&state, &router, "all").await;
    let f = create(&state, &router, "all").await;
    let a = e.slots[0].as_str();
    let v = lock(&router, &e, &[a, a]).await;
    let sleeping = group(&v, 1);
    let (res, text) = post(&router, &e, UNLOCK, json!({})).await;
    ok(&res, &text, "unlock");
    lock(&router, &e, &[a]).await;
    let foreign = group(&lock(&router, &f, &[&f.slots[0]]).await, 0);
    let before = dump(&state).await;
    for (id, ctx) in [
        ("unknown-group", "unknown"),
        (foreign.as_str(), "other event"),
        (sleeping.as_str(), "sleeping"),
        ("", "empty"),
    ] {
        let (res, text) = post(&router, &e, ROOM_NAME, json!({ "groupId": id })).await;
        assert_fields(
            &res,
            &text,
            "placement.unknown_group",
            json!({ "groupId": "placement.unknown_group" }),
        );
        assert_eq!(res.status(), StatusCode::BAD_REQUEST, "{ctx}");
    }
    assert_eq!(dump(&state).await, before);
}

#[tokio::test]
async fn a_malformed_body_is_refused() {
    let (state, router) = setup("malformed").await;
    let e = create(&state, &router, "all").await;
    lock(&router, &e, &[&e.slots[0]]).await;
    let before = dump(&state).await;
    for body in [json!({}), json!({ "groupId": 3 })] {
        let (res, text) = post(&router, &e, ROOM_NAME, body).await;
        assert_problem(&res, &text, "form.malformed", "malformed");
    }
    assert_eq!(dump(&state).await, before);
}

#[tokio::test]
async fn only_a_locked_event_regenerates_even_once_a_slot_has_started() {
    let (state, router) = setup("states").await;
    let e = create(&state, &router, "all").await;
    let a = e.slots[0].as_str();
    let g = group(&lock(&router, &e, &[a]).await, 0);
    // Créneau commencé : pas de restriction.
    sql(
        &state,
        "UPDATE slots SET starts_at = ? WHERE id = ?",
        &["2026-11-06T09:00:00Z", a],
    )
    .await;
    regenerate(&router, &e, &g).await;
    for (status, code) in [("open", "event.not_locked"), ("cancelled", "event.closed")] {
        set_event_status(&state, &e, status).await;
        let before = dump(&state).await;
        // L'état passe avant le groupe.
        for id in [g.as_str(), "unknown-group"] {
            let (res, text) = post(&router, &e, ROOM_NAME, json!({ "groupId": id })).await;
            assert_problem(&res, &text, code, status);
            assert_eq!(res.status(), StatusCode::CONFLICT);
        }
        assert_eq!(dump(&state).await, before, "{status}");
    }
}

#[tokio::test]
async fn only_the_organizer_secret_regenerates() {
    let (state, router) = setup("rights").await;
    let e = create(&state, &router, "all").await;
    let c = candidate(&state, &router, &e, "Una", true).await;
    let g = group(&lock(&router, &e, &[&e.slots[0]]).await, 0);
    let unknown = "A".repeat(43);
    let before = dump(&state).await;
    let body = json!({ "groupId": g });
    for (headers, code) in [
        (vec![], "auth.missing"),
        (vec![("x-link-secret", unknown.as_str())], "auth.not_found"),
        (vec![("x-link-secret", e.join.as_str())], "auth.wrong_right"),
        (
            vec![("x-link-secret", c.secret.as_str())],
            "auth.wrong_right",
        ),
    ] {
        let (res, text) = post_json_with(&router, ROOM_NAME, &body, &headers).await;
        assert_problem(&res, &text, code, code);
    }
    assert_eq!(dump(&state).await, before);
    let (res, text) = send_with(&router, Method::GET, ROOM_NAME, &organizer(&e)).await;
    assert_problem(&res, &text, "server.method_not_allowed", ROOM_NAME);
}

#[tokio::test]
async fn the_regeneration_uses_the_write_quota() {
    let (state, router) = setup_full(
        "quota",
        &config(&[("RATE_WRITE", "2/h")]),
        DeterministicIdGenerator::new(),
    )
    .await;
    let e = create(&state, &router, "all").await;
    let g = group(&lock(&router, &e, &[&e.slots[0]]).await, 0);
    regenerate(&router, &e, &g).await;
    let before = dump(&state).await;
    let (res, text) = post(&router, &e, ROOM_NAME, json!({ "groupId": g })).await;
    assert_problem(&res, &text, "limit.rate_limited", "quota");
    assert_eq!(dump(&state).await, before);
    let (res, _) = send_with(&router, Method::GET, VIEW, &organizer(&e)).await;
    assert_eq!(res.status(), StatusCode::OK);
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
async fn no_secret_nor_room_name_in_the_logs() {
    // G1 = HJ4R, G2 = XYZW ; premier passage : K7Q2 ; second : M3NP ; puis cinq collisions.
    let (state, router) = setup_full(
        "logs",
        &config(&[]),
        Scripted::with(&[
            "HJ4R", "XYZW", "K7Q2", "M3NP", "XYZW", "XYZW", "XYZW", "XYZW", "XYZW",
        ]),
    )
    .await;
    let e = create(&state, &router, "all").await;
    let c = candidate(&state, &router, &e, "Nyraelle", true).await;
    let v = lock(&router, &e, &[&e.slots[0], &e.slots[1]]).await;
    let g1 = group(&v, 0);
    let requests = || {
        let (router, e, g1) = (&router, &e, &g1);
        async move {
            regenerate(router, e, g1).await;
        }
    };
    // Même gabarit que `organizer_lock::no_secret_nor_room_name_in_the_logs` : premier passage
    // sous un abonné muet, pour enregistrer chaque callsite avant le `set_default`.
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
    let guard = tracing::subscriber::set_default(subscriber);
    tracing::callsite::rebuild_interest_cache();
    // Un nouveau nom (`M3NP`), puis un échec sur cinq collisions contre G2.
    requests().await;
    let (res, _) = post(&router, &e, ROOM_NAME, json!({ "groupId": g1 })).await;
    assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
    drop(guard);
    let log = String::from_utf8(captured.0.lock().unwrap().clone()).unwrap();
    for line in ["room name regenerated", "room name collisions"] {
        assert!(log.contains(line), "{line} missing in {log}");
    }
    assert!(log.contains(&e.id), "{log}");
    for leak in [
        e.organizer.as_str(),
        e.join.as_str(),
        c.secret.as_str(),
        "Nyraelle",
        "HJ4R",
        "XYZW",
        "K7Q2",
        "M3NP",
        "LUDRA-",
        TEST_CLIENT_IP,
    ] {
        assert!(!log.contains(leak), "{leak} in {log}");
    }
}
