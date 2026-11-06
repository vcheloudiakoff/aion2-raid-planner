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
    for args in [&["bogus"][..], &["serve", "extra"], &["healthcheck", "x"]] {
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
