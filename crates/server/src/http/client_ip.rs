//! IP visiteur, clé du limiteur (AD-7). Jamais journalisée ni écrite nulle part (AD-15).
//!
//! En production (`TRUST_CLIENT_IP_HEADER=true`), l'IP se lit dans l'en-tête nommé par
//! `CLIENT_IP_HEADER`, posé et écrasé par le Caddy du VPS. Absent, en double, multi-valué ou non
//! analysable : 400 `limit.client_ip_missing`, sans repli sur l'adresse du pair (celle du proxy, qui
//! ferait partager un seul quota à tous les visiteurs). Avec `false` (poste de développement), l'adresse
//! du pair est lue par `ConnectInfo<SocketAddr>`.

use std::fmt;
use std::net::{IpAddr, SocketAddr};

use axum::extract::ConnectInfo;
use axum::http::request::Parts;
use axum::http::{HeaderMap, HeaderName};

use crate::http::errors::Code;

/// Clé de limitation : l'adresse IPv4 entière, ou le préfixe /64 d'une IPv6. Ni `Display` ni `Debug`
/// en clair.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum IpKey {
    V4([u8; 4]),
    V6Prefix([u8; 8]),
}

impl fmt::Debug for IpKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("IpKey(<redacted>)")
    }
}

impl From<IpAddr> for IpKey {
    fn from(ip: IpAddr) -> Self {
        // `::ffff:a.b.c.d` est une IPv4 : même compteur que `a.b.c.d`.
        match ip.to_canonical() {
            IpAddr::V4(v4) => Self::V4(v4.octets()),
            IpAddr::V6(v6) => {
                let mut prefix = [0u8; 8];
                prefix.copy_from_slice(&v6.octets()[..8]);
                Self::V6Prefix(prefix)
            }
        }
    }
}

/// D'où lire l'IP visiteur.
#[derive(Debug, Clone)]
pub struct ClientIpSource {
    pub header: HeaderName,
    pub trust_header: bool,
}

impl ClientIpSource {
    /// Clé de la requête, ou 400 `limit.client_ip_missing`.
    pub fn resolve(&self, parts: &Parts) -> Result<IpKey, Code> {
        if self.trust_header {
            from_header(&parts.headers, &self.header)
        } else {
            parts
                .extensions
                .get::<ConnectInfo<SocketAddr>>()
                .map(|ConnectInfo(peer)| IpKey::from(peer.ip()))
                .ok_or(Code::LimitClientIpMissing)
        }
    }
}

/// Un seul en-tête, une seule adresse, rien d'autre (pas de liste `a, b`).
fn from_header(headers: &HeaderMap, name: &HeaderName) -> Result<IpKey, Code> {
    let mut values = headers.get_all(name).iter();
    let value = match (values.next(), values.next()) {
        (Some(v), None) => v,
        _ => return Err(Code::LimitClientIpMissing),
    };
    value
        .to_str()
        .ok()
        .and_then(|s| s.trim().parse::<IpAddr>().ok())
        .map(IpKey::from)
        .ok_or(Code::LimitClientIpMissing)
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderValue, Request};

    use super::*;

    const HEADER: HeaderName = HeaderName::from_static("x-client-ip");

    fn parts(values: &[&str], peer: Option<&str>) -> Parts {
        let mut req = Request::builder().uri("/");
        for v in values {
            req = req.header(HEADER, HeaderValue::from_str(v).unwrap());
        }
        let (mut parts, ()) = req.body(()).unwrap().into_parts();
        if let Some(p) = peer {
            parts
                .extensions
                .insert(ConnectInfo::<SocketAddr>(p.parse().unwrap()));
        }
        parts
    }

    fn source(trust_header: bool) -> ClientIpSource {
        ClientIpSource {
            header: HEADER,
            trust_header,
        }
    }

    fn key(s: &str) -> IpKey {
        IpKey::from(s.parse::<IpAddr>().unwrap())
    }

    #[test]
    fn trusted_header_single_value() {
        let src = source(true);
        assert_eq!(
            src.resolve(&parts(&["192.0.2.10"], None)),
            Ok(key("192.0.2.10"))
        );
        assert_eq!(
            src.resolve(&parts(&[" 2001:db8::1 "], None)),
            Ok(key("2001:db8::1"))
        );
    }

    #[test]
    fn trusted_header_rejects_absent_multiple_or_garbage_without_peer_fallback() {
        let src = source(true);
        for values in [
            &[][..],
            &["192.0.2.1", "192.0.2.2"][..],
            &["192.0.2.1, 192.0.2.2"][..],
            &["192.0.2.1:80"][..],
            &["unknown"][..],
            &[""][..],
        ] {
            assert_eq!(
                src.resolve(&parts(values, Some("192.0.2.99:5000"))),
                Err(Code::LimitClientIpMissing),
                "{values:?}"
            );
        }
    }

    #[test]
    fn untrusted_uses_peer_and_ignores_header() {
        let src = source(false);
        assert_eq!(
            src.resolve(&parts(&["192.0.2.1"], Some("192.0.2.99:5000"))),
            Ok(key("192.0.2.99"))
        );
        assert_eq!(
            src.resolve(&parts(&["192.0.2.1"], None)),
            Err(Code::LimitClientIpMissing)
        );
    }

    #[test]
    fn ipv6_is_grouped_by_64() {
        assert_eq!(key("2001:db8::1"), key("2001:db8::2"));
        assert_eq!(key("2001:db8::1"), key("2001:db8:0:0:ffff:ffff:ffff:ffff"));
        assert_ne!(key("2001:db8::1"), key("2001:db8:0:1::1"));
        assert_ne!(key("192.0.2.1"), key("192.0.2.2"));
        assert_eq!(key("::ffff:192.0.2.1"), key("192.0.2.1"));
    }

    #[test]
    fn debug_is_redacted() {
        let printed = format!("{:?}", key("192.0.2.123"));
        assert!(!printed.contains("192"), "{printed}");
        let printed = format!("{:?}", key("2001:db8::1"));
        assert!(
            !printed.contains("2001") && !printed.contains("db8"),
            "{printed}"
        );
        assert!(!printed.contains("32"), "{printed}");
    }
}
