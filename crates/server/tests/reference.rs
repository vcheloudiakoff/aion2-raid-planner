//! Matrice de la story 1-2 : `GET /api/reference`, ETag, rechargement, échecs, jeu embarqué, santé et
//! sous-commande `validate-reference`.

mod common;

use std::path::Path;
use std::process::Command;
use std::time::{Duration, SystemTime};

use axum::http::{Method, StatusCode};
use common::*;
use domain::reference::{Faction, InstanceKind, ReferenceFile};
use server::http::headers::APP_BUILD;
use server::infra::reference::{
    self, EMBEDDED, LoadError, Loader, PollOutcome, ReferenceStore, load_dir,
};

const REPO_REFERENCE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../reference");

/// Copie les cinq fichiers du dépôt dans `dir`.
fn copy_repo_reference(dir: &Path) {
    for f in ReferenceFile::ALL {
        std::fs::copy(
            Path::new(REPO_REFERENCE).join(f.file_name()),
            dir.join(f.file_name()),
        )
        .unwrap();
    }
}

/// Réécrit un fichier et avance son mtime, pour ne pas dépendre de la granularité du système de
/// fichiers.
fn rewrite(dir: &Path, file: &str, edit: impl FnOnce(String) -> String) {
    let path = dir.join(file);
    let before = std::fs::metadata(&path)
        .ok()
        .and_then(|m| m.modified().ok());
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    std::fs::write(&path, edit(text)).unwrap();
    let after = before.unwrap_or(SystemTime::now()) + Duration::from_secs(2);
    std::fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(after)
        .unwrap();
}

fn json(body: &str) -> serde_json::Value {
    serde_json::from_str(body).unwrap()
}

async fn router_on(store: ReferenceStore, name: &str) -> axum::Router {
    fixture_router(state_with(memory_db(name).await, store))
}

#[tokio::test]
async fn reference_is_served_in_camel_case_with_strong_etag() {
    let router = fixture_router(memory_state("ref_get").await);
    let (res, body) = send(&router, Method::GET, "/api/reference").await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(header(&res, "content-type"), "application/json");
    assert_eq!(header(&res, "cache-control"), "no-store");
    assert_eq!(header(&res, "x-app-build"), APP_BUILD);
    let etag = header(&res, "etag");
    assert!(
        etag.len() == 34 && etag.starts_with('"') && etag.ends_with('"'),
        "strong etag: {etag}"
    );
    assert_eq!(etag, server::http::reference::etag(body.as_bytes()));

    let v = json(&body);
    let instance = &v["instances"][0];
    for key in [
        "partySize",
        "durationMin",
        "crossFaction",
        "roomLabel",
        "parties",
    ] {
        assert!(instance.get(key).is_some(), "instances[0].{key}");
    }
    assert!(instance.get("party_size").is_none());
    let class = &v["classes"][0];
    assert!(class.get("defaultMin").is_some() && class.get("defaultMax").is_some());
    assert!(v["factions"]["elyos"]["en"].is_string());
    assert!(v["factions"]["asmodian"]["en"].is_string());

    // Valeurs d'enum sérialisées en minuscules, sans inversion des factions.
    assert_eq!(v["servers"][0]["faction"], "elyos");
    let servers = v["servers"].as_array().unwrap();
    assert!(servers.iter().any(|s| s["faction"] == "asmodian"));
    assert_eq!(v["instances"][0]["kind"], "sanctuary");
    assert!(v.get("roomLabelCharset").is_none());

    // `fr` absent quand il n'est pas saisi, présent sinon.
    let server = &v["servers"][0];
    assert!(server["label"]["en"].is_string());
    assert!(server["label"].get("fr").is_none());
    assert!(v["regions"][0]["label"]["fr"].is_string());

    // Entrées désactivées servies avec `enabled: false`.
    let regions = v["regions"].as_array().unwrap();
    assert!(regions.iter().any(|r| r["enabled"] == false));
    assert!(regions.iter().any(|r| r["enabled"] == true));
}

#[tokio::test]
async fn if_none_match_returns_304_without_body() {
    let router = fixture_router(memory_state("ref_304").await);
    let (res, _) = send(&router, Method::GET, "/api/reference").await;
    let etag = header(&res, "etag").to_owned();

    let weak = format!("W/{etag}");
    let listed = format!("\"other\", {etag}");
    for value in [etag.as_str(), weak.as_str(), "*", listed.as_str()] {
        let (res, body) = send_with(
            &router,
            Method::GET,
            "/api/reference",
            &[("if-none-match", value)],
        )
        .await;
        assert_eq!(res.status(), StatusCode::NOT_MODIFIED, "{value}");
        assert!(body.is_empty());
        assert_eq!(header(&res, "etag"), etag);
        assert_eq!(header(&res, "cache-control"), "no-store");
        assert_eq!(header(&res, "x-app-build"), APP_BUILD);
    }

    for value in ["\"other\"", "W/\"other\"", ""] {
        let (res, body) = send_with(
            &router,
            Method::GET,
            "/api/reference",
            &[("if-none-match", value)],
        )
        .await;
        assert_eq!(res.status(), StatusCode::OK, "{value}");
        assert!(!body.is_empty());
    }
}

#[tokio::test]
async fn label_change_is_served_after_poll_without_restart() {
    let dir = TempDir::new("ref-reload");
    copy_repo_reference(&dir.0);
    let store = ReferenceStore::new();
    let mut loader = Loader::new(&dir.0, store.clone());
    assert_eq!(loader.poll(), PollOutcome::Loaded);
    assert_eq!(loader.poll(), PollOutcome::Unchanged);

    let router = router_on(store, "ref_reload").await;
    let (res, body) = send(&router, Method::GET, "/api/reference").await;
    assert_eq!(res.status(), StatusCode::OK);
    let etag = header(&res, "etag").to_owned();
    assert!(!body.contains("Reloaded Label"));

    rewrite(&dir.0, "roles.toml", |t| {
        t.replacen("en = \"Tank\"", "en = \"Reloaded Label\"", 1)
    });
    assert_eq!(loader.poll(), PollOutcome::Loaded);

    let (res, body) = send(&router, Method::GET, "/api/reference").await;
    assert_eq!(res.status(), StatusCode::OK);
    assert!(body.contains("Reloaded Label"));
    assert_ne!(header(&res, "etag"), etag);

    // L'ancien ETag ne donne plus 304.
    let (res, _) = send_with(
        &router,
        Method::GET,
        "/api/reference",
        &[("if-none-match", &etag)],
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
}

#[test]
fn a_reload_the_room_template_no_longer_fits_keeps_the_previous_set() {
    let dir = TempDir::new("ref-room-template");
    copy_repo_reference(&dir.0);
    let store = ReferenceStore::new();
    let mut loader = Loader::new(&dir.0, store.clone());
    assert_eq!(loader.poll(), PollOutcome::Loaded);
    assert_eq!(store.get().unwrap().room_label_max, 8);
    // 9 + 4 = 13 caractères : tient dans `room_label_max + 5` tant que le maximum vaut 8 ou plus.
    loader.guard_room_template(domain::room::RoomTemplate::parse("RAIDROOM-{code}").unwrap());

    rewrite(&dir.0, "instances.toml", |t| {
        t.replacen("room_label_max = 8", "room_label_max = 7", 1)
    });
    assert_eq!(loader.poll(), PollOutcome::Failed { logged: true });
    assert_eq!(loader.poll(), PollOutcome::Failed { logged: false });
    assert_eq!(store.get().unwrap().room_label_max, 8);

    rewrite(&dir.0, "instances.toml", |t| {
        t.replacen("room_label_max = 7", "room_label_max = 9", 1)
    });
    assert_eq!(loader.poll(), PollOutcome::Loaded);
    assert_eq!(store.get().unwrap().room_label_max, 9);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reload_task_picks_up_changes_on_its_own() {
    let dir = TempDir::new("ref-task");
    copy_repo_reference(&dir.0);
    let store = ReferenceStore::new();
    let mut loader = Loader::new(&dir.0, store.clone());
    assert_eq!(loader.poll(), PollOutcome::Loaded);
    let task = server::app::tasks::reload::spawn(loader, Duration::from_millis(20));

    rewrite(&dir.0, "classes.toml", |t| {
        t.replacen("en = \"Cleric\"", "en = \"Ticked\"", 1)
    });
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        let set = store.get().unwrap();
        if set.classes.iter().any(|c| c.label.en == "Ticked") {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "reload never happened"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    task.abort();
    assert!(server::app::tasks::reload::TICK <= Duration::from_secs(60));
}

/// Chaque cas invalide : l'ancien jeu reste servi, l'échec n'est journalisé qu'une fois par état et
/// la correction est reprise au tick suivant.
#[tokio::test]
async fn invalid_toml_keeps_previous_set() {
    type Edit = fn(String) -> String;
    let cases: &[(&str, &str, Edit, &str)] = &[
        (
            "syntax",
            "regions.toml",
            // En tête du fichier : la position reste stable quand Vincent édite le contenu.
            |t| "[[region]\n".to_owned() + &t,
            "regions.toml: line 1, column 10: ",
        ),
        (
            "duplicate id",
            "roles.toml",
            |t| t + "\n[[role]]\nid = \"tank\"\nlabel = { en = \"Again\" }\n",
            "roles.toml: tank: duplicate id",
        ),
        (
            "missing en",
            "roles.toml",
            |t| t + "\n[[role]]\nid = \"extra\"\nlabel = { fr = \"Seul\" }\n",
            "missing field `en`",
        ),
        (
            "unknown field",
            "roles.toml",
            |t| t + "\n[[role]]\nid = \"extra\"\nlabel = { en = \"X\" }\nenabeld = false\n",
            "enabeld",
        ),
        (
            "bound",
            "instances.toml",
            |t| t.replacen("parties = 2", "parties = 0", 1),
            "instances.toml: ludra: parties must be at least 1",
        ),
        (
            "negative bound",
            "classes.toml",
            |t| t.replacen("default_min = 0", "default_min = -1", 1),
            "classes.toml",
        ),
        (
            "cross reference",
            "classes.toml",
            |t| t.replacen("role = \"tank\"", "role = \"nobody\"", 1),
            "classes.toml: templar: unknown role `nobody`",
        ),
        (
            "room label charset",
            "instances.toml",
            |t| t.replacen("room_label = \"LUDRA\"", "room_label = \"ludra\"", 1),
            "outside room_label_charset",
        ),
    ];
    for (i, (case, file, edit, expected)) in cases.iter().enumerate() {
        let dir = TempDir::new(&format!("ref-invalid-{i}"));
        copy_repo_reference(&dir.0);
        let store = ReferenceStore::new();
        let mut loader = Loader::new(&dir.0, store.clone());
        assert_eq!(loader.poll(), PollOutcome::Loaded, "{case}");
        let before = store.get().unwrap();
        let original = std::fs::read_to_string(dir.0.join(file)).unwrap();

        rewrite(&dir.0, file, edit);
        let problems = load_dir(&dir.0).unwrap_err().problems();
        assert!(
            problems.iter().any(|p| p.to_string().contains(expected)),
            "{case}: {problems:?}"
        );
        assert!(problems.iter().all(|p| p.file_name() == *file), "{case}");

        assert_eq!(
            loader.poll(),
            PollOutcome::Failed { logged: true },
            "{case}"
        );
        assert_eq!(
            loader.poll(),
            PollOutcome::Failed { logged: false },
            "{case}"
        );
        assert!(
            std::sync::Arc::ptr_eq(&before, &store.get().unwrap()),
            "{case}"
        );

        let router = router_on(store.clone(), &format!("ref_invalid_{i}")).await;
        let (res, _) = send(&router, Method::GET, "/api/reference").await;
        assert_eq!(res.status(), StatusCode::OK, "{case}: old set still served");

        rewrite(&dir.0, file, |_| original);
        assert_eq!(loader.poll(), PollOutcome::Loaded, "{case}: fix picked up");
        assert!(!std::sync::Arc::ptr_eq(&before, &store.get().unwrap()));
    }
}

#[test]
fn a_new_failure_state_is_logged_again() {
    let dir = TempDir::new("ref-relog");
    copy_repo_reference(&dir.0);
    let mut loader = Loader::new(&dir.0, ReferenceStore::new());
    assert_eq!(loader.poll(), PollOutcome::Loaded);
    rewrite(&dir.0, "roles.toml", |t| t + "\nbroken =\n");
    assert_eq!(loader.poll(), PollOutcome::Failed { logged: true });
    assert_eq!(loader.poll(), PollOutcome::Failed { logged: false });
    rewrite(&dir.0, "roles.toml", |t| t + "\nalso broken =\n");
    assert_eq!(loader.poll(), PollOutcome::Failed { logged: true });
}

#[test]
fn empty_directory_uses_embedded_set() {
    let dir = TempDir::new("ref-empty");
    let store = ReferenceStore::new();
    let mut loader = Loader::new(&dir.0, store.clone());
    assert_eq!(loader.poll(), PollOutcome::Loaded);
    assert_eq!(*store.get().unwrap(), reference::embedded().unwrap());
    assert_eq!(loader.poll(), PollOutcome::Unchanged);

    // Le dossier peut même ne pas exister.
    let mut loader = Loader::new(dir.0.join("absent"), ReferenceStore::new());
    assert_eq!(loader.poll(), PollOutcome::Loaded);

    // Des fichiers apparaissent (copie du déploiement) : ils remplacent le jeu embarqué.
    copy_repo_reference(&dir.0);
    rewrite(&dir.0, "roles.toml", |t| {
        t.replacen("en = \"Tank\"", "en = \"From Disk\"", 1)
    });
    let mut loader = Loader::new(&dir.0, store.clone());
    assert_eq!(loader.poll(), PollOutcome::Loaded);
    assert!(
        store
            .get()
            .unwrap()
            .roles
            .iter()
            .any(|r| r.label.en == "From Disk")
    );
}

#[test]
fn partial_directory_is_invalid() {
    for present in 1..=4 {
        let dir = TempDir::new(&format!("ref-partial-{present}"));
        copy_repo_reference(&dir.0);
        for f in &ReferenceFile::ALL[present..] {
            std::fs::remove_file(dir.0.join(f.file_name())).unwrap();
        }
        let store = ReferenceStore::new();
        let mut loader = Loader::new(&dir.0, store.clone());
        assert_eq!(loader.poll(), PollOutcome::Failed { logged: true });
        assert!(store.get().is_none(), "never the embedded set");
        match load_dir(&dir.0) {
            Err(LoadError::Missing(missing)) => assert_eq!(missing.len(), 5 - present),
            other => panic!("{present} files: {other:?}"),
        }
    }
}

#[tokio::test]
async fn startup_without_valid_set_serves_503_and_retries() {
    let dir = TempDir::new("ref-startup");
    copy_repo_reference(&dir.0);
    let original = std::fs::read_to_string(dir.0.join("servers.toml")).unwrap();
    rewrite(&dir.0, "servers.toml", |t| {
        t.replace("region = \"europe\"", "region = \"mars\"")
    });

    let store = ReferenceStore::new();
    let mut loader = Loader::new(&dir.0, store.clone());
    assert_eq!(loader.poll(), PollOutcome::Failed { logged: true });

    let router = router_on(store, "ref_startup").await;
    for path in ["/api/health", "/api/reference"] {
        let (res, body) = send(&router, Method::GET, path).await;
        assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE, "{path}");
        let code = if path == "/api/health" {
            "server.unavailable"
        } else {
            "reference.unavailable"
        };
        assert_problem(&res, &body, code, path);
        assert_eq!(header(&res, "x-app-build"), APP_BUILD);
        assert_eq!(header(&res, "cache-control"), "no-store");
    }

    rewrite(&dir.0, "servers.toml", |_| original);
    assert_eq!(loader.poll(), PollOutcome::Loaded);
    for path in ["/api/health", "/api/reference"] {
        let (res, _) = send(&router, Method::GET, path).await;
        assert_eq!(res.status(), StatusCode::OK, "{path}");
    }
}

#[test]
fn embedded_set_is_valid_and_matches_initial_content() {
    let set = reference::embedded().unwrap();
    for (i, f) in ReferenceFile::ALL.iter().enumerate() {
        let on_disk =
            std::fs::read_to_string(Path::new(REPO_REFERENCE).join(f.file_name())).unwrap();
        assert_eq!(EMBEDDED[i], on_disk, "{f} embedded = repository copy");
    }
    assert_eq!(load_dir(Path::new(REPO_REFERENCE)).unwrap(), set);

    let regions: Vec<(&str, bool)> = set
        .regions
        .iter()
        .map(|r| (r.id.as_str(), r.enabled))
        .collect();
    assert_eq!(
        regions,
        [
            ("europe", true),
            ("na_west", false),
            ("na_east", false),
            ("latin_america", false),
            ("asia", false)
        ]
    );

    let pairs = [
        ("Siel", "Israphel"),
        ("Nezekan", "Zikel"),
        ("Vaizel", "Triniel"),
        ("Kaisinel", "Lumiel"),
        ("Yustiel", "Marchutan"),
        ("Ariel", "Azphel"),
        ("Fregion", "Ereshkigal"),
        ("Meslamtaeda", "Beritra"),
        ("Hithanya", "Nemon"),
    ];
    assert_eq!(set.servers.len(), pairs.len() * 2);
    for (elyos, asmodian) in pairs {
        for (name, faction) in [(elyos, Faction::Elyos), (asmodian, Faction::Asmodian)] {
            let s = set
                .servers
                .iter()
                .find(|s| s.label.en == name)
                .unwrap_or_else(|| panic!("server {name}"));
            assert_eq!(s.id, name.to_lowercase());
            assert_eq!(s.region, "europe");
            assert_eq!(s.faction, faction);
            assert_eq!(s.tag, name);
            assert!(s.enabled);
        }
    }
    assert_eq!(set.factions.elyos.label.en, "Elyos");
    assert_eq!(set.factions.elyos.label.fr.as_deref(), Some("Élyséens"));
    assert_eq!(set.factions.asmodian.label.fr.as_deref(), Some("Asmodiens"));

    let classes: Vec<(&str, &str, &str, Option<&str>)> = set
        .classes
        .iter()
        .map(|c| {
            assert_eq!((c.default_min, c.default_max), (0, 10), "{}", c.id);
            (
                c.id.as_str(),
                c.role.as_str(),
                c.label.en.as_str(),
                c.label.fr.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        classes,
        [
            ("gladiator", "dps", "Gladiator", Some("Gladiateur")),
            ("templar", "tank", "Templar", Some("Templier")),
            ("assassin", "dps", "Assassin", Some("Assassin")),
            ("ranger", "dps", "Ranger", Some("Rôdeur")),
            ("sorcerer", "dps", "Sorcerer", Some("Sorcier")),
            ("spiritmaster", "dps", "Spiritmaster", Some("Spiritualiste")),
            ("cleric", "healer", "Cleric", Some("Clerc")),
            ("chanter", "support", "Chanter", Some("Aède")),
        ]
    );

    let roles: Vec<(&str, Option<&str>)> = set
        .roles
        .iter()
        .map(|r| (r.id.as_str(), r.label.fr.as_deref()))
        .collect();
    assert_eq!(
        roles,
        [
            ("tank", Some("Tank")),
            ("healer", Some("Soin")),
            ("dps", Some("DPS")),
            ("support", Some("Soutien"))
        ]
    );

    assert_eq!(set.instances.len(), 1);
    let ludra = &set.instances[0];
    assert_eq!(ludra.id, "ludra");
    assert_eq!(ludra.kind, InstanceKind::Sanctuary);
    assert_eq!(
        (ludra.parties, ludra.party_size, ludra.duration_min),
        (2, 5, 60)
    );
    assert!(ludra.cross_faction && ludra.enabled);
}

// --- sous-commande, au niveau processus ---

fn validate_reference(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_server"))
        .arg("validate-reference")
        .args(args)
        .env_clear()
        .output()
        .unwrap()
}

#[test]
fn cli_accepts_repository_reference_without_config() {
    let out = validate_reference(&[REPO_REFERENCE]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        stdout.starts_with("reference OK: 5 regions, 18 servers"),
        "{stdout}"
    );
    assert!(out.stderr.is_empty());
}

#[test]
fn cli_reports_each_problem_on_stderr() {
    let dir = TempDir::new("ref-cli-invalid");
    copy_repo_reference(&dir.0);
    rewrite(&dir.0, "classes.toml", |t| {
        t.replacen("role = \"tank\"", "role = \"nobody\"", 1)
    });
    rewrite(&dir.0, "servers.toml", |t| {
        t.replacen("tag = \"Siel\"", "tag = \"[Siel]\"", 1)
            .replacen("tag = \"Zikel\"", "tag = \"Siel\"", 1)
    });
    let out = validate_reference(&[dir.0.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    let stderr = String::from_utf8(out.stderr).unwrap();
    let lines: Vec<&str> = stderr.lines().collect();
    assert_eq!(lines.len(), 2, "{stderr}");
    assert!(stderr.contains("servers.toml: siel: tag must not contain"));
    assert!(stderr.contains("classes.toml: templar: unknown role `nobody`"));
}

#[test]
fn cli_fails_on_empty_partial_or_missing_directory() {
    let empty = TempDir::new("ref-cli-empty");
    let out = validate_reference(&[empty.0.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1));
    assert!(!out.stderr.is_empty());

    let partial = TempDir::new("ref-cli-partial");
    copy_repo_reference(&partial.0);
    std::fs::remove_file(partial.0.join("roles.toml")).unwrap();
    let out = validate_reference(&[partial.0.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1));
    assert!(
        String::from_utf8(out.stderr)
            .unwrap()
            .contains("roles.toml: file is missing")
    );

    let out = validate_reference(&[empty.0.join("absent").to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn cli_defaults_to_data_reference_and_rejects_extra_arguments() {
    // `/data/reference` n'existe pas sur la machine de test : échec, mais pas d'usage.
    if !Path::new(reference::REFERENCE_DIR).exists() {
        let out = validate_reference(&[]);
        assert_eq!(out.status.code(), Some(1));
    }
    let out = validate_reference(&[REPO_REFERENCE, "extra"]);
    assert_eq!(out.status.code(), Some(2));
}
