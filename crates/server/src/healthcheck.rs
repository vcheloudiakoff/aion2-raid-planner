//! Sous-commande `healthcheck` : `GET /api/health` en HTTP/1.1 écrit à la main sur `TcpStream`,
//! sans crate client (exception explicite à AD-17, voir AD-8).

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

/// Adresse fixe dans le conteneur : le port d'écoute du binaire, sur la boucle locale.
pub const ADDR: SocketAddr = SocketAddr::new(
    std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
    crate::config::LISTEN_PORT,
);

/// Délai par opération (connexion, écriture, lecture).
pub const TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, thiserror::Error)]
pub enum HealthcheckError {
    #[error("cannot reach server: {0}")]
    Io(#[from] std::io::Error),
    #[error("unexpected response status: {0}")]
    Status(String),
}

/// `Ok` si et seulement si le serveur répond 200 dans les délais.
pub fn check(addr: SocketAddr, timeout: Duration) -> Result<(), HealthcheckError> {
    let mut stream = TcpStream::connect_timeout(&addr, timeout)?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
    write!(
        stream,
        "GET /api/health HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\nUser-Agent: healthcheck\r\n\r\n"
    )?;
    stream.flush()?;
    // La ligne de statut tient largement dans 64 octets ; le reste est ignoré.
    let mut buf = [0u8; 64];
    let mut len = 0;
    while len < buf.len() {
        let n = stream.read(&mut buf[len..])?;
        if n == 0 {
            break;
        }
        len += n;
        if buf[..len].contains(&b'\n') {
            break;
        }
    }
    let head = String::from_utf8_lossy(&buf[..len]);
    let status_line = head.lines().next().unwrap_or_default();
    let mut parts = status_line.split(' ');
    match (parts.next(), parts.next()) {
        (Some(v), Some("200")) if v.starts_with("HTTP/1.") => Ok(()),
        _ => Err(HealthcheckError::Status(
            status_line.chars().take(40).collect(),
        )),
    }
}
