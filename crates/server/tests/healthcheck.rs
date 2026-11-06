//! La sous-commande `healthcheck` contre un vrai serveur sur port éphémère et fichier temporaire.

mod common;

use std::time::{Duration, Instant};

use common::*;
use server::healthcheck::check;
use server::infra::sqlite::{self, Db};

const TIMEOUT: Duration = Duration::from_secs(2);

async fn probe(addr: std::net::SocketAddr) -> bool {
    tokio::task::spawn_blocking(move || check(addr, TIMEOUT).is_ok())
        .await
        .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn healthcheck_follows_server_health() {
    let dir = TempDir::new("healthcheck");
    let db = Db::open(sqlite::file_options(&dir.0.join("app.db")))
        .await
        .unwrap();
    db.migrate().await.unwrap();
    let state = state(db.clone());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
    let server = tokio::spawn(server::http::serve(
        listener,
        server::http::router(state),
        async {
            let _ = stop_rx.await;
        },
    ));

    assert!(probe(addr).await, "200 → exit 0");

    db.read.close().await;
    assert!(!probe(addr).await, "503 → exit ≠ 0");

    stop_tx.send(()).unwrap();
    server.await.unwrap().unwrap();
    let started = Instant::now();
    assert!(!probe(addr).await, "server stopped → exit ≠ 0");
    assert!(started.elapsed() < TIMEOUT * 4, "bounded delay");
}

#[test]
fn healthcheck_times_out_on_silent_server() {
    // Un pair qui accepte la connexion sans jamais répondre : échec dans le délai borné.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let _keep = std::thread::spawn(move || {
        let conn = listener.accept();
        std::thread::sleep(Duration::from_secs(3));
        drop(conn);
    });
    let started = Instant::now();
    assert!(check(addr, Duration::from_millis(300)).is_err());
    assert!(started.elapsed() < Duration::from_secs(2));
}

/// Réponse brute d'un `GET` HTTP/1.1 sans en-tête `X-Client-IP`.
fn raw_get(addr: std::net::SocketAddr, path: &str) -> String {
    use std::io::{Read, Write};
    let mut conn = std::net::TcpStream::connect(addr).unwrap();
    conn.set_read_timeout(Some(TIMEOUT)).unwrap();
    write!(
        conn,
        "GET {path} HTTP/1.1\r\nHost: raid.example.org\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut out = String::new();
    conn.read_to_string(&mut out).unwrap();
    out
}

/// Le vrai `http::serve` fournit `ConnectInfo` : en `TRUST_CLIENT_IP_HEADER=false`, l'adresse du pair
/// sert de clé ; en `true`, l'en-tête absent reste un 400.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn serve_provides_peer_address_when_header_is_not_trusted() {
    for (trust, expected) in [("false", "200"), ("true", "400")] {
        let db = memory_db(&format!("serve_connect_info_{trust}")).await;
        let reference = server::infra::reference::ReferenceStore::with(
            server::infra::reference::embedded().unwrap(),
        );
        let config = test_config(&[("TRUST_CLIENT_IP_HEADER", trust)]);
        let state = state_with_config(db, reference, &config);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
        let server = tokio::spawn(server::http::serve(
            listener,
            server::http::router(state),
            async {
                let _ = stop_rx.await;
            },
        ));
        let res = tokio::task::spawn_blocking(move || raw_get(addr, "/api/reference"))
            .await
            .unwrap();
        let status = res.split(' ').nth(1).unwrap_or_default();
        assert_eq!(status, expected, "trust={trust}: {res}");
        assert_eq!(
            res.contains("limit.client_ip_missing"),
            trust == "true",
            "trust={trust}: {res}"
        );
        stop_tx.send(()).unwrap();
        server.await.unwrap().unwrap();
    }
}
