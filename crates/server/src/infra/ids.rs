//! Générateur injecté d'identifiants publics, de secrets de lien (AD-3) et de codes de salle
//! (AD-18).
//!
//! Id public : 128 bits, base64url sans padding, 22 caractères. Secret : 256 bits, base64url sans
//! padding, 43 caractères. Code de salle : 4 caractères Crockford (`domain::room`). Les trois
//! sortent d'un CSPRNG.

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

use data_encoding::BASE64URL_NOPAD;
use domain::room::{CODE_ALPHABET, CODE_LEN};
use rand::Rng;

pub const ID_BYTES: usize = 16;
pub const ID_LEN: usize = 22;
pub const SECRET_BYTES: usize = 32;
pub const SECRET_LEN: usize = 43;
/// Nonce du jeton de formulaire (AD-7).
pub const NONCE_BYTES: usize = 16;

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
    /// Octets aléatoires du jeton de formulaire.
    fn nonce(&self) -> [u8; NONCE_BYTES];
    /// Code d'un nom de salle : [`CODE_LEN`] caractères de [`CODE_ALPHABET`].
    fn room_code(&self) -> String;
}

/// Code de salle tiré de `bytes` : 32 symboles, 5 bits par octet, sans biais.
fn crockford(bytes: [u8; CODE_LEN]) -> String {
    bytes
        .iter()
        .map(|b| char::from(CODE_ALPHABET[usize::from(b & 0x1f)]))
        .collect()
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

    fn nonce(&self) -> [u8; NONCE_BYTES] {
        let mut bytes = [0u8; NONCE_BYTES];
        rand::rng().fill_bytes(&mut bytes);
        bytes
    }

    fn room_code(&self) -> String {
        let mut bytes = [0u8; CODE_LEN];
        rand::rng().fill_bytes(&mut bytes);
        crockford(bytes)
    }
}

/// Générateur de test : valeurs de même forme que la production, prévisibles d'une exécution à
/// l'autre (compteur encodé dans les premiers octets).
#[derive(Debug, Default)]
pub struct DeterministicIdGenerator {
    next: AtomicU64,
    /// Compteur propre aux codes de salle : `0000`, `0001`…, indépendant des ids.
    next_code: AtomicU64,
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

    fn nonce(&self) -> [u8; NONCE_BYTES] {
        self.bytes::<NONCE_BYTES>(b'n')
    }

    fn room_code(&self) -> String {
        let n = self.next_code.fetch_add(1, Ordering::Relaxed);
        // Chiffres de poids fort d'abord, en base 32.
        let digit = |shift: u32| ((n >> shift) & 0x1f) as u8;
        crockford([digit(15), digit(10), digit(5), digit(0)])
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
        assert_ne!(g.nonce(), g.nonce());
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
    fn room_codes_are_crockford() {
        let g = SystemIdGenerator;
        for _ in 0..64 {
            assert!(domain::room::is_room_code(&g.room_code()));
        }
        let d = DeterministicIdGenerator::new();
        let first: Vec<_> = (0..3).map(|_| d.room_code()).collect();
        assert_eq!(first, ["0000", "0001", "0002"]);
        // Les ids ne décalent pas les codes.
        d.public_id();
        assert_eq!(d.room_code(), "0003");
        assert!((0..40).all(|_| domain::room::is_room_code(&d.room_code())));
    }

    #[test]
    fn secret_debug_is_redacted() {
        let s = SystemIdGenerator.secret();
        assert!(!format!("{s:?}").contains(s.expose()));
    }
}
