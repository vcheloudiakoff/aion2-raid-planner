//! Aides partagées par les tests de candidature (stories 2-2 et 2-3) : événement créé par l'API,
//! corps nominal, erreurs de champ. Valeurs fictives.

use axum::http::StatusCode;
use domain::Timestamp;
use domain::reference::{Faction, Label, Server};
use serde_json::{Value, json};
use server::app::AppState;
use server::config::Config;
use server::http::form_token::FormClass;
use server::infra::reference::{self, ReferenceStore};

use super::*;

/// Instant de l'horloge de test.
pub const NOW: &str = "2026-11-06T10:00:00Z";

/// `NOW` moins `secs` secondes.
pub fn ago(secs: i64) -> Timestamp {
    Timestamp::from_unix_seconds(NOW.parse::<Timestamp>().unwrap().unix_seconds() - secs).unwrap()
}

/// Jeton de formulaire valide, émis dix secondes avant `NOW`.
pub fn token(state: &AppState, class: FormClass) -> String {
    state.form_tokens.issue(class, ago(10), [3; 16])
}

/// Référence embarquée, plus un serveur d'une autre région (valeurs fictives).
pub fn reference_set() -> domain::reference::ReferenceSet {
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

/// Base mémoire `name`, référence [`reference_set`], routeur de démonstration.
pub async fn candidature_setup(name: &str, config: &Config) -> (AppState, axum::Router) {
    let state = state_with_config(
        memory_db(name).await,
        ReferenceStore::with(reference_set()),
        config,
    );
    let router = fixture_router(state.clone());
    (state, router)
}

/// Événement créé par l'API.
pub struct Event {
    pub id: String,
    pub organizer: String,
    pub join: String,
    /// Ids des créneaux, triés par début.
    pub slots: Vec<String>,
}

/// Événement Ludra d'Europe à deux créneaux à venir, portée `scope`, `patch` sur le corps.
pub async fn create_with(
    state: &AppState,
    router: &axum::Router,
    scope: &str,
    patch: Value,
) -> Event {
    let mut body = json!({
        "formToken": token(state, FormClass::Create),
        "instance": "ludra",
        "region": "europe",
        "factionScope": scope,
        "slots": [
            { "startsAt": "2026-11-11T20:00:00Z" },
            { "startsAt": "2026-11-12T20:00:00Z" },
        ],
    });
    for (k, v) in patch.as_object().unwrap() {
        body[k] = v.clone();
    }
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

pub async fn create(state: &AppState, router: &axum::Router, scope: &str) -> Event {
    create_with(state, router, scope, json!({})).await
}

/// Corps nominal : Asmodienne d'Europe, Clerc, les deux créneaux, tous deux préférés.
pub fn nominal(state: &AppState, e: &Event) -> Value {
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

/// `body` avec les clés de `patch` remplacées.
pub fn with(mut body: Value, patch: Value) -> Value {
    for (k, v) in patch.as_object().unwrap() {
        body[k] = v.clone();
    }
    body
}

/// 400 problem+json avec `code` et exactement `fields`.
pub fn assert_fields(
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
