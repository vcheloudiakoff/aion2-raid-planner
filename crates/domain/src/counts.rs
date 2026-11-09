//! Comptes publics d'un événement (AD-4 *Données exposées*, AD-6) : par classe, par rôle, et rôles
//! manquants. Calculés côté serveur, jamais côté client, et sans aucune donnée nominative.
//!
//! Règle : seules les candidatures comptées ([`crate::candidature::CandidatureStatus::is_counted`])
//! entrent ici. Le rôle d'une classe se lit dans la référence ; une classe que la référence ne connaît
//! plus compte sous son propre id (repli d'AD-6). Le manque d'un rôle est la somme des `min_count`
//! des classes de ce rôle, moins les candidatures comptées de ce rôle ; seuls les manques strictement
//! positifs sont rendus, triés par id de rôle.
//!
//! [`slot_coverage`] sert l'organisateur (grille des disponibilités, verrouillage, composition) :
//! par créneau, les seules candidatures `accepted` qui l'ont coché, au total et par classe.

use std::collections::{BTreeMap, BTreeSet};

use crate::candidature::CandidatureStatus;
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

/// Une candidature vue par [`slot_coverage`] : son statut, sa classe et les ids des créneaux cochés.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoverageInput<'a> {
    pub status: CandidatureStatus,
    pub class: &'a str,
    pub slot_ids: &'a [&'a str],
}

/// Couverture d'un créneau par les candidatures `accepted`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotCoverage {
    pub slot_id: String,
    /// Candidatures `accepted` qui ont coché ce créneau.
    pub available_accepted: u32,
    /// Les mêmes, par classe : une entrée par classe bornée (zéros compris), plus toute classe
    /// d'une candidature `accepted` sans borne (zéro compris, pour que chaque créneau ait les mêmes
    /// lignes).
    pub coverage: BTreeMap<String, u32>,
}

/// Une entrée par créneau de `slots`, dans cet ordre. `bounds` : les classes bornées de
/// l'événement. Seules les candidatures `accepted` comptent ; `pending`, `rejected` et `withdrawn`
/// jamais. Un id de créneau coché qui n'est pas dans `slots` est ignoré.
pub fn slot_coverage(
    slots: &[&str],
    bounds: &[&str],
    candidatures: &[CoverageInput<'_>],
) -> Vec<SlotCoverage> {
    let accepted: Vec<&CoverageInput<'_>> = candidatures
        .iter()
        .filter(|c| c.status == CandidatureStatus::Accepted)
        .collect();
    let classes: BTreeSet<&str> = bounds
        .iter()
        .copied()
        .chain(accepted.iter().map(|c| c.class))
        .collect();
    slots
        .iter()
        .map(|&slot| {
            let mut coverage: BTreeMap<String, u32> =
                classes.iter().map(|&c| (c.to_owned(), 0)).collect();
            let mut available_accepted = 0;
            for c in accepted.iter().filter(|c| c.slot_ids.contains(&slot)) {
                available_accepted += 1;
                *coverage.entry(c.class.to_owned()).or_default() += 1;
            }
            SlotCoverage {
                slot_id: slot.to_owned(),
                available_accepted,
                coverage,
            }
        })
        .collect()
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
            icon: None,
            color: None,
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

    fn input<'a>(
        status: CandidatureStatus,
        class: &'a str,
        slot_ids: &'a [&'a str],
    ) -> CoverageInput<'a> {
        CoverageInput {
            status,
            class,
            slot_ids,
        }
    }

    #[test]
    fn coverage_counts_only_accepted_per_slot_and_class() {
        use CandidatureStatus::*;
        let c = slot_coverage(
            &["a", "b"],
            &["cleric", "gladiator", "templar"],
            &[
                input(Accepted, "cleric", &["a"]),
                input(Accepted, "templar", &["a", "b"]),
                input(Pending, "cleric", &["a", "b"]),
                input(Rejected, "gladiator", &["a"]),
                input(Withdrawn, "gladiator", &["a", "b"]),
            ],
        );
        assert_eq!(
            c,
            vec![
                SlotCoverage {
                    slot_id: "a".into(),
                    available_accepted: 2,
                    coverage: BTreeMap::from([
                        ("cleric".into(), 1),
                        ("gladiator".into(), 0),
                        ("templar".into(), 1),
                    ]),
                },
                SlotCoverage {
                    slot_id: "b".into(),
                    available_accepted: 1,
                    coverage: BTreeMap::from([
                        ("cleric".into(), 0),
                        ("gladiator".into(), 0),
                        ("templar".into(), 1),
                    ]),
                },
            ]
        );
    }

    #[test]
    fn coverage_adds_an_unbounded_accepted_class_but_never_an_excluded_one() {
        use CandidatureStatus::*;
        let c = slot_coverage(
            &["a", "b"],
            &["cleric"],
            &[
                input(Accepted, "bard", &["a"]),
                input(Pending, "ghost", &["a"]),
                input(Rejected, "ghost", &["a"]),
            ],
        );
        assert_eq!(
            c[0].coverage,
            BTreeMap::from([("bard".into(), 1), ("cleric".into(), 0)])
        );
        // Même lignes sur un créneau où la classe sans borne n'est pas disponible.
        assert_eq!(
            c[1].coverage,
            BTreeMap::from([("bard".into(), 0), ("cleric".into(), 0)])
        );
        assert_eq!(c[1].available_accepted, 0);
    }

    #[test]
    fn coverage_of_a_slot_nobody_checked_is_all_zeros_in_slot_order() {
        use CandidatureStatus::*;
        let c = slot_coverage(
            &["late", "early"],
            &["templar", "cleric"],
            &[input(Accepted, "cleric", &["early", "unknown"])],
        );
        assert_eq!(
            c.iter().map(|s| s.slot_id.as_str()).collect::<Vec<_>>(),
            ["late", "early"]
        );
        assert_eq!(c[0].available_accepted, 0);
        assert_eq!(
            c[0].coverage,
            BTreeMap::from([("cleric".into(), 0), ("templar".into(), 0)])
        );
        assert_eq!(c[1].available_accepted, 1);
        assert!(slot_coverage(&[], &["cleric"], &[]).is_empty());
    }
}
