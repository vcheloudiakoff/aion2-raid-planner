//! Matrice HTTP de la story 3-3 : `apply_placement` sous le seul secret organisateur. Placement
//! complet des groupes retenus, banc renuméroté, avertissements (indisponible, borne de classe)
//! jamais refusés, violations structurelles en 400 sans rien écrire, groupes en sommeil gardés ou
//! quittés, état `locked` seulement, quota `write`, journaux muets. Valeurs fictives.

mod common;

use std::sync::{Arc, Mutex};

use axum::http::{Method, StatusCode};
use common::candidatures::*;
use common::*;
use serde_json::{Value, json};
use server::app::AppState;
use server::config::Config;

const VIEW: &str = "/api/organizer";
const LOCK: &str = "/api/organizer/lock";
const UNLOCK: &str = "/api/organizer/unlock";
const PLACEMENT: &str = "/api/organizer/placement";

fn config(overrides: &[(&str, &str)]) -> Config {
    let mut all = vec![
        ("RATE_CREATE", "100/h"),
        ("RATE_CANDIDATURE", "1000/h"),
        ("RATE_WRITE", "1000/h"),
    ];
    all.extend_from_slice(overrides);
    test_config(&all)
}

async fn setup_with(name: &str, config: &Config) -> (AppState, axum::Router) {
    candidature_setup(&format!("organizer_placement_{name}"), config).await
}

async fn setup(name: &str) -> (AppState, axum::Router) {
    setup_with(name, &config(&[])).await
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

async fn place(router: &axum::Router, e: &Event, placements: Value) -> Value {
    let (res, text) = post(router, e, PLACEMENT, json!({ "placements": placements })).await;
    ok(&res, &text, "placement")
}

async fn read(router: &axum::Router, e: &Event) -> Value {
    let (res, text) = send_with(router, Method::GET, VIEW, &organizer(e)).await;
    ok(&res, &text, "read")
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

fn starter(c: &Cand, g: &str, party: i64, index: i64) -> Value {
    json!({ "candidatureId": c.id, "groupId": g, "party": party, "index": index })
}

fn benched(c: &Cand, g: &str, bench_index: i64) -> Value {
    json!({ "candidatureId": c.id, "groupId": g, "benchIndex": bench_index })
}

/// Le groupe `id` de la vue.
fn group_of<'a>(v: &'a Value, id: &str) -> &'a Value {
    v["groups"]
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["id"] == id)
        .unwrap()
}

/// Ligne `assignments` : `candidature_id, group_id, party, party_index, bench_index`.
type Row = (String, String, Option<i64>, Option<i64>, Option<i64>);

async fn rows(state: &AppState) -> Vec<Row> {
    sqlx::query_as(
        "SELECT candidature_id, group_id, party, party_index, bench_index FROM assignments \
         ORDER BY group_id, party, party_index, bench_index",
    )
    .fetch_all(&state.db.read)
    .await
    .unwrap()
}

/// Toutes les lignes de l'agrégat : rien ne doit changer sur un refus.
async fn dump(state: &AppState) -> Vec<String> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT 'e|' || id || '|' || status FROM events \
         UNION ALL SELECT 'c|' || id || '|' || status || '|' || updated_at FROM candidatures \
         UNION ALL SELECT 'g|' || id || '|' || retained || '|' || room_name FROM groups \
         UNION ALL SELECT 'p|' || candidature_id || '|' || group_id || '|' || \
         coalesce(party, '-') || '|' || coalesce(party_index, '-') || '|' || \
         coalesce(bench_index, '-') FROM assignments ORDER BY 1",
    )
    .fetch_all(&state.db.read)
    .await
    .unwrap();
    rows.into_iter().map(|(r,)| r).collect()
}

/// Candidature `accepted` de classe `class`, disponible sur `slots`.
async fn player(
    state: &AppState,
    router: &axum::Router,
    e: &Event,
    pseudo: &str,
    class: &str,
    slots: &[&str],
) -> Cand {
    let availabilities: Vec<Value> = slots
        .iter()
        .map(|s| json!({ "slotId": s, "preferred": false }))
        .collect();
    candidate_with(
        state,
        router,
        e,
        json!({ "pseudo": pseudo, "class": class, "availabilities": availabilities }),
        true,
    )
    .await
}

#[tokio::test]
async fn ten_starters_and_one_on_the_bench_come_back_with_warnings_but_no_error() {
    let (state, router) = setup("nominal").await;
    let e = create(&state, &router, "all").await;
    let (a, b) = (e.slots[0].as_str(), e.slots[1].as_str());
    let mut players = Vec::new();
    for n in 0..11 {
        // Le 3e et le banc n'ont coché que l'autre créneau.
        let slots: &[&str] = if n == 2 || n == 10 { &[b] } else { &[a] };
        players.push(
            player(
                &state,
                &router,
                &e,
                &format!("Joueur{n}"),
                "gladiator",
                slots,
            )
            .await,
        );
    }
    let g = group(&lock(&router, &e, &[a]).await, 0);
    let mut body: Vec<Value> = (0..10)
        .map(|n| starter(&players[n], &g, (n / 5 + 1) as i64, (n % 5 + 1) as i64))
        .collect();
    body.push(benched(&players[10], &g, 4));
    let v = place(&router, &e, json!(body)).await;
    assert_eq!(v["event"]["status"], "locked");

    let read = read(&router, &e).await;
    assert_eq!(read["groups"], v["groups"]);
    let dto = group_of(&read, &g);
    let starters = dto["starters"].as_array().unwrap();
    assert_eq!(starters.len(), 10);
    for (n, s) in starters.iter().enumerate() {
        assert_eq!(
            s,
            &json!({
                "candidatureId": players[n].id,
                "party": n / 5 + 1,
                "index": n % 5 + 1,
                "unavailable": n == 2,
            }),
            "{n}"
        );
    }
    // Banc renuméroté à 1, indisponible signalé.
    assert_eq!(
        dto["bench"],
        json!([{ "candidatureId": players[10].id, "benchIndex": 1, "unavailable": true }])
    );
    assert_eq!(dto["classWarnings"], json!({}));
    assert_eq!(rows(&state).await.len(), 11);
}

#[tokio::test]
async fn five_starters_in_party_one_and_one_benched_are_written_as_sent() {
    let (state, router) = setup("five").await;
    let e = create(&state, &router, "all").await;
    let a = e.slots[0].as_str();
    let mut players = Vec::new();
    for n in 0..6 {
        players.push(
            player(
                &state,
                &router,
                &e,
                &format!("Joueur{n}"),
                "gladiator",
                &[a],
            )
            .await,
        );
    }
    let g = group(&lock(&router, &e, &[a]).await, 0);
    let mut body: Vec<Value> = (0..5)
        .map(|n| starter(&players[n], &g, 1, n as i64 + 1))
        .collect();
    body.push(benched(&players[5], &g, 1));
    place(&router, &e, json!(body)).await;
    let mut expected: Vec<Row> = (0..5)
        .map(|n| {
            (
                players[n].id.clone(),
                g.clone(),
                Some(1),
                Some(n as i64 + 1),
                None,
            )
        })
        .collect();
    expected.insert(0, (players[5].id.clone(), g.clone(), None, None, Some(1)));
    assert_eq!(rows(&state).await, expected);
}

#[tokio::test]
async fn the_bench_is_renumbered_per_group_in_the_order_received() {
    let (state, router) = setup("bench").await;
    let e = create(&state, &router, "all").await;
    let (a, b) = (e.slots[0].as_str(), e.slots[1].as_str());
    let x = player(&state, &router, &e, "Xan", "cleric", &[a, b]).await;
    let y = player(&state, &router, &e, "Yra", "cleric", &[a, b]).await;
    let z = player(&state, &router, &e, "Zel", "cleric", &[a, b]).await;
    let v = lock(&router, &e, &[a, b]).await;
    let (g1, g2) = (group(&v, 0), group(&v, 1));
    let v = place(
        &router,
        &e,
        json!([
            benched(&x, &g1, 9),
            benched(&y, &g2, 5),
            benched(&z, &g1, 2)
        ]),
    )
    .await;
    assert_eq!(
        group_of(&v, &g1)["bench"],
        json!([
            { "candidatureId": x.id, "benchIndex": 1, "unavailable": false },
            { "candidatureId": z.id, "benchIndex": 2, "unavailable": false },
        ])
    );
    assert_eq!(
        group_of(&v, &g2)["bench"],
        json!([{ "candidatureId": y.id, "benchIndex": 1, "unavailable": false }])
    );
}

#[tokio::test]
async fn an_unavailable_starter_and_a_class_over_its_bound_are_warnings_only() {
    let (state, router) = setup("warnings").await;
    let e = create_with(
        &state,
        &router,
        "all",
        json!({ "classBounds": { "cleric": { "min": 0, "max": 2 }, "templar": { "min": 1, "max": 2 } } }),
    )
    .await;
    let (a, b) = (e.slots[0].as_str(), e.slots[1].as_str());
    let c1 = player(&state, &router, &e, "Clerca", "cleric", &[a]).await;
    let c2 = player(&state, &router, &e, "Clercb", "cleric", &[a]).await;
    // N'a pas coché le créneau du groupe.
    let c3 = player(&state, &router, &e, "Clercc", "cleric", &[b]).await;
    let t = player(&state, &router, &e, "Templa", "templar", &[a]).await;
    let g = group(&lock(&router, &e, &[a]).await, 0);
    let v = place(
        &router,
        &e,
        json!([
            starter(&c1, &g, 1, 1),
            starter(&c2, &g, 1, 2),
            starter(&c3, &g, 2, 1),
            // Le banc ne compte pas pour les bornes.
            benched(&t, &g, 1),
        ]),
    )
    .await;
    let dto = group_of(&v, &g);
    assert_eq!(dto["starters"][2]["candidatureId"], c3.id.as_str());
    assert_eq!(dto["starters"][2]["unavailable"], true);
    assert_eq!(dto["starters"][0]["unavailable"], false);
    assert_eq!(
        dto["classWarnings"],
        json!({
            "cleric": { "count": 3, "min": 0, "max": 2 },
            "templar": { "count": 0, "min": 1, "max": 2 },
        })
    );
}

#[tokio::test]
async fn a_structural_violation_is_a_400_with_fields_that_writes_nothing() {
    let (state, router) = setup("structure").await;
    let e = create(&state, &router, "all").await;
    let other = create(&state, &router, "all").await;
    let a = e.slots[0].as_str();
    let p1 = player(&state, &router, &e, "Una", "cleric", &[a]).await;
    let p2 = player(&state, &router, &e, "Duo", "cleric", &[a]).await;
    let pending = candidate(&state, &router, &e, "Attente", false).await;
    let stranger = player(
        &state,
        &router,
        &other,
        "Ailleurs",
        "cleric",
        &[&other.slots[0]],
    )
    .await;
    let g = group(&lock(&router, &e, &[a]).await, 0);
    let foreign = group(&lock(&router, &other, &[&other.slots[0]]).await, 0);
    place(&router, &e, json!([starter(&p1, &g, 1, 1)])).await;
    let before = dump(&state).await;
    for (body, code, fields) in [
        (
            json!([starter(&p1, &g, 1, 1), benched(&p1, &g, 1)]),
            "placement.duplicate_candidature",
            json!({ "placements.1.candidatureId": "placement.duplicate_candidature" }),
        ),
        (
            json!([starter(&p1, &g, 3, 1)]),
            "placement.out_of_layout",
            json!({ "placements.0.party": "placement.out_of_layout" }),
        ),
        (
            json!([starter(&p1, &g, 1, 6)]),
            "placement.out_of_layout",
            json!({ "placements.0.index": "placement.out_of_layout" }),
        ),
        (
            // Hors des bornes d'un `u32` : erreur de champ, pas un corps illisible.
            json!([starter(&p1, &g, 1, 99_999_999_999)]),
            "placement.out_of_layout",
            json!({ "placements.0.index": "placement.out_of_layout" }),
        ),
        (
            json!([{ "candidatureId": p1.id, "groupId": g }]),
            "placement.out_of_layout",
            json!({ "placements.0.party": "placement.out_of_layout" }),
        ),
        (
            json!([{ "candidatureId": p1.id, "groupId": g, "party": 1, "index": 1, "benchIndex": 1 }]),
            "placement.out_of_layout",
            json!({ "placements.0.benchIndex": "placement.out_of_layout" }),
        ),
        (
            json!([starter(&p1, &g, 2, 3), starter(&p2, &g, 2, 3)]),
            "placement.slot_taken",
            json!({ "placements.1.index": "placement.slot_taken" }),
        ),
        (
            json!([benched(&p1, &g, 1), benched(&p2, &g, 1)]),
            "placement.slot_taken",
            json!({ "placements.1.benchIndex": "placement.slot_taken" }),
        ),
        (
            json!([starter(&pending, &g, 1, 2)]),
            "placement.candidature_not_placeable",
            json!({ "placements.0.candidatureId": "placement.candidature_not_placeable" }),
        ),
        (
            json!([starter(&stranger, &g, 1, 2)]),
            "placement.candidature_not_placeable",
            json!({ "placements.0.candidatureId": "placement.candidature_not_placeable" }),
        ),
        (
            json!([{ "candidatureId": "x".repeat(22), "groupId": g, "party": 1, "index": 2 }]),
            "placement.candidature_not_placeable",
            json!({ "placements.0.candidatureId": "placement.candidature_not_placeable" }),
        ),
        (
            json!([starter(&p2, &foreign, 1, 1)]),
            "placement.unknown_group",
            json!({ "placements.0.groupId": "placement.unknown_group" }),
        ),
        (
            json!([{ "candidatureId": p2.id, "groupId": "nope", "benchIndex": 1 }]),
            "placement.unknown_group",
            json!({ "placements.0.groupId": "placement.unknown_group" }),
        ),
        (
            // Toutes les erreurs ensemble, la première donne le code.
            json!([starter(&pending, &g, 1, 1), starter(&p1, &g, 3, 1)]),
            "placement.candidature_not_placeable",
            json!({
                "placements.0.candidatureId": "placement.candidature_not_placeable",
                "placements.1.party": "placement.out_of_layout",
            }),
        ),
    ] {
        let (res, text) = post(&router, &e, PLACEMENT, json!({ "placements": body })).await;
        assert_fields(&res, &text, code, fields);
        assert_eq!(dump(&state).await, before, "{code}");
    }
    // Un corps de mauvaise forme reste `form.malformed`.
    for body in [
        json!({}),
        json!({ "placements": [{ "candidatureId": p1.id }] }),
        json!({ "placements": [{ "candidatureId": p1.id, "groupId": g, "party": "1", "index": 1 }] }),
        json!({ "placements": [{ "candidatureId": p1.id, "groupId": g, "party": 1.5, "index": 1 }] }),
    ] {
        let (res, text) = post(&router, &e, PLACEMENT, body.clone()).await;
        assert_problem(&res, &text, "form.malformed", &body.to_string());
    }
    assert_eq!(dump(&state).await, before);
}

#[tokio::test]
async fn a_sleeping_group_is_unknown() {
    let (state, router) = setup("sleeping_unknown").await;
    let e = create(&state, &router, "all").await;
    let a = e.slots[0].as_str();
    let p = player(&state, &router, &e, "Una", "cleric", &[a]).await;
    let v = lock(&router, &e, &[a, a]).await;
    let g2 = group(&v, 1);
    let (res, text) = post(&router, &e, UNLOCK, json!({})).await;
    ok(&res, &text, "unlock");
    lock(&router, &e, &[a]).await;
    let before = dump(&state).await;
    let (res, text) = post(
        &router,
        &e,
        PLACEMENT,
        json!({ "placements": [starter(&p, &g2, 1, 1)] }),
    )
    .await;
    assert_fields(
        &res,
        &text,
        "placement.unknown_group",
        json!({ "placements.0.groupId": "placement.unknown_group" }),
    );
    assert_eq!(dump(&state).await, before);
}

#[tokio::test]
async fn the_body_replaces_the_placement_of_the_retained_groups() {
    let (state, router) = setup("replace").await;
    let e = create(&state, &router, "all").await;
    let (a, b) = (e.slots[0].as_str(), e.slots[1].as_str());
    let x = player(&state, &router, &e, "Xan", "cleric", &[a, b]).await;
    let y = player(&state, &router, &e, "Yra", "cleric", &[a, b]).await;
    let v = lock(&router, &e, &[a, b]).await;
    let (g1, g2) = (group(&v, 0), group(&v, 1));
    place(
        &router,
        &e,
        json!([starter(&x, &g1, 1, 1), starter(&y, &g2, 1, 1)]),
    )
    .await;
    // Sans A (`x`) : retiré de G1 ; `y` passe de G2 au banc de G1.
    place(&router, &e, json!([benched(&y, &g1, 1)])).await;
    assert_eq!(
        rows(&state).await,
        [(y.id.clone(), g1.clone(), None, None, Some(1))]
    );
    // Un corps vide vide les groupes retenus.
    let v = place(&router, &e, json!([])).await;
    assert!(rows(&state).await.is_empty());
    assert_eq!(group_of(&v, &g1)["starters"], json!([]));
    assert_eq!(group_of(&v, &g1)["bench"], json!([]));
}

#[tokio::test]
async fn a_sleeping_group_keeps_its_placements_unless_the_body_moves_them() {
    let (state, router) = setup("sleeping").await;
    let e = create(&state, &router, "all").await;
    let (a, b) = (e.slots[0].as_str(), e.slots[1].as_str());
    let x = player(&state, &router, &e, "Xan", "cleric", &[a, b]).await;
    let y = player(&state, &router, &e, "Yra", "cleric", &[a, b]).await;
    let z = player(&state, &router, &e, "Zel", "cleric", &[a, b]).await;
    let v = lock(&router, &e, &[a, b]).await;
    let (g1, g2) = (group(&v, 0), group(&v, 1));
    place(
        &router,
        &e,
        json!([
            starter(&x, &g1, 1, 1),
            starter(&y, &g2, 1, 1),
            benched(&z, &g2, 1),
        ]),
    )
    .await;
    // G2 (créneau B) passe en sommeil.
    let (res, text) = post(&router, &e, UNLOCK, json!({})).await;
    ok(&res, &text, "unlock");
    let v = lock(&router, &e, &[a]).await;
    assert_eq!(group_of(&v, &g2)["retained"], false);

    // Sommeil conservé : `y` et `z`, absents du corps, gardent leur ligne de G2, que la vue montre.
    let v = place(&router, &e, json!([starter(&x, &g1, 1, 2)])).await;
    let mut expected = vec![
        (x.id.clone(), g1.clone(), Some(1), Some(2), None),
        (y.id.clone(), g2.clone(), Some(1), Some(1), None),
        (z.id.clone(), g2.clone(), None, None, Some(1)),
    ];
    expected.sort();
    let mut got = rows(&state).await;
    got.sort();
    assert_eq!(got, expected);
    assert_eq!(
        group_of(&v, &g2)["starters"][0]["candidatureId"],
        y.id.as_str()
    );

    // Sommeil déplacé : `y` passe dans G1, sa ligne de G2 disparaît ; `z` reste.
    place(
        &router,
        &e,
        json!([starter(&x, &g1, 1, 2), starter(&y, &g1, 2, 5)]),
    )
    .await;
    let mut expected = vec![
        (x.id.clone(), g1.clone(), Some(1), Some(2), None),
        (y.id.clone(), g1.clone(), Some(2), Some(5), None),
        (z.id.clone(), g2.clone(), None, None, Some(1)),
    ];
    expected.sort();
    let mut got = rows(&state).await;
    got.sort();
    assert_eq!(got, expected);

    // Le groupe repris au reverrouillage retrouve son placement.
    let (res, text) = post(&router, &e, UNLOCK, json!({})).await;
    ok(&res, &text, "unlock");
    let v = lock(&router, &e, &[a, b]).await;
    assert_eq!(
        group_of(&v, &g2)["bench"],
        json!([{ "candidatureId": z.id, "benchIndex": 1, "unavailable": false }])
    );
}

#[tokio::test]
async fn only_a_locked_event_takes_a_placement_even_once_a_slot_has_started() {
    let (state, router) = setup("states").await;
    let e = create(&state, &router, "all").await;
    let a = e.slots[0].as_str();
    let p = player(&state, &router, &e, "Una", "cleric", &[a]).await;
    let g = group(&lock(&router, &e, &[a]).await, 0);
    // Créneau commencé : pas de restriction.
    sql(
        &state,
        "UPDATE slots SET starts_at = ? WHERE id = ?",
        &["2026-11-06T09:00:00Z", a],
    )
    .await;
    place(&router, &e, json!([starter(&p, &g, 1, 1)])).await;
    for (status, code) in [("open", "event.not_locked"), ("cancelled", "event.closed")] {
        set_event_status(&state, &e, status).await;
        let before = dump(&state).await;
        // L'état passe avant la structure.
        for body in [json!([]), json!([starter(&p, &g, 9, 9)])] {
            let (res, text) = post(&router, &e, PLACEMENT, json!({ "placements": body })).await;
            assert_problem(&res, &text, code, status);
            assert_eq!(res.status(), StatusCode::CONFLICT);
        }
        assert_eq!(dump(&state).await, before, "{status}");
    }
}

#[tokio::test]
async fn only_the_organizer_secret_opens_the_placement() {
    let (state, router) = setup("rights").await;
    let e = create(&state, &router, "all").await;
    let a = e.slots[0].as_str();
    let c = player(&state, &router, &e, "Una", "cleric", &[a]).await;
    let g = group(&lock(&router, &e, &[a]).await, 0);
    let unknown = "A".repeat(43);
    let before = dump(&state).await;
    let body = json!({ "placements": [starter(&c, &g, 1, 1)] });
    for (headers, code) in [
        (vec![], "auth.missing"),
        (vec![("x-link-secret", unknown.as_str())], "auth.not_found"),
        (vec![("x-link-secret", e.join.as_str())], "auth.wrong_right"),
        (
            vec![("x-link-secret", c.secret.as_str())],
            "auth.wrong_right",
        ),
    ] {
        let (res, text) = post_json_with(&router, PLACEMENT, &body, &headers).await;
        assert_problem(&res, &text, code, code);
    }
    assert_eq!(dump(&state).await, before);
    let (res, text) = send_with(&router, Method::GET, PLACEMENT, &organizer(&e)).await;
    assert_problem(&res, &text, "server.method_not_allowed", PLACEMENT);
}

#[tokio::test]
async fn the_placement_uses_the_write_quota() {
    let (state, router) = setup_with("quota", &config(&[("RATE_WRITE", "2/h")])).await;
    let e = create(&state, &router, "all").await;
    let a = e.slots[0].as_str();
    let c = player(&state, &router, &e, "Una", "cleric", &[a]).await;
    let g = group(&lock(&router, &e, &[a]).await, 0);
    place(&router, &e, json!([starter(&c, &g, 1, 1)])).await;
    let before = dump(&state).await;
    let (res, text) = post(
        &router,
        &e,
        PLACEMENT,
        json!({ "placements": [benched(&c, &g, 1)] }),
    )
    .await;
    assert_problem(&res, &text, "limit.rate_limited", "quota");
    assert_eq!(dump(&state).await, before);
    let (res, _) = send_with(&router, Method::GET, VIEW, &organizer(&e)).await;
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn a_placement_in_one_event_leaves_another_event_untouched() {
    let (state, router) = setup("two_events").await;
    let e = create(&state, &router, "all").await;
    let f = create(&state, &router, "all").await;
    let (ea, fa) = (e.slots[0].as_str(), f.slots[0].as_str());
    let x = player(&state, &router, &e, "Xan", "cleric", &[ea]).await;
    let y = player(&state, &router, &f, "Yra", "cleric", &[fa]).await;
    let z = player(&state, &router, &f, "Zel", "cleric", &[fa]).await;
    let ge = group(&lock(&router, &e, &[ea]).await, 0);
    let gf = group(&lock(&router, &f, &[fa]).await, 0);
    place(&router, &e, json!([starter(&x, &ge, 1, 1)])).await;
    place(
        &router,
        &f,
        json!([starter(&y, &gf, 1, 1), benched(&z, &gf, 1)]),
    )
    .await;
    let of_f = |rows: Vec<Row>| -> Vec<Row> { rows.into_iter().filter(|r| r.1 == gf).collect() };
    let before = of_f(rows(&state).await);
    assert_eq!(before.len(), 2);
    // Remplacer puis vider le placement de `e` ne touche pas celui de `f`.
    place(&router, &e, json!([benched(&x, &ge, 1)])).await;
    place(&router, &e, json!([])).await;
    assert_eq!(of_f(rows(&state).await), before);
    assert!(rows(&state).await.iter().all(|r| r.1 != ge));
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
    let a = e.slots[0].as_str();
    let c = player(&state, &router, &e, "Nyraelle", "cleric", &[a]).await;
    let v = lock(&router, &e, &[a]).await;
    let g = group(&v, 0);
    let room = v["groups"][0]["roomName"].as_str().unwrap().to_owned();
    let requests = || {
        let (router, e, c, g) = (&router, &e, &c, &g);
        async move {
            place(router, e, json!([starter(c, g, 1, 1)])).await;
            post(
                router,
                e,
                PLACEMENT,
                json!({ "placements": [starter(c, g, 3, 1)] }),
            )
            .await;
        }
    };
    // Même gabarit que `organizer_lock::no_secret_nor_room_name_in_the_logs` : premier passage sous
    // un abonné muet, pour enregistrer chaque callsite avant le `set_default`.
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
    requests().await;
    drop(guard);
    let log = String::from_utf8(captured.0.lock().unwrap().clone()).unwrap();
    assert!(log.contains("placement applied"), "{log}");
    assert!(log.contains(&e.id), "{log}");
    for leak in [
        e.organizer.as_str(),
        e.join.as_str(),
        c.secret.as_str(),
        "Nyraelle",
        room.as_str(),
        TEST_CLIENT_IP,
    ] {
        assert!(!log.contains(leak), "{leak} in {log}");
    }
}
