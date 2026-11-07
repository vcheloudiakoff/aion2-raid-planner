//! Matrice HTTP de la story 2-3 : `GET /api/participant`, `POST /api/participant/candidature` et
//! `POST /api/participant/withdrawal`, sous le seul secret participant. Vue limitée à sa propre
//! candidature, règles d'état, disponibilités passées gardées, quota `write`, journaux muets.

mod common;

use std::sync::{Arc, Mutex};

use axum::http::{Method, StatusCode};
use common::candidatures::*;
use common::*;
use serde_json::{Value, json};
use server::app::AppState;
use server::config::Config;

const VIEW: &str = "/api/participant";
const UPDATE: &str = "/api/participant/candidature";
const WITHDRAW: &str = "/api/participant/withdrawal";
/// Instant antérieur à `NOW`, posé sur `updated_at` pour voir la commande l'écrire.
const BEFORE: &str = "2026-11-01T00:00:00Z";

async fn setup_with(name: &str, config: &Config) -> (AppState, axum::Router) {
    candidature_setup(&format!("participant_{name}"), config).await
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

/// Candidature déposée par l'API ; `direct` = par le lien `/j/` (`accepted`).
struct Cand {
    id: String,
    secret: String,
}

async fn candidate(
    state: &AppState,
    router: &axum::Router,
    e: &Event,
    pseudo: &str,
    direct: bool,
) -> Cand {
    let body = with(nominal(state, e), json!({ "pseudo": pseudo }));
    let (res, text) = if direct {
        post_json_with(
            router,
            "/api/join/candidatures",
            &body,
            &[("x-link-secret", &e.join)],
        )
        .await
    } else {
        post_json(router, &format!("/api/events/{}/candidatures", e.id), &body).await
    };
    assert_eq!(res.status(), StatusCode::CREATED, "{text}");
    let v: Value = serde_json::from_str(&text).unwrap();
    let c = Cand {
        id: v["candidatureId"].as_str().unwrap().into(),
        secret: v["participantSecret"].as_str().unwrap().into(),
    };
    sqlx::query("UPDATE candidatures SET updated_at = ? WHERE id = ?")
        .bind(BEFORE)
        .bind(&c.id)
        .execute(&state.db.write)
        .await
        .unwrap();
    c
}

async fn sql(state: &AppState, query: &str, binds: &[&str]) {
    let mut q = sqlx::query(query);
    for b in binds {
        q = q.bind(*b);
    }
    q.execute(&state.db.write).await.unwrap();
}

async fn set_event_status(state: &AppState, e: &Event, status: &str) {
    sql(
        state,
        "UPDATE events SET status = ? WHERE id = ?",
        &[status, &e.id],
    )
    .await;
}

async fn set_status(state: &AppState, c: &Cand, status: &str) {
    sql(
        state,
        "UPDATE candidatures SET status = ? WHERE id = ?",
        &[status, &c.id],
    )
    .await;
}

fn secret(c: &Cand) -> [(&'static str, &str); 1] {
    [("x-link-secret", c.secret.as_str())]
}

async fn read(router: &axum::Router, c: &Cand) -> (StatusCode, Value, String) {
    let (res, body) = send_with(router, Method::GET, VIEW, &secret(c)).await;
    assert_eq!(header(&res, "cache-control"), "no-store");
    let status = res.status();
    (
        status,
        serde_json::from_str(&body).unwrap_or(Value::Null),
        body,
    )
}

/// Corps de modification : CP 2000, second créneau seul, non préféré. Sans jeton ni honeypot.
fn edit(e: &Event) -> Value {
    json!({
        "pseudo": "Nyraelle",
        "server": "israphel",
        "class": "cleric",
        "cp": 2000,
        "availabilities": [{ "slotId": e.slots[1], "preferred": false }],
    })
}

/// Ce qu'une commande peut changer : contenu, statut, `updated_at` et disponibilités.
type Row = (String, String, String, i64, String, String, String);

async fn row(state: &AppState, c: &Cand) -> (Row, Vec<(String, i64)>) {
    let row: Row = sqlx::query_as(
        "SELECT pseudo, server, class, cp, status, origin, updated_at FROM candidatures WHERE id = ?",
    )
    .bind(&c.id)
    .fetch_one(&state.db.read)
    .await
    .unwrap();
    let avail: Vec<(String, i64)> = sqlx::query_as(
        "SELECT slot_id, preferred FROM availabilities WHERE candidature_id = ? ORDER BY slot_id",
    )
    .bind(&c.id)
    .fetch_all(&state.db.read)
    .await
    .unwrap();
    (row, avail)
}

fn sorted_keys(v: &Value) -> Vec<String> {
    let mut keys: Vec<_> = v.as_object().unwrap().keys().cloned().collect();
    keys.sort_unstable();
    keys
}

#[tokio::test]
async fn read_shows_only_its_own_candidature_and_the_public_event() {
    let (state, router) = setup("read").await;
    let e = create_with(
        &state,
        &router,
        "all",
        json!({ "discordUrl": "https://discord.gg/abcdef" }),
    )
    .await;
    let mine = candidate(&state, &router, &e, "Nyraelle", false).await;
    let other = candidate(&state, &router, &e, "Kaelthorn", true).await;

    let (status, v, body) = read(&router, &mine).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        sorted_keys(&v),
        [
            "availabilities",
            "candidatureId",
            "class",
            "cp",
            "event",
            "eventId",
            "pseudo",
            "server",
            "status"
        ]
    );
    assert_eq!(v["eventId"], e.id);
    assert_eq!(v["candidatureId"], mine.id);
    assert_eq!(v["pseudo"], "Nyraelle");
    assert_eq!(v["server"], "israphel");
    assert_eq!(v["class"], "cleric");
    assert_eq!(v["cp"], 1500);
    assert_eq!(v["status"], "pending");
    assert_eq!(
        v["availabilities"],
        json!([
            { "slotId": e.slots[0], "preferred": true },
            { "slotId": e.slots[1], "preferred": true },
        ])
    );
    // L'événement imbriqué est le `PublicEvent`, sans nom ni Discord.
    assert_eq!(v["event"]["id"], e.id);
    assert_eq!(v["event"]["countsByClass"]["cleric"], 2);
    assert!(v["event"].get("candidatures").is_none());
    for leak in ["Kaelthorn", other.id.as_str(), "discord", "discordUrl"] {
        assert!(!body.contains(leak), "{leak} in {body}");
    }
}

#[tokio::test]
async fn discord_url_only_for_an_accepted_candidature_of_an_event_that_has_one() {
    let (state, router) = setup("discord").await;
    let e = create_with(
        &state,
        &router,
        "all",
        json!({ "discordUrl": "https://discord.gg/abcdef" }),
    )
    .await;
    let accepted = candidate(&state, &router, &e, "Nyraelle", true).await;
    let (_, v, _) = read(&router, &accepted).await;
    assert_eq!(v["status"], "accepted");
    assert_eq!(v["discordUrl"], "https://discord.gg/abcdef");

    for status in ["pending", "rejected", "withdrawn"] {
        set_status(&state, &accepted, status).await;
        let (_, v, body) = read(&router, &accepted).await;
        assert!(v.get("discordUrl").is_none(), "{status}: {body}");
    }

    let bare = create(&state, &router, "all").await;
    let c = candidate(&state, &router, &bare, "Tomasquin", true).await;
    let (_, v, body) = read(&router, &c).await;
    assert_eq!(v["status"], "accepted");
    assert!(v.get("discordUrl").is_none(), "{body}");
}

#[tokio::test]
async fn only_the_participant_secret_opens_the_three_routes() {
    let (state, router) = setup("rights").await;
    let e = create(&state, &router, "all").await;
    let c = candidate(&state, &router, &e, "Nyraelle", false).await;
    let unknown = "A".repeat(43);
    let before = row(&state, &c).await;
    for (headers, code) in [
        (vec![], "auth.missing"),
        (vec![("x-link-secret", unknown.as_str())], "auth.not_found"),
        (vec![("x-link-secret", e.join.as_str())], "auth.wrong_right"),
        (
            vec![("x-link-secret", e.organizer.as_str())],
            "auth.wrong_right",
        ),
    ] {
        let (res, body) = send_with(&router, Method::GET, VIEW, &headers).await;
        assert_problem(&res, &body, code, &format!("GET {code}"));
        for (uri, payload) in [(UPDATE, edit(&e)), (WITHDRAW, json!({}))] {
            let (res, body) = post_json_with(&router, uri, &payload, &headers).await;
            assert_problem(&res, &body, code, &format!("{uri} {code}"));
        }
    }
    assert_eq!(row(&state, &c).await, before);
    // Ni l'id ni le secret dans l'URL : une query string ne vaut rien.
    let (res, body) = send(&router, Method::GET, &format!("{VIEW}?secret={}", c.secret)).await;
    assert_problem(&res, &body, "auth.missing", "query");
}

#[tokio::test]
async fn update_replaces_the_content_and_keeps_the_status() {
    let (state, router) = setup("update").await;
    let e = create(&state, &router, "all").await;
    let c = candidate(&state, &router, &e, "Nyraelle", true).await;
    let body = with(
        edit(&e),
        json!({ "pseudo": "Nyra", "server": "siel", "class": "templar" }),
    );
    let (res, text) = post_json_with(&router, UPDATE, &body, &secret(&c)).await;
    assert_eq!(res.status(), StatusCode::OK, "{text}");
    assert_eq!(header(&res, "cache-control"), "no-store");
    let v: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["status"], "accepted");
    assert_eq!(v["pseudo"], "Nyra");
    assert_eq!(v["cp"], 2000);
    assert_eq!(
        v["availabilities"],
        json!([{ "slotId": e.slots[1], "preferred": false }])
    );
    // La réponse est l'agrégat à jour : les comptes publics suivent la nouvelle classe.
    assert_eq!(v["event"]["countsByClass"]["templar"], 1);
    assert_eq!(v["event"]["countsByClass"]["cleric"], 0);
    let (row, avail) = row(&state, &c).await;
    assert_eq!(
        row,
        (
            "Nyra".into(),
            "siel".into(),
            "templar".into(),
            2000,
            "accepted".into(),
            "direct".into(),
            NOW.into()
        )
    );
    assert_eq!(avail, [(e.slots[1].clone(), 0)]);
}

#[tokio::test]
async fn a_rejected_candidature_is_still_editable_while_open() {
    let (state, router) = setup("rejected_edit").await;
    let e = create(&state, &router, "all").await;
    let c = candidate(&state, &router, &e, "Nyraelle", false).await;
    set_status(&state, &c, "rejected").await;
    let (res, text) = post_json_with(&router, UPDATE, &edit(&e), &secret(&c)).await;
    assert_eq!(res.status(), StatusCode::OK, "{text}");
    let ((_, _, _, cp, status, origin, updated_at), _) = row(&state, &c).await;
    assert_eq!(
        (cp, status.as_str(), origin.as_str(), updated_at.as_str()),
        (2000, "rejected", "public", NOW)
    );
}

#[tokio::test]
async fn update_of_a_locked_event_is_refused() {
    let (state, router) = setup("locked").await;
    let e = create(&state, &router, "all").await;
    let c = candidate(&state, &router, &e, "Nyraelle", true).await;
    set_event_status(&state, &e, "locked").await;
    let before = row(&state, &c).await;
    let (res, body) = post_json_with(&router, UPDATE, &edit(&e), &secret(&c)).await;
    assert_problem(&res, &body, "event.locked", "locked");
    assert_eq!(row(&state, &c).await, before);
}

#[tokio::test]
async fn invalid_update_writes_nothing_and_names_its_fields() {
    let (state, router) = setup("invalid").await;
    let e = create(&state, &router, "all").await;
    let other = create(&state, &router, "all").await;
    let c = candidate(&state, &router, &e, "Nyraelle", false).await;
    let before = row(&state, &c).await;
    let cases = [
        (
            json!({ "server": "far_away" }),
            "candidature.server_out_of_region",
            json!({ "server": "candidature.server_out_of_region" }),
        ),
        (
            json!({ "availabilities": [] }),
            "candidature.no_availability",
            json!({ "availabilities": "candidature.no_availability" }),
        ),
        (
            json!({ "availabilities": [{ "slotId": other.slots[0] }] }),
            "candidature.slot_unknown",
            json!({ "availabilities.0.slotId": "candidature.slot_unknown" }),
        ),
        (
            json!({ "pseudo": "a".repeat(17), "cp": 1_000_000 }),
            "candidature.pseudo_invalid",
            json!({
                "pseudo": "candidature.pseudo_invalid",
                "cp": "candidature.cp_out_of_range",
            }),
        ),
    ];
    for (patch, code, fields) in cases {
        let (res, text) =
            post_json_with(&router, UPDATE, &with(edit(&e), patch), &secret(&c)).await;
        assert_fields(&res, &text, code, fields);
    }
    for raw in ["{", r#"{"pseudo":"Nyra"}"#] {
        let (res, text) = post_raw_with(&router, UPDATE, raw, &secret(&c)).await;
        assert_problem(&res, &text, "form.malformed", raw);
    }
    assert_eq!(row(&state, &c).await, before);
}

#[tokio::test]
async fn a_past_availability_is_kept_and_counts_as_one() {
    let (state, router) = setup("past").await;
    let e = create(&state, &router, "all").await;
    let c = candidate(&state, &router, &e, "Nyraelle", false).await;
    // Créneau déjà commencé, coché avant qu'il ne passe.
    let past = "p".repeat(22);
    sqlx::query("INSERT INTO slots (id, event_id, starts_at, duration_min) VALUES (?, ?, ?, 60)")
        .bind(&past)
        .bind(&e.id)
        .bind(ago(600))
        .execute(&state.db.write)
        .await
        .unwrap();
    sql(
        &state,
        "INSERT INTO availabilities (candidature_id, slot_id, preferred) VALUES (?, ?, 1)",
        &[&c.id, &past],
    )
    .await;

    let (res, text) = post_json_with(&router, UPDATE, &edit(&e), &secret(&c)).await;
    assert_eq!(res.status(), StatusCode::OK, "{text}");
    let mut expected = vec![(past.clone(), 1), (e.slots[1].clone(), 0)];
    expected.sort();
    assert_eq!(row(&state, &c).await.1, expected);
    let v: Value = serde_json::from_str(&text).unwrap();
    // Triées par début : le créneau passé d'abord.
    assert_eq!(
        v["availabilities"][0],
        json!({ "slotId": past, "preferred": true })
    );

    // Aucun créneau à venir : la disponibilité passée suffit.
    let body = with(edit(&e), json!({ "availabilities": [] }));
    let (res, text) = post_json_with(&router, UPDATE, &body, &secret(&c)).await;
    assert_eq!(res.status(), StatusCode::OK, "{text}");
    assert_eq!(row(&state, &c).await.1, [(past.clone(), 1)]);

    // Viser le créneau passé est refusé.
    let body = with(edit(&e), json!({ "availabilities": [{ "slotId": past }] }));
    let (res, text) = post_json_with(&router, UPDATE, &body, &secret(&c)).await;
    assert_fields(
        &res,
        &text,
        "slot.in_past",
        json!({ "availabilities.0.slotId": "slot.in_past" }),
    );
}

#[tokio::test]
async fn withdraw_from_pending_or_accepted_in_open_or_locked() {
    let (state, router) = setup("withdraw").await;
    for (event_status, direct) in [
        ("open", false),
        ("open", true),
        ("locked", false),
        ("locked", true),
    ] {
        let e = create(&state, &router, "all").await;
        let c = candidate(&state, &router, &e, "Nyraelle", direct).await;
        set_event_status(&state, &e, event_status).await;
        let ctx = format!("{event_status} direct={direct}");
        let (res, text) = post_json_with(&router, WITHDRAW, &json!({}), &secret(&c)).await;
        assert_eq!(res.status(), StatusCode::OK, "{ctx}: {text}");
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["status"], "withdrawn", "{ctx}");
        let ((_, _, _, cp, status, _, updated_at), avail) = row(&state, &c).await;
        assert_eq!(
            (cp, status.as_str(), updated_at.as_str()),
            (1500, "withdrawn", NOW),
            "{ctx}"
        );
        assert_eq!(avail.len(), 2, "{ctx}");
        // Plus compté sur la page publique.
        assert_eq!(v["event"]["countsByClass"]["cleric"], 0, "{ctx}");
    }
}

#[tokio::test]
async fn withdraw_is_refused_when_rejected_or_already_withdrawn() {
    let (state, router) = setup("withdraw_refused").await;
    let e = create(&state, &router, "all").await;
    let c = candidate(&state, &router, &e, "Nyraelle", false).await;
    for (status, code) in [
        ("rejected", "candidature.transition_forbidden"),
        ("withdrawn", "candidature.withdrawn"),
    ] {
        set_status(&state, &c, status).await;
        let before = row(&state, &c).await;
        let (res, body) = post_json_with(&router, WITHDRAW, &json!({}), &secret(&c)).await;
        assert_problem(&res, &body, code, status);
        assert_eq!(row(&state, &c).await, before, "{status}");
    }
    // Une candidature retirée ne se modifie plus non plus, même en `open`.
    let (res, body) = post_json_with(&router, UPDATE, &edit(&e), &secret(&c)).await;
    assert_problem(&res, &body, "candidature.withdrawn", "update withdrawn");
    // Le corps du retrait doit être un objet JSON.
    let (res, body) = post_raw_with(&router, WITHDRAW, "", &secret(&c)).await;
    assert_problem(&res, &body, "form.malformed", "empty body");
}

#[tokio::test]
async fn a_cancelled_event_closes_update_and_withdrawal_first() {
    let (state, router) = setup("cancelled").await;
    let e = create(&state, &router, "all").await;
    let c = candidate(&state, &router, &e, "Nyraelle", true).await;
    set_event_status(&state, &e, "cancelled").await;
    // L'annulation passe avant le statut de la candidature et avant toute règle de champ.
    for status in ["accepted", "withdrawn"] {
        set_status(&state, &c, status).await;
        let before = row(&state, &c).await;
        let bad = with(edit(&e), json!({ "server": "far_away" }));
        let (res, body) = post_json_with(&router, UPDATE, &bad, &secret(&c)).await;
        assert_problem(&res, &body, "event.closed", status);
        let (res, body) = post_json_with(&router, WITHDRAW, &json!({}), &secret(&c)).await;
        assert_problem(&res, &body, "event.closed", status);
        assert_eq!(row(&state, &c).await, before, "{status}");
    }
    // La lecture reste possible.
    let (status, v, _) = read(&router, &c).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["event"]["status"], "cancelled");
}

#[tokio::test]
async fn participant_commands_use_the_write_quota() {
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
    let c = candidate(&state, &router, &e, "Nyraelle", false).await;
    let (res, _) = post_json_with(&router, UPDATE, &edit(&e), &secret(&c)).await;
    assert_eq!(res.status(), StatusCode::OK);
    let (res, body) = post_json_with(&router, WITHDRAW, &json!({}), &secret(&c)).await;
    assert_problem(&res, &body, "limit.rate_limited", "quota");
    // La lecture est en classe `read`, pas `write`.
    let (status, _, _) = read(&router, &c).await;
    assert_eq!(status, StatusCode::OK);
    // Routes de commande : POST seulement.
    for uri in [UPDATE, WITHDRAW] {
        let (res, body) = send_with(&router, Method::GET, uri, &secret(&c)).await;
        assert_problem(&res, &body, "server.method_not_allowed", uri);
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
async fn no_secret_nor_pseudo_in_the_logs() {
    let (state, router) = setup("logs").await;
    let e = create(&state, &router, "all").await;
    let c = candidate(&state, &router, &e, "Nyraelle", false).await;
    let requests = || async {
        let (status, _, _) = read(&router, &c).await;
        assert_eq!(status, StatusCode::OK);
        let body = with(edit(&e), json!({ "pseudo": "Nyrarenamed" }));
        let (res, _) = post_json_with(&router, UPDATE, &body, &secret(&c)).await;
        assert_eq!(res.status(), StatusCode::OK);
        let bad = with(edit(&e), json!({ "server": "far_away" }));
        let (res, _) = post_json_with(&router, UPDATE, &bad, &secret(&c)).await;
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
        let (res, _) = post_raw_with(&router, UPDATE, "{", &secret(&c)).await;
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    };
    // Même gabarit que `candidature_submit::no_secret_nor_pseudo_in_the_logs`.
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
    requests().await;
    let (res, _) = post_json_with(&router, WITHDRAW, &json!({}), &secret(&c)).await;
    assert_eq!(res.status(), StatusCode::OK);
    drop(guard);
    let log = String::from_utf8(captured.0.lock().unwrap().clone()).unwrap();
    assert!(log.contains("candidature updated"), "{log}");
    assert!(log.contains("candidature withdrawn"), "{log}");
    assert!(log.contains(&c.id), "{log}");
    for leak in [c.secret.as_str(), "Nyraelle", "Nyrarenamed", TEST_CLIENT_IP] {
        assert!(!log.contains(leak), "{leak} in {log}");
    }
}

#[tokio::test]
async fn an_unchanged_server_and_class_disabled_since_are_kept() {
    let (state, router) = setup("disabled_since").await;
    let e = create(&state, &router, "all").await;
    let c = candidate(&state, &router, &e, "Nyraelle", false).await;
    let mut set = reference_set();
    for s in set.servers.iter_mut().filter(|s| s.id == "israphel") {
        s.enabled = false;
    }
    for cl in set.classes.iter_mut().filter(|cl| cl.id == "cleric") {
        cl.enabled = false;
    }
    state.reference.replace(std::sync::Arc::new(set));
    let (res, text) = post_json_with(&router, UPDATE, &edit(&e), &secret(&c)).await;
    assert_eq!(res.status(), StatusCode::OK, "{text}");
    let ((_, server, class, cp, _, _, _), _) = row(&state, &c).await;
    assert_eq!(
        (server.as_str(), class.as_str(), cp),
        ("israphel", "cleric", 2000)
    );
}

#[tokio::test]
async fn a_withdrawn_candidature_on_a_locked_event_is_withdrawn_first() {
    let (state, router) = setup("locked_withdrawn").await;
    let e = create(&state, &router, "all").await;
    let c = candidate(&state, &router, &e, "Nyraelle", false).await;
    set_status(&state, &c, "withdrawn").await;
    set_event_status(&state, &e, "locked").await;
    let before = row(&state, &c).await;
    let (res, body) = post_json_with(&router, UPDATE, &edit(&e), &secret(&c)).await;
    assert_problem(&res, &body, "candidature.withdrawn", "update");
    let (res, body) = post_json_with(&router, WITHDRAW, &json!({}), &secret(&c)).await;
    assert_problem(&res, &body, "candidature.withdrawn", "withdraw");
    assert_eq!(row(&state, &c).await, before);
}

#[tokio::test]
async fn an_availability_on_a_deleted_slot_is_slot_unknown_never_500() {
    let (state, router) = setup("deleted_slot").await;
    let e = create(&state, &router, "all").await;
    let c = candidate(&state, &router, &e, "Nyraelle", false).await;
    // Créneau supprimé après la candidature (cascade sur ses disponibilités).
    sql(&state, "DELETE FROM slots WHERE id = ?", &[&e.slots[1]]).await;
    let before = row(&state, &c).await;
    let (res, text) = post_json_with(&router, UPDATE, &edit(&e), &secret(&c)).await;
    assert_fields(
        &res,
        &text,
        "candidature.slot_unknown",
        json!({ "availabilities.0.slotId": "candidature.slot_unknown" }),
    );
    assert_eq!(row(&state, &c).await, before);
}
