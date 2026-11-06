//! Le binaire au niveau processus : codes de sortie et messages lus par le healthcheck du conteneur
//! et par l'exploitant.

use std::net::TcpListener;
use std::process::{Command, Output};

fn server(args: &[&str]) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_server"));
    // Aucune variable héritée : ni configuration, ni `RUST_LOG` du poste.
    cmd.args(args).env_clear();
    cmd
}

fn run(cmd: &mut Command) -> Output {
    cmd.output().expect("server binary runs")
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn unknown_argument_exits_2_with_usage() {
    for args in [
        &["bogus"][..],
        &["serve", "extra"],
        &["healthcheck", "x"],
        &["backup", "weekly"],
        &["backup", "daily", "x"],
        &["restore", "a", "b"],
        &["list-error-codes", "x"],
    ] {
        let out = run(&mut server(args));
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        assert!(stderr(&out).contains("usage:"), "{args:?}");
    }
}

#[test]
fn healthcheck_without_server_fails() {
    // Le port fixe 127.0.0.1:8080 est occupé par un faux serveur qui ferme aussitôt : le résultat
    // ne dépend pas de ce qui tourne sur le poste. S'il est déjà pris (un conteneur de l'outil en
    // local, par exemple), le test est sauté plutôt que de dépendre de ce qui y répond.
    let Ok(listener) = TcpListener::bind("127.0.0.1:8080") else {
        eprintln!("skipped: 127.0.0.1:8080 already in use");
        return;
    };
    let accept = listener.try_clone().unwrap();
    std::thread::spawn(move || {
        for stream in accept.incoming() {
            drop(stream);
        }
    });
    let out = run(&mut server(&["healthcheck"]));
    assert!(!out.status.success());
    assert_ne!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("healthcheck failed"));
    drop(listener);
}

#[test]
fn serve_without_config_names_the_variable() {
    for args in [&[][..], &["serve"]] {
        let out = run(&mut server(args));
        assert!(!out.status.success(), "{args:?}");
        assert_ne!(out.status.code(), Some(2), "{args:?}");
        assert!(stderr(&out).contains("PUBLIC_BASE_URL"), "{args:?}");
    }
}

#[test]
fn serve_with_short_secret_names_it_without_echoing_it() {
    let secret = "short-secret-value";
    let out = run(server(&["serve"])
        .env("PUBLIC_BASE_URL", "https://raid.example.org")
        .env("FORM_TOKEN_SECRET", secret));
    assert!(!out.status.success());
    let err = stderr(&out);
    assert!(err.contains("FORM_TOKEN_SECRET"));
    assert!(!err.contains(secret));
}

#[test]
fn validate_reference_accepts_the_repository_set() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../reference");
    let out = run(&mut server(&["validate-reference", dir]));
    assert!(out.status.success(), "{}", stderr(&out));
}

// `backup` et `restore` travaillent sur les chemins fixes `/data/...` du conteneur ; sur le poste,
// `/data/backups` est absent et ces tests vérifient seulement les codes de sortie. Le comportement
// sur fichiers est testé dans `infra::backup` et par la fumée du compose en CI.

#[test]
fn restore_without_name_lists_and_exits_2() {
    if std::path::Path::new("/data/backups").exists() {
        eprintln!("skipped: /data/backups exists on this machine");
        return;
    }
    let out = run(&mut server(&["restore"]));
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    assert!(stderr(&out).contains("restore NAME"));
}

#[test]
fn restore_of_unknown_backup_fails_without_usage() {
    let out = run(&mut server(&["restore", "daily-20261106T000000Z.db"]));
    assert!(!out.status.success());
    assert_ne!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("unknown backup"), "{}", stderr(&out));
}

#[test]
fn backup_without_database_fails_without_creating_it() {
    if std::path::Path::new("/data/app.db").exists() {
        eprintln!("skipped: /data/app.db exists on this machine");
        return;
    }
    for args in [
        &["backup"][..],
        &["backup", "daily"],
        &["backup", "pre-deploy"],
    ] {
        let out = run(&mut server(args));
        assert!(!out.status.success(), "{args:?}");
        assert_ne!(out.status.code(), Some(2), "{args:?}");
        assert!(out.stdout.is_empty(), "{args:?}");
        assert!(stderr(&out).contains("error:"), "{args:?}");
    }
    assert!(!std::path::Path::new("/data/app.db").exists());
}

#[test]
fn list_error_codes_prints_sorted_registry_without_config() {
    let out = run(&mut server(&["list-error-codes"]));
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let stdout = String::from_utf8(out.stdout).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines, server::http::errors::Code::sorted());
    assert!(lines.windows(2).all(|w| w[0] < w[1]), "sorted, unique");
    for expected in [
        "reference.unavailable",
        "server.internal",
        "server.method_not_allowed",
        "server.not_found",
        "server.unavailable",
    ] {
        assert!(lines.contains(&expected), "{expected}");
    }
}
