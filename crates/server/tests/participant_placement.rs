//! Matrice HTTP de la story 3-5 : `placement`, `teammates` et `backInPreparation` de la vue
//! participant `GET /api/participant`. Placement seulement en `locked`, pour une candidature
//! `accepted` placée dans un groupe retenu ; coéquipiers par équipe, réduits à `pseudoSrv` et `class` ;
//! jamais le banc pour un titulaire, jamais un autre groupe ni une candidature non `accepted`.
//! Valeurs fictives.

mod common;

use axum::http::{Method, StatusCode};
use common::candidatures::*;
use common::*;
use serde_json::{Value, json};
use server::app::AppState;

const VIEW: &str = "/api/participant";
const ORGANIZER_VIEW: &str = "/api/organizer";
const LOCK: &str = "/api/organizer/lock";
const UNLOCK: &str = "/api/organizer/unlock";
const PLACEMENT: &str = "/api/organizer/placement";

async fn setup(name: &str) -> (AppState, axum::Router) {
    candidature_setup(
        &format!("participant_placement_{name}"),
        &test_config(&[
            ("RATE_CREATE", "100/h"),
            ("RATE_CANDIDATURE", "1000/h"),
            ("RATE_WRITE", "1000/h"),
            ("RATE_READ", "1000/min"),
        ]),
    )
    .await
}

async fn organizer(router: &axum::Router, e: &Event, uri: &str, body: Value) -> Value {
    let (res, text) = post_json_with(router, uri, &body, &[("x-link-secret", &e.organizer)]).await;
    assert_eq!(res.status(), StatusCode::OK, "{uri}: {text}");
    serde_json::from_str(&text).unwrap()
}

async fn lock(router: &axum::Router, e: &Event, slots: &[&str]) -> Value {
    organizer(router, e, LOCK, json!({ "slotIds": slots })).await
}

async fn unlock(router: &axum::Router, e: &Event) {
    organizer(router, e, UNLOCK, json!({})).await;
}

async fn place(router: &axum::Router, e: &Event, placements: Vec<Value>) {
    organizer(router, e, PLACEMENT, json!({ "placements": placements })).await;
}

async fn organizer_view(router: &axum::Router, e: &Event) -> Value {
    let (res, text) = send_with(
        router,
        Method::GET,
        ORGANIZER_VIEW,
        &[("x-link-secret", &e.organizer)],
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK, "{text}");
    serde_json::from_str(&text).unwrap()
}

/// Vue participant, et son corps brut (pour chercher un pseudo qui ne doit pas y être).
async fn read(router: &axum::Router, c: &Cand) -> (Value, String) {
    let (res, text) = send_with(router, Method::GET, VIEW, &[("x-link-secret", &c.secret)]).await;
    assert_eq!(res.status(), StatusCode::OK, "{text}");
    (serde_json::from_str(&text).unwrap(), text)
}

/// Id du groupe retenu `n` (0 = G1) de la vue organisateur.
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

/// Le groupe `id` de la vue organisateur.
fn group_of<'a>(v: &'a Value, id: &str) -> &'a Value {
    v["groups"]
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["id"] == id)
        .unwrap()
}

fn starter(c: &Cand, g: &str, party: i64, index: i64) -> Value {
    json!({ "candidatureId": c.id, "groupId": g, "party": party, "index": index })
}

fn benched(c: &Cand, g: &str, bench_index: i64) -> Value {
    json!({ "candidatureId": c.id, "groupId": g, "benchIndex": bench_index })
}

/// Candidature `accepted` (lien direct) au pseudo `pseudo`, de classe `class`, aux deux créneaux.
async fn player(
    state: &AppState,
    router: &axum::Router,
    e: &Event,
    pseudo: &str,
    class: &str,
) -> Cand {
    candidate_with(
        state,
        router,
        e,
        json!({ "pseudo": pseudo, "class": class }),
        true,
    )
    .await
}

/// `Pseudo[SRV]` de la candidature `c`, tel que le serveur le compose pour l'organisateur.
fn pseudo_srv(organizer: &Value, c: &Cand) -> String {
    organizer["candidatures"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["id"] == c.id.as_str())
        .unwrap()["pseudoSrv"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn mate(organizer: &Value, c: &Cand, class: &str) -> Value {
    json!({ "pseudoSrv": pseudo_srv(organizer, c), "class": class })
}

fn party(n: u32, members: Vec<Value>) -> Value {
    json!({ "party": n, "members": members })
}

fn assert_unplaced(v: &Value, ctx: &str) {
    assert!(v.get("placement").is_none(), "{ctx}: {v}");
    assert!(v.get("teammates").is_none(), "{ctx}: {v}");
}

/// Un raid verrouillé sur le premier créneau, G1 placé : Équipe 1 = A1, A2 ; Équipe 2 = B1, B2, B4
/// (`B4` en slot 4) ; banc = Ben1, Ben2.
struct Raid {
    e: Event,
    g1: String,
    a1: Cand,
    a2: Cand,
    b1: Cand,
    b2: Cand,
    b4: Cand,
    ben1: Cand,
    ben2: Cand,
}

async fn raid(state: &AppState, router: &axum::Router) -> Raid {
    let e = create(state, router, "all").await;
    let a1 = player(state, router, &e, "Aldric", "gladiator").await;
    let a2 = player(state, router, &e, "Aveline", "cleric").await;
    let b1 = player(state, router, &e, "Borin", "templar").await;
    let b2 = player(state, router, &e, "Brisa", "chanter").await;
    let b4 = player(state, router, &e, "Bastien", "cleric").await;
    let ben1 = player(state, router, &e, "Benedra", "gladiator").await;
    let ben2 = player(state, router, &e, "Bertil", "templar").await;
    let g1 = group(&lock(router, &e, &[&e.slots[0]]).await, 0);
    place(
        router,
        &e,
        vec![
            starter(&a1, &g1, 1, 2),
            starter(&a2, &g1, 1, 1),
            starter(&b4, &g1, 2, 4),
            starter(&b1, &g1, 2, 1),
            starter(&b2, &g1, 2, 2),
            benched(&ben1, &g1, 1),
            benched(&ben2, &g1, 2),
        ],
    )
    .await;
    Raid {
        e,
        g1,
        a1,
        a2,
        b1,
        b2,
        b4,
        ben1,
        ben2,
    }
}

#[tokio::test]
async fn a_starter_sees_their_seat_the_room_and_only_the_other_starters_of_their_party() {
    let (state, router) = setup("starter").await;
    let r = raid(&state, &router).await;
    let o = organizer_view(&router, &r.e).await;
    let g1 = group_of(&o, &r.g1);
    let (v, _) = read(&router, &r.b4).await;
    assert_eq!(
        v["placement"],
        json!({
            "groupNumber": 1,
            "slotId": r.e.slots[0],
            "startsAt": "2026-11-11T20:00:00Z",
            "roomName": g1["roomName"],
            "seat": { "kind": "starter", "party": 2, "index": 4 },
        })
    );
    assert_eq!(
        v["teammates"],
        json!([party(
            2,
            vec![mate(&o, &r.b1, "templar"), mate(&o, &r.b2, "chanter")]
        )])
    );
    assert!(v.get("backInPreparation").is_none(), "{v}");
}

#[tokio::test]
async fn a_benched_player_sees_the_starters_of_both_parties_by_party_then_slot() {
    let (state, router) = setup("bench").await;
    let r = raid(&state, &router).await;
    let o = organizer_view(&router, &r.e).await;
    let (v, body) = read(&router, &r.ben2).await;
    assert_eq!(
        v["placement"]["seat"],
        json!({ "kind": "bench", "benchIndex": 2 })
    );
    assert_eq!(v["placement"]["groupNumber"], 1);
    assert_eq!(
        v["teammates"],
        json!([
            party(
                1,
                vec![mate(&o, &r.a2, "cleric"), mate(&o, &r.a1, "gladiator")]
            ),
            party(
                2,
                vec![
                    mate(&o, &r.b1, "templar"),
                    mate(&o, &r.b2, "chanter"),
                    mate(&o, &r.b4, "cleric"),
                ]
            ),
        ])
    );
    // Pas l'autre joueur au banc.
    assert!(!body.contains("Benedra"), "{body}");
}

#[tokio::test]
async fn every_teammate_has_exactly_pseudo_srv_and_class() {
    let (state, router) = setup("keys").await;
    let r = raid(&state, &router).await;
    for c in [&r.a1, &r.b4, &r.ben1] {
        let (v, _) = read(&router, c).await;
        let parties = v["teammates"].as_array().unwrap();
        assert!(!parties.is_empty());
        let sorted = |x: &Value| {
            let mut keys: Vec<_> = x.as_object().unwrap().keys().cloned().collect();
            keys.sort_unstable();
            keys
        };
        let mut seen = 0;
        for p in parties {
            assert_eq!(sorted(p), ["members", "party"], "{p}");
            for m in p["members"].as_array().unwrap() {
                assert_eq!(sorted(m), ["class", "pseudoSrv"], "{m}");
                seen += 1;
            }
        }
        assert!(seen > 0);
        let mut keys: Vec<_> = v["placement"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            ["groupNumber", "roomName", "seat", "slotId", "startsAt"]
        );
    }
}

#[tokio::test]
async fn an_accepted_candidature_without_a_seat_is_unplaced() {
    let (state, router) = setup("unplaced").await;
    let r = raid(&state, &router).await;
    let late = player(&state, &router, &r.e, "Tardif", "cleric").await;
    let (v, _) = read(&router, &late).await;
    assert_eq!(v["status"], "accepted");
    assert_eq!(v["event"]["status"], "locked");
    assert_unplaced(&v, "no row");
    assert!(v.get("backInPreparation").is_none(), "{v}");
}

#[tokio::test]
async fn a_seat_in_a_sleeping_group_is_unplaced_and_the_group_number_skips_it() {
    let (state, router) = setup("sleeping").await;
    let r = raid(&state, &router).await;
    // Re-verrouillage sur le second créneau seul : G1 (premier créneau) s'endort avec ses
    // placements, un nouveau G1 apparaît.
    unlock(&router, &r.e).await;
    let o = lock(&router, &r.e, &[&r.e.slots[1]]).await;
    let new_g1 = group(&o, 0);
    assert_ne!(new_g1, r.g1);
    assert_eq!(group_of(&o, &r.g1)["retained"], false);
    let fresh = player(&state, &router, &r.e, "Nouveau", "cleric").await;
    place(&router, &r.e, vec![starter(&fresh, &new_g1, 1, 1)]).await;
    for c in [&r.b4, &r.ben1] {
        let (v, _) = read(&router, c).await;
        assert_unplaced(&v, "sleeping");
    }
    let (v, body) = read(&router, &fresh).await;
    assert_eq!(v["placement"]["groupNumber"], 1);
    assert_eq!(v["teammates"], json!([party(1, vec![])]));
    assert!(!body.contains("Bastien"), "{body}");
}

#[tokio::test]
async fn a_candidature_that_is_not_accepted_sees_no_seat_and_is_never_a_teammate() {
    let (state, router) = setup("not_accepted").await;
    let r = raid(&state, &router).await;
    // Lignes gardées en SQL malgré le statut : la vue doit filtrer d'elle-même.
    set_status(&state, &r.b1, "pending").await;
    set_status(&state, &r.b2, "rejected").await;
    set_status(&state, &r.a1, "withdrawn").await;
    for (c, status) in [
        (&r.b1, "pending"),
        (&r.b2, "rejected"),
        (&r.a1, "withdrawn"),
    ] {
        let (v, _) = read(&router, c).await;
        assert_eq!(v["status"], status);
        assert_unplaced(&v, status);
    }
    let o = organizer_view(&router, &r.e).await;
    let (v, body) = read(&router, &r.b4).await;
    assert_eq!(v["teammates"], json!([party(2, vec![])]));
    for pseudo in ["Borin", "Brisa", "Aldric"] {
        assert!(!body.contains(pseudo), "{pseudo}: {body}");
    }
    let (v, _) = read(&router, &r.ben1).await;
    assert_eq!(
        v["teammates"],
        json!([
            party(1, vec![mate(&o, &r.a2, "cleric")]),
            party(2, vec![mate(&o, &r.b4, "cleric")]),
        ])
    );
}

#[tokio::test]
async fn back_in_preparation_after_an_unlock_without_seat_nor_teammates() {
    let (state, router) = setup("unlocked").await;
    let r = raid(&state, &router).await;
    unlock(&router, &r.e).await;
    for c in [&r.b4, &r.ben1] {
        let (v, body) = read(&router, c).await;
        assert_eq!(v["event"]["status"], "open");
        assert_unplaced(&v, "open");
        assert_eq!(v["backInPreparation"], true);
        assert!(!body.contains("Borin"), "{body}");
    }
    // Seule une candidature `accepted` est concernée : ni un `pending`, ni un `rejected`.
    let newcomer = candidate(&state, &router, &r.e, "Novice", false).await;
    set_status(&state, &r.b2, "rejected").await;
    for (c, status) in [(&newcomer, "pending"), (&r.b2, "rejected")] {
        let (v, _) = read(&router, c).await;
        assert_eq!(v["status"], status);
        assert_unplaced(&v, status);
        assert!(v.get("backInPreparation").is_none(), "{status}: {v}");
    }
    // Reverrouillé sur le même créneau : la place revient, avec le même nom de salle.
    let room = {
        let o = organizer_view(&router, &r.e).await;
        group_of(&o, &r.g1)["roomName"].clone()
    };
    lock(&router, &r.e, &[&r.e.slots[0]]).await;
    let (v, _) = read(&router, &r.b4).await;
    assert_eq!(v["placement"]["roomName"], room);
    assert!(v.get("backInPreparation").is_none(), "{v}");
}

#[tokio::test]
async fn an_open_event_never_locked_is_not_back_in_preparation() {
    let (state, router) = setup("never_locked").await;
    let e = create(&state, &router, "all").await;
    let c = player(&state, &router, &e, "Nyraelle", "cleric").await;
    let (v, _) = read(&router, &c).await;
    assert_unplaced(&v, "never locked");
    assert!(v.get("backInPreparation").is_none(), "{v}");
}

#[tokio::test]
async fn two_placed_groups_never_leak_into_each_other() {
    let (state, router) = setup("two_groups").await;
    let e = create(&state, &router, "all").await;
    let names = [
        "Gun", "Gdeux", "Gtrois", "Gbanc", "Hun", "Hdeux", "Htrois", "Hbanc",
    ];
    let mut p = Vec::new();
    for name in names {
        p.push(player(&state, &router, &e, name, "gladiator").await);
    }
    let o = lock(&router, &e, &[&e.slots[0], &e.slots[1]]).await;
    let (g, h) = (group(&o, 0), group(&o, 1));
    place(
        &router,
        &e,
        vec![
            starter(&p[0], &g, 1, 1),
            starter(&p[1], &g, 1, 2),
            starter(&p[2], &g, 2, 1),
            benched(&p[3], &g, 1),
            starter(&p[4], &h, 1, 1),
            starter(&p[5], &h, 1, 2),
            starter(&p[6], &h, 2, 1),
            benched(&p[7], &h, 1),
        ],
    )
    .await;
    let o = organizer_view(&router, &e).await;
    let room_h = group_of(&o, &h)["roomName"].as_str().unwrap().to_owned();
    // Titulaire de G : ni l'autre équipe, ni le banc, ni H.
    let (v, body) = read(&router, &p[0]).await;
    assert_eq!(
        v["teammates"],
        json!([party(1, vec![mate(&o, &p[1], "gladiator")])])
    );
    for absent in [
        "Gtrois", "Gbanc", "Hun", "Hdeux", "Htrois", "Hbanc", &room_h,
    ] {
        assert!(!body.contains(absent), "{absent}: {body}");
    }
    // Banc de H : ses titulaires, rien de G.
    let (v, body) = read(&router, &p[7]).await;
    assert_eq!(v["placement"]["groupNumber"], 2);
    assert_eq!(v["placement"]["slotId"], e.slots[1]);
    assert_eq!(v["placement"]["roomName"], room_h);
    assert_eq!(
        v["teammates"],
        json!([
            party(
                1,
                vec![mate(&o, &p[4], "gladiator"), mate(&o, &p[5], "gladiator")]
            ),
            party(2, vec![mate(&o, &p[6], "gladiator")]),
        ])
    );
    for absent in ["Gun", "Gdeux", "Gtrois", "Gbanc"] {
        assert!(!body.contains(absent), "{absent}: {body}");
    }
}

#[tokio::test]
async fn a_locked_then_cancelled_raid_shows_neither_seat_nor_back_in_preparation() {
    let (state, router) = setup("cancelled").await;
    let r = raid(&state, &router).await;
    organizer(&router, &r.e, "/api/organizer/cancellation", json!({})).await;
    for c in [&r.b4, &r.ben1] {
        let (v, _) = read(&router, c).await;
        assert_eq!(v["event"]["status"], "cancelled");
        assert_unplaced(&v, "cancelled");
        assert!(v.get("backInPreparation").is_none(), "{v}");
    }
}

#[tokio::test]
async fn an_edit_after_an_unlock_answers_back_in_preparation_without_a_seat() {
    let (state, router) = setup("edit_unlocked").await;
    let r = raid(&state, &router).await;
    unlock(&router, &r.e).await;
    let body = json!({
        "pseudo": "Bastien",
        "server": "israphel",
        "class": "cleric",
        "cp": 2000,
        "availabilities": [{ "slotId": r.e.slots[0], "preferred": false }],
    });
    let (res, text) = post_json_with(
        &router,
        "/api/participant/candidature",
        &body,
        &[("x-link-secret", &r.b4.secret)],
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK, "{text}");
    let v: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["cp"], 2000);
    assert_unplaced(&v, "edit");
    assert_eq!(v["backInPreparation"], true);
}
