//! Comptes publics d'un événement (AD-4 *Données exposées*, AD-6) : par classe, par rôle, et rôles
//! manquants. Calculés côté serveur, jamais côté client, et sans aucune donnée nominative.
//!
//! Règle : seules les candidatures comptées ([`crate::candidature::CandidatureStatus::is_counted`])
//! entrent ici. Le rôle d'une classe se lit dans la référence ; une classe que la référence ne connaît
//! plus compte sous son propre id (repli d'AD-6). Le manque d'un rôle est la somme des `min_count`
//! des classes de ce rôle, moins les candidatures comptées de ce rôle ; seuls les manques strictement
//! positifs sont rendus, triés par id de rôle.

use std::collections::BTreeMap;

use crate::reference::ReferenceSet;

/// Borne minimale d'une classe dans l'événement (ligne `class_bounds`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassMin {
    pub class: String,
    pub min: u32,
}

/// Manque d'un rôle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingRole {
    pub role: String,
    pub missing: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PublicCounts {
    /// Une entrée par classe bornée, zéros compris.
    pub by_class: BTreeMap<String, u32>,
    /// Une entrée par rôle d'une classe bornée ou d'une candidature comptée, zéros compris.
    pub by_role: BTreeMap<String, u32>,
    /// Manques strictement positifs, triés par id de rôle.
    pub missing_roles: Vec<MissingRole>,
}

/// Rôle d'une classe, ou l'id de la classe si la référence ne la connaît pas.
pub fn role_of<'a>(reference: &'a ReferenceSet, class: &'a str) -> &'a str {
    reference
        .classes
        .iter()
        .find(|c| c.id == class)
        .map_or(class, |c| c.role.as_str())
}

/// `counted` : la classe de chaque candidature comptée, une entrée par candidature.
pub fn public_counts(
    reference: &ReferenceSet,
    bounds: &[ClassMin],
    counted: &[String],
) -> PublicCounts {
    let mut by_class: BTreeMap<String, u32> = BTreeMap::new();
    let mut by_role: BTreeMap<String, u32> = BTreeMap::new();
    let mut wanted: BTreeMap<String, u32> = BTreeMap::new();
    for b in bounds {
        by_class.entry(b.class.clone()).or_default();
        let role = role_of(reference, &b.class).to_owned();
        by_role.entry(role.clone()).or_default();
        *wanted.entry(role).or_default() += b.min;
    }
    for class in counted {
        *by_class.entry(class.clone()).or_default() += 1;
        *by_role
            .entry(role_of(reference, class).to_owned())
            .or_default() += 1;
    }
    let missing_roles = wanted
        .into_iter()
        .filter_map(|(role, want)| {
            let have = by_role.get(&role).copied().unwrap_or(0);
            (want > have).then(|| MissingRole {
                role,
                missing: want - have,
            })
        })
        .collect();
    PublicCounts {
        by_class,
        by_role,
        missing_roles,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference::{Class, FactionEntry, Factions, Label};

    fn label(en: &str) -> Label {
        Label {
            en: en.into(),
            fr: None,
        }
    }

    fn class(id: &str, role: &str) -> Class {
        Class {
            id: id.into(),
            role: role.into(),
            default_min: 0,
            default_max: 10,
            label: label(id),
            enabled: true,
        }
    }

    fn reference() -> ReferenceSet {
        ReferenceSet {
            regions: vec![],
            factions: Factions {
                elyos: FactionEntry { label: label("E") },
                asmodian: FactionEntry { label: label("A") },
            },
            servers: vec![],
            room_label_charset: "R".into(),
            room_label_max: 4,
            instances: vec![],
            classes: vec![
                class("cleric", "healer"),
                class("chanter", "healer"),
                class("gladiator", "dps"),
                class("templar", "tank"),
            ],
            roles: vec![],
        }
    }

    fn min(class: &str, min: u32) -> ClassMin {
        ClassMin {
            class: class.into(),
            min,
        }
    }

    fn strings(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn counts_classes_roles_and_missing() {
        let c = public_counts(
            &reference(),
            &[
                min("cleric", 2),
                min("chanter", 1),
                min("gladiator", 0),
                min("templar", 1),
            ],
            &strings(&["cleric", "gladiator", "gladiator", "templar"]),
        );
        assert_eq!(
            c.by_class,
            BTreeMap::from([
                ("chanter".into(), 0),
                ("cleric".into(), 1),
                ("gladiator".into(), 2),
                ("templar".into(), 1),
            ])
        );
        assert_eq!(
            c.by_role,
            BTreeMap::from([("dps".into(), 2), ("healer".into(), 1), ("tank".into(), 1)])
        );
        assert_eq!(
            c.missing_roles,
            vec![MissingRole {
                role: "healer".into(),
                missing: 2
            }]
        );
    }

    #[test]
    fn empty_event_has_zeros_and_every_minimum_missing() {
        let c = public_counts(&reference(), &[min("templar", 1), min("cleric", 2)], &[]);
        assert_eq!(
            c.by_class,
            BTreeMap::from([("cleric".into(), 0), ("templar".into(), 0)])
        );
        assert_eq!(
            c.missing_roles,
            vec![
                MissingRole {
                    role: "healer".into(),
                    missing: 2
                },
                MissingRole {
                    role: "tank".into(),
                    missing: 1
                },
            ]
        );
    }

    #[test]
    fn surplus_never_goes_negative() {
        let c = public_counts(
            &reference(),
            &[min("cleric", 1)],
            &strings(&["cleric", "cleric", "chanter"]),
        );
        assert!(c.missing_roles.is_empty());
        assert_eq!(c.by_role["healer"], 3);
    }

    #[test]
    fn unknown_class_counts_under_its_own_id() {
        let c = public_counts(
            &reference(),
            &[min("bard", 1), min("cleric", 0)],
            &strings(&["bard", "ghost"]),
        );
        assert_eq!(c.by_class["bard"], 1);
        assert_eq!(c.by_class["ghost"], 1);
        assert_eq!(c.by_role["bard"], 1);
        assert_eq!(c.by_role["ghost"], 1);
        assert_eq!(c.by_role["healer"], 0);
        assert!(c.missing_roles.is_empty());
        assert_eq!(role_of(&reference(), "bard"), "bard");
        assert_eq!(role_of(&reference(), "cleric"), "healer");
    }
}
