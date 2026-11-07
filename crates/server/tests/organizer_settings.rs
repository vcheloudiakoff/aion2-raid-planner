//! Matrice HTTP de la story 2-5 : réglages, ajout et suppression de créneau, annulation, sous le
//! seul secret organisateur. Mutabilité d'AD-4, champs figés dès la première candidature, bornes,
//! cascade des disponibilités, lecture seule après annulation, quota `write`, journaux muets.

mod common;

use std::sync::{Arc, Mutex};

use axum::http::{Method, StatusCode};
use common::candidatures::*;
use common::*;
use domain::reference::ReferenceSet;
use serde_json::{Value, json};
use server::app::AppState;
use server::config::Config;
use server::infra::reference::ReferenceStore;

const VIEW: &str = "/api/organizer";
const SETTINGS: &str = "/api/organizer/settings";
const SLOTS: &str = "/api/organizer/slots";
const REMOVAL: &str = "/api/organizer/slot-removal";
const CANCELLATION: &str = "/api/organizer/cancellation";

/// Référence des tests de candidature, plus une petite instance mono-faction (`mini`, 1 × 4, 45
/// min), une instance retirée (`retired`, 75 min) et la région `na_west` ouverte. Valeurs fictives.
fn test_reference() -> ReferenceSet {
    let mut set = reference_set();
    let ludra = set
        .instances
        .iter()
        .find(|i| i.id == "ludra")
        .unwrap()
        .clone();
    let mut mini = ludra.clone();
    mini.id = "mini".into();
    mini.parties = 1;
    mini.party_size = 4;
    mini.duration_min = 45;
    mini.cross_faction = false;
    let mut retired = ludra;
    retired.id = "retired".into();
    retired.enabled = false;
    // Durée propre, pour voir qu'un ajout de créneau la lit bien.
    retired.duration_min = 75;
    set.instances.extend([mini, retired]);
    for r in &mut set.regions {
        if r.id == "na_west" {
            r.enabled = true;
        }
    }
    set
}

async fn setup_with(name: &str, config: &Config) -> (AppState, axum::Router) {
    let state = state_with_config(
        memory_db(&format!("organizer_settings_{name}")).await,
        ReferenceStore::with(test_reference()),
        config,
    );
    let router = fixture_router(state.clone());
    (state, router)
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

/// POST d'une commande organisateur sur `e`.
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
    assert_eq!(header(res, "cache-control"), "no-store", "{ctx}");
    serde_json::from_str(text).unwrap()
}

/// Corps de réglages qui reprend l'événement créé par [`create`] tel quel, `patch` par-dessus.
fn settings(patch: Value) -> Value {
    with(
        json!({
            "instance": "ludra",
            "region": "europe",
            "factionScope": "all",
            "note": "",
            "visibility": "listed",
            "classBounds": {},
        }),
        patch,
    )
}

/// Problème avec `fields`, de statut quelconque.
fn assert_refused_fields(
    res: &axum::http::Response<axum::body::Body>,
    body: &str,
    status: StatusCode,
    code: &str,
    fields: Value,
) {
    assert_eq!(res.status(), status, "{body}");
    assert_eq!(header(res, "content-type"), "application/problem+json");
    let v: Value = serde_json::from_str(body).unwrap();
    assert_eq!(v["code"], code, "{body}");
    assert_eq!(v["fields"], fields, "{body}");
}

/// Toutes les lignes de toutes les tables de l'agrégat, liens compris : rien ne doit changer sur un
/// refus.
async fn dump(state: &AppState) -> Vec<String> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT 'e|' || id || '|' || instance || '|' || region || '|' || faction_scope || '|' || \
         visibility || '|' || status || '|' || coalesce(min_cp, '-') || '|' || \
         coalesce(discord_url, '-') || '|' || note || '|' || coalesce(cancelled_at, '-') \
         FROM events \
         UNION ALL SELECT 's|' || id || '|' || event_id || '|' || starts_at || '|' || duration_min \
         FROM slots \
         UNION ALL SELECT 'b|' || event_id || '|' || class || '|' || min_count || '|' || max_count \
         FROM class_bounds \
         UNION ALL SELECT 'c|' || id || '|' || status || '|' || updated_at FROM candidatures \
         UNION ALL SELECT 'a|' || candidature_id || '|' || slot_id || '|' || preferred \
         FROM availabilities \
         UNION ALL SELECT 'l|' || hex(secret_hash) || '|' || link_right || '|' || event_id \
         FROM links ORDER BY 1",
    )
    .fetch_all(&state.db.read)
    .await
    .unwrap();
    rows.into_iter().map(|(r,)| r).collect()
}

type EventColumns = (
    String,
    String,
    String,
    String,
    String,
    Option<i64>,
    Option<String>,
    String,
    Option<String>,
);

/// `instance, region, faction_scope, visibility, status, min_cp, discord_url, note, cancelled_at`.
async fn event_row(state: &AppState, e: &Event) -> EventColumns {
    sqlx::query_as(
        "SELECT instance, region, faction_scope, visibility, status, min_cp, discord_url, note, \
         cancelled_at FROM events WHERE id = ?",
    )
    .bind(&e.id)
    .fetch_one(&state.db.read)
    .await
    .unwrap()
}

async fn bounds(state: &AppState, e: &Event) -> Vec<(String, i64, i64)> {
    sqlx::query_as(
        "SELECT class, min_count, max_count FROM class_bounds WHERE event_id = ? ORDER BY class",
    )
    .bind(&e.id)
    .fetch_all(&state.db.read)
    .await
    .unwrap()
}

async fn slots(state: &AppState, e: &Event) -> Vec<(String, String, i64)> {
    sqlx::query_as(
        "SELECT id, starts_at, duration_min FROM slots WHERE event_id = ? ORDER BY starts_at",
    )
    .bind(&e.id)
    .fetch_all(&state.db.read)
    .await
    .unwrap()
}

async fn availabilities(state: &AppState, c: &Cand) -> Vec<String> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT slot_id FROM availabilities WHERE candidature_id = ? ORDER BY slot_id",
    )
    .bind(&c.id)
    .fetch_all(&state.db.read)
    .await
    .unwrap();
    rows.into_iter().map(|(s,)| s).collect()
}

fn slot_starts(v: &Value) -> Vec<String> {
    v["event"]["slots"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["startsAt"].as_str().unwrap().to_owned())
        .collect()
}

#[tokio::test]
async fn settings_replace_every_value_in_open_or_locked() {
    let (state, router) = setup("settings").await;
    for status in ["open", "locked"] {
        let e = create_with(
            &state,
            &router,
            "all",
            json!({ "discordUrl": "https://discord.gg/abcdef", "minCp": 900 }),
        )
        .await;
        set_event_status(&state, &e, status).await;
        let before = bounds(&state, &e).await;
        let (res, text) = post(
            &router,
            &e,
            SETTINGS,
            settings(json!({
                "note": "Vocal prêt",
                "discordUrl": "",
                "visibility": "unlisted",
                "minCp": 1500,
                "classBounds": { "cleric": { "min": 2, "max": 3 } },
            })),
        )
        .await;
        let v = ok(&res, &text, status);
        // La réponse est l'agrégat à jour.
        assert_eq!(v["event"]["note"], "Vocal prêt", "{status}");
        assert_eq!(v["event"]["visibility"], "unlisted", "{status}");
        assert_eq!(v["event"]["minCp"], 1500, "{status}");
        assert_eq!(v["event"]["status"], status, "{status}");
        assert!(v.get("discordUrl").is_none(), "{status}: {text}");
        assert_eq!(v["classBounds"]["cleric"], json!({ "min": 2, "max": 3 }));
        assert_eq!(
            event_row(&state, &e).await,
            (
                "ludra".into(),
                "europe".into(),
                "all".into(),
                "unlisted".into(),
                status.into(),
                Some(1500),
                None,
                "Vocal prêt".into(),
                None,
            ),
            "{status}"
        );
        // Une seule ligne de bornes a changé ; aucune n'est ajoutée ni retirée.
        let after = bounds(&state, &e).await;
        assert_eq!(after.len(), before.len());
        for (b, a) in before.iter().zip(&after) {
            if a.0 == "cleric" {
                assert_eq!(*a, ("cleric".into(), 2, 3));
            } else {
                assert_eq!(a, b);
            }
        }

        // `minCp` absent l'efface ; un lien Discord revient.
        let (res, text) = post(
            &router,
            &e,
            SETTINGS,
            settings(json!({ "discordUrl": "https://discord.gg/xyz" })),
        )
        .await;
        let v = ok(&res, &text, status);
        assert!(v["event"].get("minCp").is_none(), "{text}");
        assert_eq!(v["discordUrl"], "https://discord.gg/xyz");
        let row = event_row(&state, &e).await;
        assert_eq!(
            (row.5, row.6),
            (None, Some("https://discord.gg/xyz".into()))
        );
    }
}

#[tokio::test]
async fn the_raid_fields_freeze_at_the_first_candidature_of_any_status() {
    let (state, router) = setup("frozen").await;
    let e = create(&state, &router, "all").await;
    let c = candidate(&state, &router, &e, "Nyraelle", false).await;
    set_status(&state, &c, "withdrawn").await;
    for (field, value) in [
        ("instance", "mini"),
        ("region", "na_west"),
        ("factionScope", "asmodian"),
    ] {
        let before = dump(&state).await;
        // Le 409 passe avant l'erreur de note.
        let (res, text) = post(
            &router,
            &e,
            SETTINGS,
            settings(json!({ field: value, "note": "voir https://x" })),
        )
        .await;
        assert_refused_fields(
            &res,
            &text,
            StatusCode::CONFLICT,
            "event.field_immutable",
            json!({ field: "event.field_immutable" }),
        );
        assert_eq!(dump(&state).await, before, "{field}");
    }
    // Les mêmes valeurs renvoyées passent, et le reste s'écrit.
    let (res, text) = post(&router, &e, SETTINGS, settings(json!({ "note": "Go" }))).await;
    let v = ok(&res, &text, "unchanged");
    assert_eq!(v["event"]["note"], "Go");
    assert_eq!(event_row(&state, &e).await.7, "Go");
}

#[tokio::test]
async fn without_candidature_the_raid_fields_change_under_the_creation_rules() {
    let (state, router) = setup("raid_fields").await;
    let e = create(&state, &router, "all").await;
    // `mini` prend 4 joueurs : chaque ligne de bornes doit y tenir.
    let (_, text) = send_with(&router, Method::GET, VIEW, &organizer(&e)).await;
    let view: Value = serde_json::from_str(&text).unwrap();
    let fitted: serde_json::Map<String, Value> = view["classBounds"]
        .as_object()
        .unwrap()
        .keys()
        .map(|k| (k.clone(), json!({ "min": 0, "max": 4 })))
        .collect();
    // Portée « all » sur une instance mono-faction : refusée.
    let before = dump(&state).await;
    let (res, text) = post(
        &router,
        &e,
        SETTINGS,
        settings(json!({ "instance": "mini", "classBounds": fitted })),
    )
    .await;
    assert_fields(
        &res,
        &text,
        "event.faction_scope_not_allowed",
        json!({ "factionScope": "event.faction_scope_not_allowed" }),
    );
    // Instance retirée ou inconnue, région fermée ou inconnue : seulement parce qu'elles changent.
    for (patch, path) in [
        (json!({ "instance": "retired" }), "instance"),
        (json!({ "instance": "nope" }), "instance"),
        (json!({ "region": "asia" }), "region"),
        (json!({ "region": "nope" }), "region"),
    ] {
        let (res, text) = post(&router, &e, SETTINGS, settings(patch)).await;
        assert_fields(
            &res,
            &text,
            "reference.unknown",
            json!({ path: "reference.unknown" }),
        );
    }
    // Instance plus petite sans resserrer les bornes : la ligne gardée dépasse.
    let (res, text) = post(
        &router,
        &e,
        SETTINGS,
        settings(json!({ "instance": "mini", "factionScope": "asmodian" })),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST, "{text}");
    let v: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["code"], "event.class_bounds_invalid");
    assert_eq!(dump(&state).await, before);

    let (res, text) = post(
        &router,
        &e,
        SETTINGS,
        settings(json!({
            "instance": "mini",
            "region": "na_west",
            "factionScope": "asmodian",
            "classBounds": fitted,
        })),
    )
    .await;
    let v = ok(&res, &text, "change");
    assert_eq!(v["event"]["instance"], "mini");
    assert_eq!(v["event"]["region"], "na_west");
    assert_eq!(v["event"]["factionScope"], "asmodian");
    let row = event_row(&state, &e).await;
    assert_eq!(
        (row.0.as_str(), row.1.as_str(), row.2.as_str()),
        ("mini", "na_west", "asmodian")
    );
    assert!(bounds(&state, &e).await.iter().all(|b| b.2 == 4));
}

#[tokio::test]
async fn invalid_settings_write_nothing() {
    let (state, router) = setup("invalid").await;
    let e = create(&state, &router, "all").await;
    let before = dump(&state).await;
    for (patch, code, fields) in [
        (
            json!({ "classBounds": { "cleric": { "min": 3, "max": 2 } } }),
            "event.class_bounds_invalid",
            json!({ "classBounds.cleric.min": "event.class_bounds_invalid" }),
        ),
        (
            json!({ "classBounds": { "cleric": { "min": 0, "max": 11 } } }),
            "event.class_bounds_invalid",
            json!({ "classBounds.cleric.max": "event.class_bounds_invalid" }),
        ),
        (
            json!({ "classBounds": { "necromancer": { "min": 0, "max": 1 } } }),
            "reference.unknown",
            json!({ "classBounds.necromancer": "reference.unknown" }),
        ),
        (
            json!({ "note": "a".repeat(281) }),
            "event.note_invalid",
            json!({ "note": "event.note_invalid" }),
        ),
        (
            json!({ "discordUrl": "https://evil.example/x" }),
            "event.discord_url_invalid",
            json!({ "discordUrl": "event.discord_url_invalid" }),
        ),
        (
            json!({ "minCp": 1_000_000 }),
            "event.min_cp_out_of_range",
            json!({ "minCp": "event.min_cp_out_of_range" }),
        ),
    ] {
        let (res, text) = post(&router, &e, SETTINGS, settings(patch)).await;
        assert_fields(&res, &text, code, fields);
        assert_eq!(dump(&state).await, before, "{code}");
    }
    // Corps incomplet ou de mauvaise forme : illisible.
    for raw in [
        "{".to_owned(),
        "{}".to_owned(),
        json!({ "note": "x" }).to_string(),
        {
            let mut b = settings(json!({}));
            b.as_object_mut().unwrap().remove("classBounds");
            b.to_string()
        },
        settings(json!({ "visibility": "hidden" })).to_string(),
        settings(json!({ "minCp": 1.5 })).to_string(),
    ] {
        let (res, text) = post_raw_with(&router, SETTINGS, &raw, &organizer(&e)).await;
        assert_problem(&res, &text, "form.malformed", &raw);
    }
    assert_eq!(dump(&state).await, before);
}

#[tokio::test]
async fn add_a_slot_in_open() {
    let (state, router) = setup("add").await;
    let e = create(&state, &router, "all").await;
    let (res, text) = post(
        &router,
        &e,
        SLOTS,
        json!({ "startsAt": "2026-11-13T20:00:00Z" }),
    )
    .await;
    let v = ok(&res, &text, "add");
    assert_eq!(
        slot_starts(&v),
        [
            "2026-11-11T20:00:00Z",
            "2026-11-12T20:00:00Z",
            "2026-11-13T20:00:00Z"
        ]
    );
    let stored = slots(&state, &e).await;
    assert_eq!(stored.len(), 3);
    // Durée de l'instance par défaut, id de l'`IdGenerator`.
    assert_eq!(stored[2].1, "2026-11-13T20:00:00Z");
    assert_eq!(stored[2].2, 60);
    assert_eq!(stored[2].0.len(), 22);
    assert_eq!(v["event"]["slots"][2]["id"], stored[2].0);

    let (res, text) = post(
        &router,
        &e,
        SLOTS,
        json!({ "startsAt": "2026-11-10T18:30:00Z", "durationMin": 90 }),
    )
    .await;
    let v = ok(&res, &text, "with duration");
    assert_eq!(v["event"]["slots"][0]["startsAt"], "2026-11-10T18:30:00Z");
    assert_eq!(v["event"]["slots"][0]["durationMin"], 90);
}

#[tokio::test]
async fn add_a_slot_on_a_retired_instance_takes_its_duration() {
    let (state, router) = setup("add_retired").await;
    let e = create(&state, &router, "all").await;
    // Instance retirée de la référence depuis (`enabled = false`) : l'ajout ne la pose pas, AD-6.
    sql(
        &state,
        "UPDATE events SET instance = 'retired' WHERE id = ?",
        &[&e.id],
    )
    .await;
    let (res, text) = post(
        &router,
        &e,
        SLOTS,
        json!({ "startsAt": "2026-11-13T20:00:00Z" }),
    )
    .await;
    let v = ok(&res, &text, "retired");
    assert_eq!(v["event"]["slots"][2]["startsAt"], "2026-11-13T20:00:00Z");
    assert_eq!(v["event"]["slots"][2]["durationMin"], 75);
    assert_eq!(slots(&state, &e).await[2].2, 75);
}

#[tokio::test]
async fn an_unlisted_raid_is_noindex_on_its_public_page() {
    let (state, router) = setup("noindex").await;
    let e = create(&state, &router, "all").await;
    let page = format!("/e/{}", e.id);
    let (res, _) = send(&router, Method::GET, &page).await;
    assert_eq!(res.status(), StatusCode::OK);
    assert!(res.headers().get("x-robots-tag").is_none());
    let (res, text) = post(
        &router,
        &e,
        SETTINGS,
        settings(json!({ "visibility": "unlisted" })),
    )
    .await;
    ok(&res, &text, "unlisted");
    let (res, body) = send(&router, Method::GET, &page).await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(body, FIXTURE_INDEX);
    assert_eq!(header(&res, "x-robots-tag"), "noindex");
    // Relisté : la page redevient indexable.
    let (res, text) = post(&router, &e, SETTINGS, settings(json!({}))).await;
    ok(&res, &text, "listed");
    let (res, _) = send(&router, Method::GET, &page).await;
    assert!(res.headers().get("x-robots-tag").is_none());
}

#[tokio::test]
async fn an_added_slot_follows_the_creation_rules() {
    let (state, router) = setup("add_rules").await;
    let e = create(&state, &router, "all").await;
    let before = dump(&state).await;
    for (body, code, fields) in [
        (
            json!({ "startsAt": "2026-11-06T09:00:00Z" }),
            "slot.in_past",
            json!({ "startsAt": "slot.in_past" }),
        ),
        (
            json!({ "startsAt": NOW }),
            "slot.in_past",
            json!({ "startsAt": "slot.in_past" }),
        ),
        (
            json!({ "startsAt": "2026-11-11T20:00:00Z" }),
            "slot.duplicate",
            json!({ "startsAt": "slot.duplicate" }),
        ),
        (
            json!({ "startsAt": "2026-11-13T20:00:00Z", "durationMin": 721 }),
            "slot.duration_out_of_range",
            json!({ "durationMin": "slot.duration_out_of_range" }),
        ),
        (
            json!({ "startsAt": "2026-11-13T20:00:00Z", "durationMin": 14 }),
            "slot.duration_out_of_range",
            json!({ "durationMin": "slot.duration_out_of_range" }),
        ),
    ] {
        let (res, text) = post(&router, &e, SLOTS, body).await;
        assert_fields(&res, &text, code, fields);
        assert_eq!(dump(&state).await, before, "{code}");
    }
    for raw in [
        "{}",
        r#"{"startsAt":"2026-11-13 20:00"}"#,
        r#"{"startsAt":"2026-11-13T20:00:00+01:00"}"#,
    ] {
        let (res, text) = post_raw_with(&router, SLOTS, raw, &organizer(&e)).await;
        assert_problem(&res, &text, "form.malformed", raw);
    }
    assert_eq!(dump(&state).await, before);
}

#[tokio::test]
async fn at_most_fourteen_slots_past_ones_included() {
    let (state, router) = setup("fourteen").await;
    let e = create(&state, &router, "all").await;
    // Le premier créneau passe dans le passé : il compte encore.
    sql(
        &state,
        "UPDATE slots SET starts_at = ? WHERE id = ?",
        &["2026-11-01T20:00:00Z", &e.slots[0]],
    )
    .await;
    for day in 13..25 {
        let (res, text) = post(
            &router,
            &e,
            SLOTS,
            json!({ "startsAt": format!("2026-11-{day}T20:00:00Z") }),
        )
        .await;
        ok(&res, &text, &day.to_string());
    }
    assert_eq!(slots(&state, &e).await.len(), 14);
    let before = dump(&state).await;
    let (res, text) = post(
        &router,
        &e,
        SLOTS,
        json!({ "startsAt": "2026-11-28T20:00:00Z" }),
    )
    .await;
    assert_problem(&res, &text, "slot.count_out_of_range", "15th");
    assert_eq!(dump(&state).await, before);
}

#[tokio::test]
async fn remove_a_slot_cascades_its_availabilities_and_keeps_every_status() {
    let (state, router) = setup("remove").await;
    let e = create(&state, &router, "all").await;
    let both = candidate(&state, &router, &e, "Nyraelle", false).await;
    let accepted = candidate(&state, &router, &e, "Kaelthorn", true).await;
    // Cette candidature n'a coché que le créneau supprimé : elle reste, sans disponibilité.
    let only = candidate_with(
        &state,
        &router,
        &e,
        json!({
            "pseudo": "Tomasquin",
            "availabilities": [{ "slotId": e.slots[0], "preferred": true }],
        }),
        false,
    )
    .await;
    set_status(&state, &only, "rejected").await;
    let statuses = |v: &Value| -> Vec<(String, String)> {
        v["candidatures"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| {
                (
                    c["id"].as_str().unwrap().to_owned(),
                    c["status"].as_str().unwrap().to_owned(),
                )
            })
            .collect()
    };
    let (_, text) = send_with(&router, Method::GET, VIEW, &organizer(&e)).await;
    let before: Value = serde_json::from_str(&text).unwrap();
    let rows_before: Vec<(String, String, String)> =
        sqlx::query_as("SELECT id, status, updated_at FROM candidatures ORDER BY id")
            .fetch_all(&state.db.read)
            .await
            .unwrap();

    let (res, text) = post(&router, &e, REMOVAL, json!({ "slotId": e.slots[0] })).await;
    let v = ok(&res, &text, "remove");
    assert_eq!(slot_starts(&v), ["2026-11-12T20:00:00Z"]);
    assert_eq!(slots(&state, &e).await.len(), 1);
    // Les disponibilités du créneau sont parties, les autres restent.
    assert_eq!(availabilities(&state, &both).await, [e.slots[1].clone()]);
    assert_eq!(
        availabilities(&state, &accepted).await,
        [e.slots[1].clone()]
    );
    assert!(availabilities(&state, &only).await.is_empty());
    // Aucun statut ne bouge, aucune candidature ne disparaît, `updated_at` compris.
    assert_eq!(statuses(&v), statuses(&before));
    let rows_after: Vec<(String, String, String)> =
        sqlx::query_as("SELECT id, status, updated_at FROM candidatures ORDER BY id")
            .fetch_all(&state.db.read)
            .await
            .unwrap();
    assert_eq!(rows_after, rows_before);
    let kept = v["candidatures"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == only.id.as_str())
        .unwrap();
    assert_eq!(kept["availabilities"], json!([]));
    // La vue organisateur ne mentionne plus le créneau nulle part.
    assert!(!text.contains(&e.slots[0]));
    // La lecture publique non plus.
    let (res, public) = send(&router, Method::GET, &format!("/api/events/{}", e.id)).await;
    assert_eq!(res.status(), StatusCode::OK, "{public}");
    let p: Value = serde_json::from_str(&public).unwrap();
    assert_eq!(
        slot_starts(&json!({ "event": p })),
        ["2026-11-12T20:00:00Z"]
    );
    assert!(!public.contains(&e.slots[0]));
}

#[tokio::test]
async fn a_past_slot_can_be_removed_but_never_the_last_one() {
    let (state, router) = setup("remove_rules").await;
    let e = create(&state, &router, "all").await;
    let other = create(&state, &router, "all").await;
    sql(
        &state,
        "UPDATE slots SET starts_at = ? WHERE id = ?",
        &["2026-11-01T20:00:00Z", &e.slots[0]],
    )
    .await;
    let before = dump(&state).await;
    for id in [other.slots[0].as_str(), &"x".repeat(22), "", "short"] {
        let (res, text) = post(&router, &e, REMOVAL, json!({ "slotId": id })).await;
        assert_problem(&res, &text, "slot.not_found", id);
    }
    for raw in ["{}", r#"{"slotId":3}"#, "{"] {
        let (res, text) = post_raw_with(&router, REMOVAL, raw, &organizer(&e)).await;
        assert_problem(&res, &text, "form.malformed", raw);
    }
    assert_eq!(dump(&state).await, before);

    let (res, text) = post(&router, &e, REMOVAL, json!({ "slotId": e.slots[0] })).await;
    ok(&res, &text, "past slot");
    let before = dump(&state).await;
    let (res, text) = post(&router, &e, REMOVAL, json!({ "slotId": e.slots[1] })).await;
    assert_problem(&res, &text, "slot.count_out_of_range", "last");
    assert_eq!(dump(&state).await, before);
}

#[tokio::test]
async fn slots_do_not_move_on_a_locked_raid_but_settings_do() {
    let (state, router) = setup("locked").await;
    let e = create(&state, &router, "all").await;
    set_event_status(&state, &e, "locked").await;
    let before = dump(&state).await;
    // L'état passe avant toute règle de champ ou de créneau.
    for (uri, body) in [
        (SLOTS, json!({ "startsAt": "2026-11-13T20:00:00Z" })),
        (SLOTS, json!({ "startsAt": "2020-01-01T00:00:00Z" })),
        (REMOVAL, json!({ "slotId": e.slots[0] })),
        (REMOVAL, json!({ "slotId": "x".repeat(22) })),
    ] {
        let (res, text) = post(&router, &e, uri, body).await;
        assert_problem(&res, &text, "event.locked", uri);
    }
    assert_eq!(dump(&state).await, before);
    let (res, text) = post(&router, &e, SETTINGS, settings(json!({ "note": "Go" }))).await;
    ok(&res, &text, "settings in locked");
}

#[tokio::test]
async fn cancel_from_open_or_locked_changes_nothing_else() {
    let (state, router) = setup("cancel").await;
    for status in ["open", "locked"] {
        let e = create(&state, &router, "all").await;
        candidate(&state, &router, &e, "Nyraelle", false).await;
        set_event_status(&state, &e, status).await;
        let before: Vec<String> = dump(&state)
            .await
            .into_iter()
            .filter(|r| !r.starts_with(&format!("e|{}", e.id)))
            .collect();
        let (res, text) = post(&router, &e, CANCELLATION, json!({})).await;
        let v = ok(&res, &text, status);
        assert_eq!(v["event"]["status"], "cancelled", "{status}");
        let row = event_row(&state, &e).await;
        assert_eq!(row.4, "cancelled", "{status}");
        assert_eq!(row.8.as_deref(), Some(NOW), "{status}");
        // Aucune autre ligne ne change : créneaux, bornes, candidatures, liens.
        let after: Vec<String> = dump(&state)
            .await
            .into_iter()
            .filter(|r| !r.starts_with(&format!("e|{}", e.id)))
            .collect();
        assert_eq!(after, before, "{status}");
        // Les liens restent valides : la vue se lit toujours.
        let (res, _) = send_with(&router, Method::GET, VIEW, &organizer(&e)).await;
        assert_eq!(res.status(), StatusCode::OK);
    }
}

#[tokio::test]
async fn a_cancelled_event_refuses_every_write_first() {
    let (state, router) = setup("closed").await;
    let e = create(&state, &router, "all").await;
    candidate(&state, &router, &e, "Nyraelle", false).await;
    let (res, text) = post(&router, &e, CANCELLATION, json!({})).await;
    ok(&res, &text, "cancel");
    let before = dump(&state).await;
    for (uri, body) in [
        (CANCELLATION, json!({})),
        (SETTINGS, settings(json!({ "note": "Go" }))),
        // Le refus d'état passe avant le champ figé et les erreurs de champ.
        (
            SETTINGS,
            settings(json!({ "region": "na_west", "note": "https://x" })),
        ),
        (SLOTS, json!({ "startsAt": "2026-11-13T20:00:00Z" })),
        (SLOTS, json!({ "startsAt": "2020-01-01T00:00:00Z" })),
        (REMOVAL, json!({ "slotId": e.slots[0] })),
        (REMOVAL, json!({ "slotId": "x".repeat(22) })),
    ] {
        let (res, text) = post(&router, &e, uri, body).await;
        assert_problem(&res, &text, "event.closed", uri);
    }
    assert_eq!(dump(&state).await, before);
}

#[tokio::test]
async fn a_raid_cancelled_from_o_reads_cancelled_and_refuses_writes_everywhere() {
    let (state, router) = setup("acceptance").await;
    let e = create(&state, &router, "all").await;
    let c = candidate(&state, &router, &e, "Nyraelle", false).await;
    let (res, text) = post(&router, &e, CANCELLATION, json!({})).await;
    ok(&res, &text, "cancel");

    let (res, text) = send(&router, Method::GET, &format!("/api/events/{}", e.id)).await;
    assert_eq!(res.status(), StatusCode::OK, "{text}");
    let v: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["status"], "cancelled");
    let participant = [("x-link-secret", c.secret.as_str())];
    let (res, text) = send_with(&router, Method::GET, "/api/participant", &participant).await;
    assert_eq!(res.status(), StatusCode::OK, "{text}");
    let v: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["event"]["status"], "cancelled");
    let (res, _) = send_with(
        &router,
        Method::GET,
        "/api/join",
        &[("x-link-secret", e.join.as_str())],
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);

    let before = dump(&state).await;
    let update = with(nominal(&state, &e), json!({ "cp": 1600 }));
    let (res, text) = post_json_with(
        &router,
        "/api/participant/candidature",
        &update,
        &participant,
    )
    .await;
    assert_problem(&res, &text, "event.closed", "update");
    let (res, text) = post_json_with(
        &router,
        "/api/participant/withdrawal",
        &json!({}),
        &participant,
    )
    .await;
    assert_problem(&res, &text, "event.closed", "withdraw");
    let body = nominal(&state, &e);
    let (res, text) = post_json(
        &router,
        &format!("/api/events/{}/candidatures", e.id),
        &body,
    )
    .await;
    assert_problem(&res, &text, "event.closed", "public candidature");
    let (res, text) = post_json_with(
        &router,
        "/api/join/candidatures",
        &body,
        &[("x-link-secret", e.join.as_str())],
    )
    .await;
    assert_problem(&res, &text, "event.closed", "direct candidature");
    let (res, text) = post_json_with(
        &router,
        "/api/organizer/candidature-status",
        &json!({ "candidatureId": c.id, "status": "accepted" }),
        &organizer(&e),
    )
    .await;
    assert_problem(&res, &text, "event.closed", "decision");
    assert_eq!(dump(&state).await, before);
}

#[tokio::test]
async fn only_the_organizer_secret_opens_the_commands() {
    let (state, router) = setup("rights").await;
    let e = create(&state, &router, "all").await;
    let c = candidate(&state, &router, &e, "Nyraelle", false).await;
    let unknown = "A".repeat(43);
    let before = dump(&state).await;
    let bodies = [
        (SETTINGS, settings(json!({ "note": "Go" }))),
        (SLOTS, json!({ "startsAt": "2026-11-13T20:00:00Z" })),
        (REMOVAL, json!({ "slotId": e.slots[0] })),
        (CANCELLATION, json!({})),
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
    // Événement purgé, ligne du lien restée : le lien ne mène plus à rien.
    sql(&state, "DELETE FROM events WHERE id = ?", &[&e.id]).await;
    for (uri, body) in &bodies {
        let (res, text) = post(&router, &e, uri, body.clone()).await;
        assert_problem(&res, &text, "auth.not_found", uri);
    }
}

#[tokio::test]
async fn the_commands_use_the_write_quota() {
    let (state, router) = setup_with(
        "quota",
        &test_config(&[
            ("RATE_CREATE", "100/h"),
            ("RATE_CANDIDATURE", "100/h"),
            ("RATE_WRITE", "1/h"),
        ]),
    )
    .await;
    let e = create(&state, &router, "all").await;
    let (res, text) = post(&router, &e, SETTINGS, settings(json!({ "note": "Go" }))).await;
    ok(&res, &text, "first write");
    let before = dump(&state).await;
    for (uri, body) in [
        (SETTINGS, settings(json!({ "note": "Again" }))),
        (SLOTS, json!({ "startsAt": "2026-11-13T20:00:00Z" })),
        (REMOVAL, json!({ "slotId": e.slots[0] })),
        (CANCELLATION, json!({})),
    ] {
        let (res, text) = post(&router, &e, uri, body).await;
        assert_problem(&res, &text, "limit.rate_limited", uri);
    }
    assert_eq!(dump(&state).await, before);
    // La lecture garde son propre quota.
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
async fn no_secret_nor_free_text_in_the_logs() {
    let (state, router) = setup("logs").await;
    let e = create(&state, &router, "all").await;
    let requests = |note: &'static str, start: &'static str| {
        let (router, e) = (&router, &e);
        async move {
            let (res, _) = post(
                router,
                e,
                SETTINGS,
                settings(json!({ "note": note, "discordUrl": "https://discord.gg/secretroom" })),
            )
            .await;
            assert_eq!(res.status(), StatusCode::OK);
            let (res, text) = post(router, e, SLOTS, json!({ "startsAt": start })).await;
            assert_eq!(res.status(), StatusCode::OK, "{text}");
            let v: Value = serde_json::from_str(&text).unwrap();
            let added = v["event"]["slots"]
                .as_array()
                .unwrap()
                .iter()
                .find(|s| s["startsAt"] == start)
                .unwrap()["id"]
                .as_str()
                .unwrap()
                .to_owned();
            let (res, _) = post(router, e, REMOVAL, json!({ "slotId": added })).await;
            assert_eq!(res.status(), StatusCode::OK);
            let (res, _) = post(router, e, REMOVAL, json!({ "slotId": "x".repeat(22) })).await;
            assert_eq!(res.status(), StatusCode::NOT_FOUND);
            let (res, _) = post_raw_with(router, SETTINGS, "{", &organizer(e)).await;
            assert_eq!(res.status(), StatusCode::BAD_REQUEST);
        }
    };
    // Même gabarit que `candidature_submit::no_secret_nor_pseudo_in_the_logs` : premier passage sous
    // un abonné muet, pour que chaque callsite soit enregistré avant le `set_default` qui les
    // réévalue tous.
    {
        let mute = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .with_writer(std::io::sink)
            .finish();
        let _mute = tracing::subscriber::set_default(mute);
        requests("Premier passage", "2026-11-13T20:00:00Z").await;
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
    requests("Vocal prêt ce soir", "2026-11-14T20:00:00Z").await;
    let (res, _) = post(&router, &e, CANCELLATION, json!({})).await;
    assert_eq!(res.status(), StatusCode::OK);
    drop(guard);
    let log = String::from_utf8(captured.0.lock().unwrap().clone()).unwrap();
    for line in [
        "event settings updated",
        "slot added",
        "slot removed",
        "event cancelled",
    ] {
        assert!(log.contains(line), "{line} missing in {log}");
    }
    assert!(log.contains(&e.id), "{log}");
    for leak in [
        e.organizer.as_str(),
        e.join.as_str(),
        "Vocal prêt ce soir",
        "secretroom",
        TEST_CLIENT_IP,
    ] {
        assert!(!log.contains(leak), "{leak} in {log}");
    }
}
