//! Générateur injecté d'identifiants publics et de secrets de lien (AD-3).
//!
//! Id public : 128 bits, base64url sans padding, 22 caractères. Secret : 256 bits, base64url sans
//! padding, 43 caractères. Les deux sortent d'un CSPRNG.

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

use data_encoding::BASE64URL_NOPAD;
use rand::Rng;

pub const ID_BYTES: usize = 16;
pub const ID_LEN: usize = 22;
pub const SECRET_BYTES: usize = 32;
pub const SECRET_LEN: usize = 43;

/// Identifiant public, indevinable, sans ordre.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PublicId(String);

impl PublicId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PublicId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Secret de lien. Ni `Display` ni `Debug` en clair : il ne doit jamais finir dans un journal.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    /// Valeur en clair, à ne remettre qu'au porteur du lien.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(<redacted>)")
    }
}

pub trait IdGenerator: Send + Sync {
    fn public_id(&self) -> PublicId;
    fn secret(&self) -> Secret;
}

/// Générateur de production : `rand::rng()`, CSPRNG ChaCha ensemencé par le système.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemIdGenerator;

impl IdGenerator for SystemIdGenerator {
    fn public_id(&self) -> PublicId {
        let mut bytes = [0u8; ID_BYTES];
        rand::rng().fill_bytes(&mut bytes);
        PublicId(BASE64URL_NOPAD.encode(&bytes))
    }

    fn secret(&self) -> Secret {
        let mut bytes = [0u8; SECRET_BYTES];
        rand::rng().fill_bytes(&mut bytes);
        Secret(BASE64URL_NOPAD.encode(&bytes))
    }
}

/// Générateur de test : valeurs de même forme que la production, prévisibles d'une exécution à
/// l'autre (compteur encodé dans les premiers octets).
#[derive(Debug, Default)]
pub struct DeterministicIdGenerator {
    next: AtomicU64,
}

impl DeterministicIdGenerator {
    pub fn new() -> Self {
        Self::default()
    }

    fn bytes<const N: usize>(&self, tag: u8) -> [u8; N] {
        let n = self.next.fetch_add(1, Ordering::Relaxed);
        let mut bytes = [0u8; N];
        bytes[0] = tag;
        bytes[1..9].copy_from_slice(&n.to_be_bytes());
        bytes
    }
}

impl IdGenerator for DeterministicIdGenerator {
    fn public_id(&self) -> PublicId {
        PublicId(BASE64URL_NOPAD.encode(&self.bytes::<ID_BYTES>(b'i')))
    }

    fn secret(&self) -> Secret {
        Secret(BASE64URL_NOPAD.encode(&self.bytes::<SECRET_BYTES>(b's')))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_base64url(s: &str) -> bool {
        s.bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
    }

    #[test]
    fn system_values_have_the_contract_shape() {
        let g = SystemIdGenerator;
        let (id, secret) = (g.public_id(), g.secret());
        assert_eq!(id.as_str().len(), ID_LEN);
        assert_eq!(secret.expose().len(), SECRET_LEN);
        assert!(is_base64url(id.as_str()) && is_base64url(secret.expose()));
        assert_eq!(
            BASE64URL_NOPAD
                .decode(id.as_str().as_bytes())
                .unwrap()
                .len(),
            ID_BYTES
        );
        assert_eq!(
            BASE64URL_NOPAD
                .decode(secret.expose().as_bytes())
                .unwrap()
                .len(),
            SECRET_BYTES
        );
        assert_ne!(g.secret(), g.secret());
        assert_ne!(g.public_id(), g.public_id());
    }

    #[test]
    fn deterministic_values_are_distinct_and_well_formed() {
        let g = DeterministicIdGenerator::new();
        let a = g.public_id();
        let b = g.public_id();
        assert_ne!(a, b);
        assert_eq!(a.as_str().len(), ID_LEN);
        assert_eq!(g.secret().expose().len(), SECRET_LEN);
        let again = DeterministicIdGenerator::new();
        assert_eq!(again.public_id(), a);
    }

    #[test]
    fn secret_debug_is_redacted() {
        let s = SystemIdGenerator.secret();
        assert!(!format!("{s:?}").contains(s.expose()));
    }
}
