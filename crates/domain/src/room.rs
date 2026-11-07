//! Nom de salle d'un groupe (AD-18) : composé par le serveur depuis `ROOM_NAME_TEMPLATE`, au
//! verrouillage, indépendant de la langue. Le code de quatre caractères Crockford est tiré par
//! l'appelant (aléa injecté) ; ici, seulement la forme du gabarit et la composition.

use std::fmt;

/// Gabarit par défaut, sans le CP (décision de Vincent, 2026-11-07).
pub const DEFAULT_TEMPLATE: &str = "{label}-{code}";

/// Longueur du code, en caractères.
pub const CODE_LEN: usize = 4;

/// Alphabet de Crockford : chiffres et majuscules sans `I`, `L`, `O` ni `U`.
pub const CODE_ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// Marge accordée au gabarit au-delà de `room_label_max`, en caractères `[ASSUMPTION]` : le nom le
/// plus long (libellé à sa longueur maximale, littéraux et code) tient dans `room_label_max + 5`.
pub const NAME_MARGIN: usize = 5;

const LABEL: &str = "{label}";
const CODE: &str = "{code}";

/// Morceau d'un gabarit.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Part {
    Literal(String),
    Label,
    Code,
}

/// Gabarit validé : jetons `{label}` et `{code}` seulement, `{code}` exactement une fois, littéraux
/// en ASCII imprimable sans accolade.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoomTemplate {
    parts: Vec<Part>,
}

/// Refus d'un gabarit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemplateError {
    /// Une accolade ouvre autre chose que `{label}` ou `{code}`, ou ne se ferme pas.
    UnknownToken,
    /// `{code}` absent ou répété.
    CodeCount,
    /// Un littéral sort de l'ASCII imprimable (`}` isolé compris).
    Literal,
}

impl fmt::Display for TemplateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::UnknownToken => "unknown token",
            Self::CodeCount => "{code} must appear exactly once",
            Self::Literal => "literal outside printable ASCII",
        })
    }
}

impl RoomTemplate {
    pub fn parse(s: &str) -> Result<Self, TemplateError> {
        let mut parts = Vec::new();
        let mut literal = String::new();
        let mut rest = s;
        while let Some(c) = rest.chars().next() {
            if c == '{' {
                let part = if rest.starts_with(LABEL) {
                    rest = &rest[LABEL.len()..];
                    Part::Label
                } else if rest.starts_with(CODE) {
                    rest = &rest[CODE.len()..];
                    Part::Code
                } else {
                    return Err(TemplateError::UnknownToken);
                };
                if !literal.is_empty() {
                    parts.push(Part::Literal(std::mem::take(&mut literal)));
                }
                parts.push(part);
                continue;
            }
            // Espace compris ; ni contrôle, ni accolade, ni caractère hors ASCII.
            if !(c == ' ' || c.is_ascii_graphic()) || c == '}' {
                return Err(TemplateError::Literal);
            }
            literal.push(c);
            rest = &rest[c.len_utf8()..];
        }
        if !literal.is_empty() {
            parts.push(Part::Literal(literal));
        }
        if parts.iter().filter(|p| **p == Part::Code).count() != 1 {
            return Err(TemplateError::CodeCount);
        }
        Ok(Self { parts })
    }

    /// Longueur du nom le plus long, avec un libellé de `room_label_max` caractères.
    pub fn max_len(&self, room_label_max: u32) -> usize {
        self.parts
            .iter()
            .map(|p| match p {
                Part::Literal(s) => s.len(),
                Part::Label => room_label_max as usize,
                Part::Code => CODE_LEN,
            })
            .sum()
    }

    /// Vrai si le nom le plus long tient dans `room_label_max + NAME_MARGIN`.
    pub fn fits(&self, room_label_max: u32) -> bool {
        self.max_len(room_label_max) <= room_label_max as usize + NAME_MARGIN
    }
}

impl Default for RoomTemplate {
    fn default() -> Self {
        Self::parse(DEFAULT_TEMPLATE).expect("default template is valid")
    }
}

/// Vrai si `code` est un code de salle : [`CODE_LEN`] caractères de [`CODE_ALPHABET`].
pub fn is_room_code(code: &str) -> bool {
    code.len() == CODE_LEN && code.bytes().all(|b| CODE_ALPHABET.contains(&b))
}

/// Nom de salle : le gabarit, `{label}` remplacé par le libellé figé du groupe, `{code}` par le code.
pub fn compose(template: &RoomTemplate, label: &str, code: &str) -> String {
    let mut name = String::new();
    for part in &template.parts {
        match part {
            Part::Literal(s) => name.push_str(s),
            Part::Label => name.push_str(label),
            Part::Code => name.push_str(code),
        }
    }
    name
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_template_composes_label_dash_code() {
        let t = RoomTemplate::default();
        assert_eq!(t, RoomTemplate::parse("{label}-{code}").unwrap());
        assert_eq!(compose(&t, "LUDRA", "K7Q2"), "LUDRA-K7Q2");
        // 8 + 1 + 4 : juste la marge.
        assert_eq!(t.max_len(8), 13);
        assert!(t.fits(8));
    }

    #[test]
    fn other_valid_templates() {
        for (s, name) in [
            ("{code}", "K7Q2"),
            ("{code} {label}", "K7Q2 LUDRA"),
            ("R{label}{code}", "RLUDRAK7Q2"),
            ("{label}{code}", "LUDRAK7Q2"),
            ("{label}{label}{code}", "LUDRALUDRAK7Q2"),
        ] {
            let t = RoomTemplate::parse(s).unwrap_or_else(|e| panic!("{s}: {e}"));
            assert_eq!(compose(&t, "LUDRA", "K7Q2"), name, "{s}");
        }
    }

    #[test]
    fn invalid_templates_name_their_fault() {
        for (s, e) in [
            ("", TemplateError::CodeCount),
            ("{label}", TemplateError::CodeCount),
            ("{label}-{code}-{code}", TemplateError::CodeCount),
            ("{label}-{code} {cp}+", TemplateError::UnknownToken),
            ("{label}-{code", TemplateError::UnknownToken),
            ("{LABEL}-{code}", TemplateError::UnknownToken),
            ("{{code}", TemplateError::UnknownToken),
            ("{code}}", TemplateError::Literal),
            ("{label}é{code}", TemplateError::Literal),
            ("{label}\t{code}", TemplateError::Literal),
        ] {
            assert_eq!(RoomTemplate::parse(s), Err(e), "{s:?}");
        }
    }

    #[test]
    fn the_longest_name_must_fit_the_margin() {
        // Un littéral de plus que le défaut, avec le libellé : hors marge.
        let t = RoomTemplate::parse("{label}--{code}").unwrap();
        assert_eq!(t.max_len(8), 14);
        assert!(!t.fits(8));
        // Sans libellé, la marge porte sur `room_label_max + 5` entier.
        let t = RoomTemplate::parse("RAID-ROOM{code}").unwrap();
        assert_eq!(t.max_len(8), 13);
        assert!(t.fits(8));
        assert!(!RoomTemplate::parse("RAID-ROOM-{code}").unwrap().fits(8));
        // Libellé deux fois : il compte deux fois.
        assert!(!RoomTemplate::parse("{label}{label}{code}").unwrap().fits(8));
    }

    #[test]
    fn room_codes_are_four_crockford_characters() {
        assert_eq!(CODE_ALPHABET.len(), 32);
        for c in b"ILOU" {
            assert!(!CODE_ALPHABET.contains(c));
        }
        assert!(is_room_code("K7Q2"));
        assert!(is_room_code("0000"));
        for bad in [
            "K7Q", "K7Q2X", "k7q2", "K7QI", "K7QO", "K7QU", "K7QL", "K7-2",
        ] {
            assert!(!is_room_code(bad), "{bad}");
        }
    }
}
