//! Matrice HTTP de la story 3-2 : verrouiller et déverrouiller sous le seul secret organisateur.
//! Groupes appariés par `(créneau, occurrence)`, disposition figée, noms de salle uniques (collision
//! par un générateur de codes scripté), groupes en sommeil au re-verrouillage, cascade à la
//! suppression d'un créneau, placement retiré au retrait et au refus, quota `write`, journaux muets.
//! Valeurs fictives.

mod common;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use axum::http::{Method, StatusCode};
use common::candidatures::*;
use common::*;
use domain::reference::ReferenceSet;
use serde_json::{Value, json};
use server::app::AppState;
use server::config::Config;
use server::infra::clock::FixedClock;
use server::infra::ids::{DeterministicIdGenerator, IdGenerator, NONCE_BYTES, PublicId, Secret};
use server::infra::reference::ReferenceStore;

const VIEW: &str = "/api/organizer";
const LOCK: &str = "/api/organizer/lock";
const UNLOCK: &str = "/api/organizer/unlock";
const REMOVAL: &str = "/api/organizer/slot-removal";
const STATUS: &str = "/api/organizer/candidature-status";

/// Générateur déterministe dont les codes de salle sortent d'abord d'une liste scriptée.
#[derive(Default)]
struct Scripted {
    inner: DeterministicIdGenerator,
    codes: Mutex<VecDeque<&'static str>>,
}

impl Scripted {
    fn with(codes: &[&'static str]) -> Self {
        Self {
            inner: DeterministicIdGenerator::new(),
            codes: Mutex::new(codes.iter().copied().collect()),
        }
    }
}

impl IdGenerator for Scripted {
    fn public_id(&self) -> PublicId {
        self.inner.public_id()
    }

    fn secret(&self) -> Secret {
        self.inner.secret()
    }

    fn nonce(&self) -> [u8; NONCE_BYTES] {
        self.inner.nonce()
    }

    fn room_code(&self) -> String {
        match self.codes.lock().unwrap().pop_front() {
            Some(code) => code.to_owned(),
            None => self.inner.room_code(),
        }
    }
}

/// Référence des tests de candidature, plus une instance retirée (`retired`, 1 × 4, `OLD`).
fn test_reference() -> ReferenceSet {
    let mut set = reference_set();
    let mut retired = set
        .instances
        .iter()
        .find(|i| i.id == "ludra")
        .unwrap()
        .clone();
    retired.id = "retired".into();
    retired.parties = 1;
    retired.party_size = 4;
    retired.room_label = "OLD".into();
    retired.enabled = false;
    set.instances.push(retired);
    set
}

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
        memory_db(&format!("organizer_lock_{name}")).await,
        Arc::new(FixedClock::new(NOW.parse().unwrap())),
        Arc::new(ids),
        ReferenceStore::with(test_reference()),
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

async fn unlock(router: &axum::Router, e: &Event) -> Value {
    let (res, text) = post(router, e, UNLOCK, json!({})).await;
    ok(&res, &text, "unlock")
}

/// Ligne `groups` : `slot_id, occurrence, retained, parties, party_size, room_label, room_name`.
type GroupRow = (String, i64, i64, i64, i64, String, String);

/// Groupes de l'événement, par créneau puis occurrence.
async fn groups(state: &AppState, e: &Event) -> Vec<GroupRow> {
    sqlx::query_as(
        "SELECT g.slot_id, g.occurrence, g.retained, g.parties, g.party_size, g.room_label, \
         g.room_name FROM groups g JOIN slots s ON s.id = g.slot_id WHERE g.event_id = ? \
         ORDER BY s.starts_at, g.occurrence",
    )
    .bind(&e.id)
    .fetch_all(&state.db.read)
    .await
    .unwrap()
}

async fn group_id(state: &AppState, e: &Event, slot: &str, occurrence: i64) -> String {
    let (id,): (String,) = sqlx::query_as(
        "SELECT id FROM groups WHERE event_id = ? AND slot_id = ? AND occurrence = ?",
    )
    .bind(&e.id)
    .bind(slot)
    .bind(occurrence)
    .fetch_one(&state.db.read)
    .await
    .unwrap();
    id
}

async fn event_status(state: &AppState, e: &Event) -> String {
    let (s,): (String,) = sqlx::query_as("SELECT status FROM events WHERE id = ?")
        .bind(&e.id)
        .fetch_one(&state.db.read)
        .await
        .unwrap();
    s
}

/// Placement direct en base : `apply_placement` arrive avec la story 3-3. L'événement est celui du
/// groupe.
async fn place(state: &AppState, c: &Cand, group: &str) {
    sql(
        state,
        "INSERT INTO assignments (event_id, candidature_id, group_id, party, party_index, \
         bench_index) VALUES ((SELECT event_id FROM groups WHERE id = ?2), ?1, ?2, 1, 1, NULL)",
        &[&c.id, group],
    )
    .await;
}

async fn assignments(state: &AppState) -> Vec<(String, String)> {
    sqlx::query_as("SELECT candidature_id, group_id FROM assignments ORDER BY candidature_id")
        .fetch_all(&state.db.read)
        .await
        .unwrap()
}

/// Toutes les lignes de l'agrégat, groupes et placements compris : rien ne doit changer sur un
/// refus.
async fn dump(state: &AppState) -> Vec<String> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT 'e|' || id || '|' || status || '|' || coalesce(cancelled_at, '-') FROM events \
         UNION ALL SELECT 's|' || id || '|' || starts_at FROM slots \
         UNION ALL SELECT 'c|' || id || '|' || status || '|' || updated_at FROM candidatures \
         UNION ALL SELECT 'g|' || id || '|' || slot_id || '|' || occurrence || '|' || retained \
         || '|' || parties || '|' || party_size || '|' || room_label || '|' || room_name || '|' \
         || created_at FROM groups \
         UNION ALL SELECT 'p|' || candidature_id || '|' || group_id FROM assignments ORDER BY 1",
    )
    .fetch_all(&state.db.read)
    .await
    .unwrap();
    rows.into_iter().map(|(r,)| r).collect()
}

fn names(rows: &[GroupRow]) -> Vec<String> {
    rows.iter().map(|g| g.6.clone()).collect()
}

fn is_ludra_name(name: &str) -> bool {
    name.strip_prefix("LUDRA-")
        .is_some_and(domain::room::is_room_code)
}

#[tokio::test]
async fn lock_creates_one_group_per_entry_with_a_frozen_layout_and_a_room_name() {
    let (state, router) = setup("lock").await;
    let e = create(&state, &router, "all").await;
    let accepted = candidate(&state, &router, &e, "Nyraelle", true).await;
    let pending = candidate(&state, &router, &e, "Kaelen", false).await;
    let (a, b) = (e.slots[0].as_str(), e.slots[1].as_str());

    let v = lock(&router, &e, &[a, a, b]).await;
    assert_eq!(v["event"]["status"], "locked");
    let rows = groups(&state, &e).await;
    assert_eq!(
        rows.iter()
            .map(|g| (g.0.as_str(), g.1, g.2, g.3, g.4, g.5.as_str()))
            .collect::<Vec<_>>(),
        [
            (a, 1, 1, 2, 5, "LUDRA"),
            (a, 2, 1, 2, 5, "LUDRA"),
            (b, 1, 1, 2, 5, "LUDRA"),
        ]
    );
    let all = names(&rows);
    assert!(all.iter().all(|n| is_ludra_name(n)), "{all:?}");
    assert_eq!(
        all.iter().collect::<std::collections::HashSet<_>>().len(),
        3
    );
    // Le DTO : tous les groupes, triés par créneau puis occurrence, et `hasGroups` par créneau.
    let dto = v["groups"].as_array().unwrap();
    assert_eq!(dto.len(), 3);
    let mut keys: Vec<_> = dto[0].as_object().unwrap().keys().cloned().collect();
    keys.sort();
    assert_eq!(
        keys,
        [
            "id",
            "occurrence",
            "parties",
            "partySize",
            "retained",
            "roomLabel",
            "roomName",
            "slotId"
        ]
    );
    for (g, row) in dto.iter().zip(&rows) {
        assert_eq!(g["slotId"], row.0.as_str());
        assert_eq!(g["occurrence"], row.1);
        assert_eq!(g["retained"], true);
        assert_eq!(
            (g["parties"].clone(), g["partySize"].clone()),
            (json!(2), json!(5))
        );
        assert_eq!(g["roomLabel"], "LUDRA");
        assert_eq!(g["roomName"], row.6.as_str());
        assert_eq!(g["id"].as_str().unwrap().len(), 22);
    }
    assert_eq!(v["slots"][0]["hasGroups"], true);
    assert_eq!(v["slots"][1]["hasGroups"], true);
    // Les statuts des candidatures ne changent pas.
    let statuses: Vec<(String, String)> =
        sqlx::query_as("SELECT id, status FROM candidatures ORDER BY created_at, id")
            .fetch_all(&state.db.read)
            .await
            .unwrap();
    assert_eq!(
        statuses,
        [
            (accepted.id.clone(), "accepted".to_owned()),
            (pending.id.clone(), "pending".to_owned())
        ]
    );
    // La relecture rend la même vue.
    let (res, text) = send_with(&router, Method::GET, VIEW, &organizer(&e)).await;
    assert_eq!(ok(&res, &text, "read")["groups"], v["groups"]);
}

#[tokio::test]
async fn groups_come_back_by_slot_start_then_occurrence_whatever_the_list_order() {
    let (state, router) = setup("order").await;
    let e = create(&state, &router, "all").await;
    let (a, b) = (e.slots[0].as_str(), e.slots[1].as_str());
    let v = lock(&router, &e, &[a, b, a]).await;
    let order: Vec<(String, i64)> = v["groups"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| {
            (
                g["slotId"].as_str().unwrap().to_owned(),
                g["occurrence"].as_i64().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        order,
        [(a.to_owned(), 1), (a.to_owned(), 2), (b.to_owned(), 1)]
    );
}

#[tokio::test]
async fn the_database_refuses_a_row_that_mixes_two_events() {
    let (state, router) = setup("cross_event").await;
    let e = create(&state, &router, "all").await;
    let f = create(&state, &router, "all").await;
    let ce = candidate(&state, &router, &e, "Nyraelle", true).await;
    let cf = candidate(&state, &router, &f, "Kaelen", true).await;
    lock(&router, &e, &[&e.slots[0]]).await;
    lock(&router, &f, &[&f.slots[0]]).await;
    let (ge, gf) = (
        group_id(&state, &e, &e.slots[0], 1).await,
        group_id(&state, &f, &f.slots[0], 1).await,
    );
    let try_sql = |query: &'static str, binds: Vec<String>| {
        let state = state.clone();
        async move {
            let mut q = sqlx::query(query);
            for b in binds {
                q = q.bind(b);
            }
            q.execute(&state.db.write).await
        }
    };
    // Un groupe de `e` sur un créneau de `f`.
    let group = "INSERT INTO groups (id, event_id, slot_id, occurrence, retained, parties, \
                 party_size, room_label, room_name, created_at) \
                 VALUES (?, ?, ?, 9, 1, 2, 5, 'LUDRA', ?, ?)";
    assert!(
        try_sql(
            group,
            vec![
                "x".repeat(22),
                e.id.clone(),
                f.slots[0].clone(),
                "LUDRA-XXXX".into(),
                NOW.into()
            ]
        )
        .await
        .is_err()
    );
    let assign = "INSERT INTO assignments (event_id, candidature_id, group_id, party, \
                  party_index, bench_index) VALUES (?, ?, ?, NULL, NULL, 1)";
    // Candidature de `f` dans un groupe de `e`, puis candidature de `e` dans un groupe de `f`.
    for (event, cand, group) in [
        (&e.id, &cf.id, &ge),
        (&e.id, &ce.id, &gf),
        (&f.id, &ce.id, &gf),
    ] {
        assert!(
            try_sql(assign, vec![event.clone(), cand.clone(), group.clone()])
                .await
                .is_err(),
            "{event} {cand} {group}"
        );
    }
    // Le même événement partout : accepté.
    try_sql(assign, vec![e.id.clone(), ce.id.clone(), ge.clone()])
        .await
        .unwrap();
    assert_eq!(assignments(&state).await, [(ce.id.clone(), ge)]);
}

#[tokio::test]
async fn an_empty_event_and_a_single_slot_lock_too() {
    // Le serveur ne vérifie pas les disponibilités : un groupe sans personne est permis.
    let (state, router) = setup("empty").await;
    let e = create(&state, &router, "all").await;
    let v = lock(&router, &e, &[&e.slots[1]]).await;
    assert_eq!(v["groups"].as_array().unwrap().len(), 1);
    assert_eq!(v["slots"][0]["hasGroups"], false);
    assert_eq!(v["slots"][1]["hasGroups"], true);
}

#[tokio::test]
async fn a_taken_room_name_draws_a_new_code() {
    let (state, router) = setup_full(
        "collision",
        &config(&[]),
        Scripted::with(&["AAAA", "AAAA", "BBBB"]),
    )
    .await;
    let e = create(&state, &router, "all").await;
    lock(&router, &e, &[&e.slots[0], &e.slots[1]]).await;
    assert_eq!(
        names(&groups(&state, &e).await),
        ["LUDRA-AAAA", "LUDRA-BBBB"]
    );
}

#[tokio::test]
async fn five_collisions_are_a_500_that_writes_nothing() {
    let (state, router) = setup_full(
        "collision_5",
        &config(&[]),
        Scripted::with(&["AAAA", "AAAA", "AAAA", "AAAA", "AAAA", "AAAA"]),
    )
    .await;
    let e = create(&state, &router, "all").await;
    let before = dump(&state).await;
    let (res, text) = post(
        &router,
        &e,
        LOCK,
        json!({ "slotIds": [e.slots[0], e.slots[1]] }),
    )
    .await;
    assert_problem(&res, &text, "server.internal", "5 collisions");
    assert_eq!(dump(&state).await, before);
    // Quatre collisions puis un code libre : le cinquième essai passe.
    let (state, router) = setup_full(
        "collision_4",
        &config(&[]),
        Scripted::with(&["AAAA", "AAAA", "AAAA", "AAAA", "AAAA", "CCCC"]),
    )
    .await;
    let e = create(&state, &router, "all").await;
    lock(&router, &e, &[&e.slots[0], &e.slots[1]]).await;
    assert_eq!(
        names(&groups(&state, &e).await),
        ["LUDRA-AAAA", "LUDRA-CCCC"]
    );
}

#[tokio::test]
async fn the_room_name_follows_room_name_template() {
    let (state, router) = setup_full(
        "template",
        &config(&[("ROOM_NAME_TEMPLATE", "{code} {label}")]),
        DeterministicIdGenerator::new(),
    )
    .await;
    let e = create(&state, &router, "all").await;
    lock(&router, &e, &[&e.slots[0]]).await;
    assert_eq!(names(&groups(&state, &e).await), ["0000 LUDRA"]);
}

#[tokio::test]
async fn an_invalid_list_writes_nothing() {
    let (state, router) = setup("invalid").await;
    let e = create(&state, &router, "all").await;
    let other = create(&state, &router, "all").await;
    // Un créneau passé : le premier.
    sql(
        &state,
        "UPDATE slots SET starts_at = ? WHERE id = ?",
        &["2026-11-05T20:00:00Z", &e.slots[0]],
    )
    .await;
    let before = dump(&state).await;
    let (a, b) = (e.slots[0].as_str(), e.slots[1].as_str());
    for (body, code) in [
        (json!({ "slotIds": [] }), "event.group_count_out_of_range"),
        (
            json!({ "slotIds": vec![b; 15] }),
            "event.group_count_out_of_range",
        ),
        (json!({ "slotIds": [b, other.slots[0]] }), "slot.not_found"),
        (json!({ "slotIds": [b, "x".repeat(22)] }), "slot.not_found"),
        (json!({ "slotIds": [b, ""] }), "slot.not_found"),
    ] {
        let (res, text) = post(&router, &e, LOCK, body.clone()).await;
        assert_problem(&res, &text, code, &body.to_string());
    }
    let (res, text) = post(&router, &e, LOCK, json!({ "slotIds": [b, a, b, a] })).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST, "{text}");
    let v: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["code"], "slot.in_past");
    assert_eq!(
        v["fields"],
        json!({ "slotIds.1": "slot.in_past", "slotIds.3": "slot.in_past" })
    );
    // Corps de mauvaise forme.
    for body in [
        json!({}),
        json!({ "slotIds": "x" }),
        json!({ "slotIds": [1] }),
    ] {
        let (res, text) = post(&router, &e, LOCK, body.clone()).await;
        assert_problem(&res, &text, "form.malformed", &body.to_string());
    }
    assert_eq!(dump(&state).await, before);
    // 14 entrées passent.
    let v = lock(&router, &e, &[b; 14]).await;
    assert_eq!(v["groups"].as_array().unwrap().len(), 14);
}

#[tokio::test]
async fn a_retired_instance_still_locks_but_an_unknown_one_does_not() {
    let (state, router) = setup("instance").await;
    let e = create(&state, &router, "all").await;
    sql(
        &state,
        "UPDATE events SET instance = 'gone' WHERE id = ?",
        &[&e.id],
    )
    .await;
    let before = dump(&state).await;
    let (res, text) = post(&router, &e, LOCK, json!({ "slotIds": [e.slots[0]] })).await;
    assert_problem(&res, &text, "reference.unavailable", "unknown instance");
    assert_eq!(dump(&state).await, before);
    // Retirée (`enabled = false`) : sa disposition et son libellé sont figés quand même (AD-6).
    sql(
        &state,
        "UPDATE events SET instance = 'retired' WHERE id = ?",
        &[&e.id],
    )
    .await;
    lock(&router, &e, &[&e.slots[0]]).await;
    let rows = groups(&state, &e).await;
    assert_eq!((rows[0].3, rows[0].4, rows[0].5.as_str()), (1, 4, "OLD"));
    assert!(rows[0].6.starts_with("OLD-"));
}

#[tokio::test]
async fn a_locked_or_cancelled_event_refuses_the_lock() {
    let (state, router) = setup("states").await;
    let e = create(&state, &router, "all").await;
    lock(&router, &e, &[&e.slots[0]]).await;
    let before = dump(&state).await;
    // L'état passe avant la liste.
    for body in [json!({ "slotIds": [e.slots[1]] }), json!({ "slotIds": [] })] {
        let (res, text) = post(&router, &e, LOCK, body).await;
        assert_problem(&res, &text, "event.locked", "locked");
    }
    assert_eq!(dump(&state).await, before);
    for status in ["locked", "open"] {
        set_event_status(&state, &e, "cancelled").await;
        let before = dump(&state).await;
        let (res, text) = post(&router, &e, LOCK, json!({ "slotIds": [] })).await;
        assert_problem(&res, &text, "event.closed", status);
        let (res, text) = post(&router, &e, UNLOCK, json!({})).await;
        assert_problem(&res, &text, "event.closed", status);
        assert_eq!(dump(&state).await, before);
        set_event_status(&state, &e, status).await;
    }
}

#[tokio::test]
async fn unlock_keeps_groups_names_and_placements() {
    let (state, router) = setup("unlock").await;
    let e = create(&state, &router, "all").await;
    let c = candidate(&state, &router, &e, "Nyraelle", true).await;
    lock(&router, &e, &[&e.slots[0], &e.slots[1]]).await;
    let g = group_id(&state, &e, &e.slots[0], 1).await;
    place(&state, &c, &g).await;
    let rows = groups(&state, &e).await;

    let v = unlock(&router, &e).await;
    assert_eq!(v["event"]["status"], "open");
    assert_eq!(event_status(&state, &e).await, "open");
    assert_eq!(groups(&state, &e).await, rows);
    assert_eq!(assignments(&state).await, [(c.id.clone(), g.clone())]);
    // Le DTO garde les groupes, retenus tels quels.
    assert_eq!(v["groups"].as_array().unwrap().len(), 2);
    assert!(
        v["groups"]
            .as_array()
            .unwrap()
            .iter()
            .all(|g| g["retained"] == true)
    );

    // Déjà `open` : rien à déverrouiller.
    let before = dump(&state).await;
    let (res, text) = post(&router, &e, UNLOCK, json!({})).await;
    assert_problem(&res, &text, "event.not_locked", "open");
    assert_eq!(dump(&state).await, before);
}

#[tokio::test]
async fn unlock_needs_an_upcoming_slot() {
    let (state, router) = setup("no_upcoming").await;
    let e = create(&state, &router, "all").await;
    lock(&router, &e, &[&e.slots[0]]).await;
    // Le premier a commencé, le second reste à venir : permis.
    sql(
        &state,
        "UPDATE slots SET starts_at = ? WHERE id = ?",
        &["2026-11-05T20:00:00Z", &e.slots[0]],
    )
    .await;
    unlock(&router, &e).await;
    lock(&router, &e, &[&e.slots[1]]).await;
    // Le second commence maintenant : plus aucun créneau à venir.
    sql(
        &state,
        "UPDATE slots SET starts_at = ? WHERE id = ?",
        &[NOW, &e.slots[1]],
    )
    .await;
    let before = dump(&state).await;
    let (res, text) = post(&router, &e, UNLOCK, json!({})).await;
    assert_problem(&res, &text, "event.no_upcoming_slot", "no upcoming");
    assert_eq!(dump(&state).await, before);
    // Corps de mauvaise forme.
    let (res, text) = post_raw_with(&router, UNLOCK, "{", &organizer(&e)).await;
    assert_problem(&res, &text, "form.malformed", "unlock body");
}

#[tokio::test]
async fn relock_with_fewer_groups_puts_the_rest_to_sleep() {
    let (state, router) = setup("fewer").await;
    let e = create(&state, &router, "all").await;
    let c = candidate(&state, &router, &e, "Nyraelle", true).await;
    let a = e.slots[0].as_str();
    lock(&router, &e, &[a, a]).await;
    let first = groups(&state, &e).await;
    let a2 = group_id(&state, &e, a, 2).await;
    place(&state, &c, &a2).await;
    unlock(&router, &e).await;

    let v = lock(&router, &e, &[a]).await;
    let rows = groups(&state, &e).await;
    // A#1 repris tel quel ; A#2 gardé en sommeil, nom et placement compris.
    assert_eq!(rows[0], first[0]);
    assert_eq!(
        rows[1],
        (
            first[1].0.clone(),
            2,
            0,
            2,
            5,
            "LUDRA".into(),
            first[1].6.clone()
        )
    );
    assert_eq!(assignments(&state).await, [(c.id.clone(), a2.clone())]);
    let dto = v["groups"].as_array().unwrap();
    assert_eq!(
        dto.iter()
            .map(|g| g["retained"].clone())
            .collect::<Vec<_>>(),
        [json!(true), json!(false)]
    );
    // Un groupe en sommeil compte pour `hasGroups`.
    assert_eq!(v["slots"][0]["hasGroups"], true);

    // Repris au verrouillage suivant : réactivé, même nom.
    unlock(&router, &e).await;
    lock(&router, &e, &[a, a]).await;
    assert_eq!(groups(&state, &e).await, first);
}

#[tokio::test]
async fn relock_leaves_the_groups_of_past_slots_as_they_are() {
    let (state, router) = setup("past_groups").await;
    let e = create(&state, &router, "all").await;
    let (a, b) = (e.slots[0].as_str(), e.slots[1].as_str());
    lock(&router, &e, &[a, b]).await;
    unlock(&router, &e).await;
    // `a` a commencé : il ne peut plus figurer dans la liste, son groupe reste retenu.
    sql(
        &state,
        "UPDATE slots SET starts_at = ? WHERE id = ?",
        &["2026-11-05T20:00:00Z", a],
    )
    .await;
    let before = groups(&state, &e).await;
    let v = lock(&router, &e, &[b, b]).await;
    let rows = groups(&state, &e).await;
    assert_eq!(rows[0], before[0]);
    assert_eq!(rows[0].2, 1);
    assert_eq!(rows[1], before[1]);
    assert_eq!((rows[2].0.as_str(), rows[2].1, rows[2].2), (b, 2, 1));
    assert_eq!(v["groups"][0]["retained"], true);
}

#[tokio::test]
async fn relock_with_more_groups_names_only_the_new_ones() {
    let (state, router) = setup_full(
        "more",
        &config(&[]),
        Scripted::with(&["AAAA", "BBBB", "CCCC"]),
    )
    .await;
    let e = create(&state, &router, "all").await;
    let a = e.slots[0].as_str();
    lock(&router, &e, &[a]).await;
    unlock(&router, &e).await;
    lock(&router, &e, &[a, a]).await;
    // A#1 garde son nom ; un seul code tiré pour A#2.
    assert_eq!(
        names(&groups(&state, &e).await),
        ["LUDRA-AAAA", "LUDRA-BBBB"]
    );
}

#[tokio::test]
async fn an_identical_relock_keeps_every_room_name() {
    let (state, router) = setup("identical").await;
    let e = create(&state, &router, "all").await;
    let (a, b) = (e.slots[0].as_str(), e.slots[1].as_str());
    lock(&router, &e, &[a, b, a]).await;
    let first = groups(&state, &e).await;
    unlock(&router, &e).await;
    lock(&router, &e, &[a, b, a]).await;
    assert_eq!(groups(&state, &e).await, first);
    // L'ordre de la liste ne compte pas, seul le rang d'occurrence.
    unlock(&router, &e).await;
    lock(&router, &e, &[a, a, b]).await;
    assert_eq!(groups(&state, &e).await, first);
}

#[tokio::test]
async fn removing_a_slot_in_open_cascades_its_groups_and_placements() {
    let (state, router) = setup("cascade").await;
    let e = create(&state, &router, "all").await;
    let c = candidate(&state, &router, &e, "Nyraelle", true).await;
    let d = candidate(&state, &router, &e, "Kaelen", true).await;
    let (a, b) = (e.slots[0].as_str(), e.slots[1].as_str());
    lock(&router, &e, &[a, a, b]).await;
    place(&state, &c, &group_id(&state, &e, a, 2).await).await;
    let gb = group_id(&state, &e, b, 1).await;
    place(&state, &d, &gb).await;
    let kept = groups(&state, &e).await[2].clone();
    unlock(&router, &e).await;

    let (res, text) = post(&router, &e, REMOVAL, json!({ "slotId": a })).await;
    let v = ok(&res, &text, "remove");
    assert_eq!(groups(&state, &e).await, [kept]);
    assert_eq!(assignments(&state).await, [(d.id.clone(), gb)]);
    assert_eq!(v["groups"].as_array().unwrap().len(), 1);
    assert_eq!(v["slots"][0]["hasGroups"], true);
}

#[tokio::test]
async fn withdrawal_and_refusal_remove_the_placement() {
    let (state, router) = setup("unplace").await;
    let e = create(&state, &router, "all").await;
    let c = candidate(&state, &router, &e, "Nyraelle", true).await;
    let d = candidate(&state, &router, &e, "Kaelen", true).await;
    let x = candidate(&state, &router, &e, "Toma", true).await;
    lock(&router, &e, &[&e.slots[0]]).await;
    let g = group_id(&state, &e, &e.slots[0], 1).await;
    place(&state, &c, &g).await;
    sql(
        &state,
        "INSERT INTO assignments (event_id, candidature_id, group_id, party, party_index, \
         bench_index) VALUES ((SELECT event_id FROM groups WHERE id = ?2), ?1, ?2, NULL, NULL, 1)",
        &[&d.id, &g],
    )
    .await;
    sql(
        &state,
        "INSERT INTO assignments (event_id, candidature_id, group_id, party, party_index, \
         bench_index) VALUES ((SELECT event_id FROM groups WHERE id = ?2), ?1, ?2, 1, 2, NULL)",
        &[&x.id, &g],
    )
    .await;

    // Refus par l'organisateur, en `locked`.
    let (res, text) = post(
        &router,
        &e,
        STATUS,
        json!({ "candidatureId": c.id, "status": "rejected" }),
    )
    .await;
    ok(&res, &text, "reject");
    // Retrait par le participant.
    let (res, text) = post_json_with(
        &router,
        "/api/participant/withdrawal",
        &json!({}),
        &[("x-link-secret", d.secret.as_str())],
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK, "{text}");
    assert_eq!(assignments(&state).await, [(x.id.clone(), g.clone())]);
    // Accepter ne place rien, et ne retire rien.
    let (res, text) = post(
        &router,
        &e,
        STATUS,
        json!({ "candidatureId": x.id, "status": "accepted" }),
    )
    .await;
    ok(&res, &text, "accept again");
    assert_eq!(assignments(&state).await, [(x.id.clone(), g)]);
}

#[tokio::test]
async fn only_the_organizer_secret_opens_lock_and_unlock() {
    let (state, router) = setup("rights").await;
    let e = create(&state, &router, "all").await;
    let c = candidate(&state, &router, &e, "Nyraelle", false).await;
    let unknown = "A".repeat(43);
    let before = dump(&state).await;
    let bodies = [
        (LOCK, json!({ "slotIds": [e.slots[0]] })),
        (UNLOCK, json!({})),
    ];
    for (headers, code) in [
        (vec![], "auth.missing"),
        (vec![("x-link-secret", unknown.as_str())], "auth.not_found"),
        (vec![("x-link-secret", e.join.as_str())], "auth.wrong_right"),
        (
            vec![("x-link-secret", c.secret.as_str())],
            "auth.wrong_right",
        ),
    ] {
        for (uri, body) in &bodies {
            let (res, text) = post_json_with(&router, uri, body, &headers).await;
            assert_problem(&res, &text, code, &format!("{uri} {code}"));
        }
    }
    assert_eq!(dump(&state).await, before);
    for (uri, _) in &bodies {
        let (res, text) = send_with(&router, Method::GET, uri, &organizer(&e)).await;
        assert_problem(&res, &text, "server.method_not_allowed", uri);
    }
}

#[tokio::test]
async fn lock_and_unlock_use_the_write_quota() {
    let (state, router) = setup_full(
        "quota",
        &config(&[("RATE_WRITE", "1/h")]),
        DeterministicIdGenerator::new(),
    )
    .await;
    let e = create(&state, &router, "all").await;
    lock(&router, &e, &[&e.slots[0]]).await;
    let before = dump(&state).await;
    for (uri, body) in [
        (UNLOCK, json!({})),
        (LOCK, json!({ "slotIds": [e.slots[0]] })),
    ] {
        let (res, text) = post(&router, &e, uri, body).await;
        assert_problem(&res, &text, "limit.rate_limited", uri);
    }
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
    let (state, router) = setup_full(
        "logs",
        &config(&[]),
        // A#1 au premier passage, A#2 au second, puis B#1 libre et cinq collisions pour B#2.
        Scripted::with(&[
            "K7Q2", "M3NP", "XYZW", "XYZW", "XYZW", "XYZW", "XYZW", "XYZW",
        ]),
    )
    .await;
    let e = create(&state, &router, "all").await;
    let requests = || {
        let (router, e) = (&router, &e);
        async move {
            lock(router, e, &[&e.slots[0]]).await;
            unlock(router, e).await;
        }
    };
    // Même gabarit que `organizer_settings::no_secret_nor_free_text_in_the_logs` : premier passage
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
    // Un groupe neuf (`M3NP`), puis un échec sur collisions (cinq `XYZW` contre B#1).
    lock(&router, &e, &[&e.slots[0], &e.slots[0]]).await;
    unlock(&router, &e).await;
    let (res, _) = post(
        &router,
        &e,
        LOCK,
        json!({ "slotIds": [e.slots[0], e.slots[0], e.slots[1], e.slots[1]] }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
    drop(guard);
    let log = String::from_utf8(captured.0.lock().unwrap().clone()).unwrap();
    for line in ["event locked", "event unlocked", "room name collisions"] {
        assert!(log.contains(line), "{line} missing in {log}");
    }
    assert!(log.contains(&e.id), "{log}");
    for leak in [
        e.organizer.as_str(),
        e.join.as_str(),
        "LUDRA",
        "K7Q2",
        "M3NP",
        "XYZW",
        TEST_CLIENT_IP,
    ] {
        assert!(!log.contains(leak), "{leak} in {log}");
    }
}
