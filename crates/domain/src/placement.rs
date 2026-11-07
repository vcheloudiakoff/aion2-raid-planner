//! Placement des groupes (AD-4 *Placement*, SPEC de l'epic 3 CAP-3 et CAP-6) : règles pures de
//! `apply_placement`, avertissements de la vue organisateur, place et coéquipiers visibles de la
//! vue participant ([`participant_placement`], [`teammates`]).
//!
//! [`validate`] ne refuse que les violations structurelles : candidature non plaçable ou en double,
//! groupe inconnu ou en sommeil, place hors de la disposition figée du groupe, place déjà prise.
//! Une indisponibilité ou une borne de classe n'est jamais une erreur : [`warnings`] les calcule à
//! la lecture, rien n'en est stocké. Le banc est renuméroté de façon contiguë dans chaque groupe,
//! dans l'ordre reçu `[ASSUMPTION]`.

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::candidature::{CANDIDATURES_MAX, CandidatureStatus};

/// Une entrée du corps d'`apply_placement`, nombres tels que reçus : titulaire (`party` et
/// `index`) ou banc (`bench_index`), l'un ou l'autre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacementInput {
    pub candidature_id: String,
    pub group_id: String,
    pub party: Option<i64>,
    pub index: Option<i64>,
    pub bench_index: Option<i64>,
}

/// Un groupe de l'événement vu par [`validate`] : `retained` s'il fait partie du verrouillage
/// courant, et sa disposition figée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutGroup<'a> {
    pub id: &'a str,
    pub retained: bool,
    pub parties: u32,
    pub party_size: u32,
}

/// Place d'une candidature dans son groupe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Seat {
    /// Titulaire : équipe `party` (1…parties), slot `index` (1…party_size).
    Starter { party: u32, index: u32 },
    /// Banc, rang `bench_index` à partir de 1.
    Bench { bench_index: u32 },
}

/// Un placement validé, banc renuméroté.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placement {
    pub candidature_id: String,
    pub group_id: String,
    pub seat: Seat,
}

/// Violation structurelle d'un placement : 400, avec le chemin du champ fautif.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacementError {
    /// Candidature inconnue, d'un autre événement, ou non `accepted`.
    CandidatureNotPlaceable,
    /// Même candidature déjà vue plus haut dans la liste.
    DuplicateCandidature,
    /// Groupe inconnu, d'un autre événement, ou en sommeil.
    UnknownGroup,
    /// Ni titulaire ni banc, les deux, ou une place hors de la disposition du groupe.
    OutOfLayout,
    /// Une entrée plus haut occupe déjà ce slot ou ce rang de banc.
    SlotTaken,
}

/// Chemin JSON camelCase du champ `field` de l'entrée `i`.
fn path(i: usize, field: &str) -> String {
    format!("placements.{i}.{field}")
}

/// Place demandée, avant renumérotation du banc : `Err` = chemin du champ hors disposition.
fn seat_of(entry: &PlacementInput, group: &LayoutGroup<'_>) -> Result<Seat, &'static str> {
    let in_range = |v: i64, max: u32| v >= 1 && v <= i64::from(max);
    match (entry.party, entry.index, entry.bench_index) {
        (Some(party), Some(index), None) => {
            if !in_range(party, group.parties) {
                Err("party")
            } else if !in_range(index, group.party_size) {
                Err("index")
            } else {
                // Bornés par des `u32` juste au-dessus.
                Ok(Seat::Starter {
                    party: party as u32,
                    index: index as u32,
                })
            }
        }
        // Un rang au-delà du nombre de candidatures permises n'a pas de sens.
        (None, None, Some(bench)) if (1..=CANDIDATURES_MAX).contains(&bench) => Ok(Seat::Bench {
            bench_index: bench as u32,
        }),
        (None, None, Some(_)) => Err("benchIndex"),
        // Les deux à la fois : le banc est de trop.
        (Some(_), Some(_), Some(_)) => Err("benchIndex"),
        // Titulaire incomplet : le champ manquant ; rien du tout : `party`.
        (Some(_), None, _) => Err("index"),
        (None, Some(_), _) | (None, None, None) => Err("party"),
    }
}

/// Vérifie le placement complet des groupes retenus contre les candidatures de l'événement (id et
/// statut) et ses groupes. Toutes les erreurs sont rendues ensemble, dans l'ordre de la liste, avec
/// des chemins `placements.N.<champ>`. En cas de succès, les placements dans l'ordre reçu, le banc
/// de chaque groupe renuméroté 1, 2, 3… dans l'ordre de ses entrées.
pub fn validate(
    entries: &[PlacementInput],
    candidatures: &[(&str, CandidatureStatus)],
    groups: &[LayoutGroup<'_>],
) -> Result<Vec<Placement>, Vec<(String, PlacementError)>> {
    let statuses: HashMap<&str, CandidatureStatus> = candidatures.iter().copied().collect();
    let mut errors = Vec::new();
    let mut seen = HashSet::new();
    let mut taken: HashSet<(&str, Seat)> = HashSet::new();
    let mut seats = Vec::with_capacity(entries.len());
    for (i, entry) in entries.iter().enumerate() {
        let id = entry.candidature_id.as_str();
        if statuses.get(id) != Some(&CandidatureStatus::Accepted) {
            errors.push((
                path(i, "candidatureId"),
                PlacementError::CandidatureNotPlaceable,
            ));
        } else if !seen.insert(id) {
            errors.push((
                path(i, "candidatureId"),
                PlacementError::DuplicateCandidature,
            ));
        }
        let Some(group) = groups.iter().find(|g| g.id == entry.group_id && g.retained) else {
            errors.push((path(i, "groupId"), PlacementError::UnknownGroup));
            continue;
        };
        match seat_of(entry, group) {
            Err(field) => errors.push((path(i, field), PlacementError::OutOfLayout)),
            Ok(seat) => {
                if taken.insert((group.id, seat)) {
                    seats.push(seat);
                } else {
                    let field = match seat {
                        Seat::Starter { .. } => "index",
                        Seat::Bench { .. } => "benchIndex",
                    };
                    errors.push((path(i, field), PlacementError::SlotTaken));
                }
            }
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    let mut bench: HashMap<&str, u32> = HashMap::new();
    Ok(entries
        .iter()
        .zip(seats)
        .map(|(entry, seat)| {
            let seat = match seat {
                Seat::Bench { .. } => {
                    let n = bench.entry(entry.group_id.as_str()).or_insert(0);
                    *n += 1;
                    Seat::Bench { bench_index: *n }
                }
                starter => starter,
            };
            Placement {
                candidature_id: entry.candidature_id.clone(),
                group_id: entry.group_id.clone(),
                seat,
            }
        })
        .collect())
}

/// Une candidature placée, vue par [`warnings`] : sa classe et les ids des créneaux cochés.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placed<'a> {
    pub id: &'a str,
    pub class: &'a str,
    pub slot_ids: &'a [&'a str],
}

/// Bornes d'une classe dans l'événement (ligne `class_bounds`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bound<'a> {
    pub class: &'a str,
    pub min: u32,
    pub max: u32,
}

/// Un titulaire du groupe ; `unavailable` s'il n'a pas coché le créneau du groupe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StarterSeat {
    pub candidature_id: String,
    pub party: u32,
    pub index: u32,
    pub unavailable: bool,
}

/// Un joueur au banc du groupe ; `unavailable` s'il n'a pas coché le créneau du groupe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BenchSeat {
    pub candidature_id: String,
    pub bench_index: u32,
    pub unavailable: bool,
}

/// Compte des titulaires d'une classe hors de ses bornes (« 3 / max 2 », « 1 / min 2 »).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClassWarning {
    pub count: u32,
    pub min: u32,
    pub max: u32,
}

/// Placements et avertissements d'un groupe, calculés à la lecture.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GroupWarnings {
    /// Triés par équipe puis par slot.
    pub starters: Vec<StarterSeat>,
    /// Triés par rang.
    pub bench: Vec<BenchSeat>,
    /// Classes bornées dont le compte de titulaires sort de `[min, max]`, seulement elles
    /// `[ASSUMPTION]` : le banc ne compte pas.
    pub class_warnings: BTreeMap<String, ClassWarning>,
}

/// Avertissements d'un groupe sur le créneau `slot_id` : `seats` sont ses placements,
/// `candidatures` celles de l'événement (une candidature introuvable est indisponible et sans
/// classe), `bounds` les bornes de l'événement. Jamais une erreur.
pub fn warnings(
    slot_id: &str,
    seats: &[(&str, Seat)],
    candidatures: &[Placed<'_>],
    bounds: &[Bound<'_>],
) -> GroupWarnings {
    let by_id: HashMap<&str, &Placed<'_>> = candidatures.iter().map(|c| (c.id, c)).collect();
    let unavailable = |id: &str| by_id.get(id).is_none_or(|c| !c.slot_ids.contains(&slot_id));
    let mut sorted: Vec<(&str, Seat)> = seats.to_vec();
    sorted.sort_by_key(|&(id, seat)| (seat, id));
    let mut out = GroupWarnings::default();
    let mut counts: BTreeMap<&str, u32> = BTreeMap::new();
    for (id, seat) in sorted {
        match seat {
            Seat::Starter { party, index } => {
                if let Some(c) = by_id.get(id) {
                    *counts.entry(c.class).or_default() += 1;
                }
                out.starters.push(StarterSeat {
                    candidature_id: id.to_owned(),
                    party,
                    index,
                    unavailable: unavailable(id),
                });
            }
            Seat::Bench { bench_index } => out.bench.push(BenchSeat {
                candidature_id: id.to_owned(),
                bench_index,
                unavailable: unavailable(id),
            }),
        }
    }
    for b in bounds {
        let count = counts.get(b.class).copied().unwrap_or(0);
        if count < b.min || count > b.max {
            out.class_warnings.insert(
                b.class.to_owned(),
                ClassWarning {
                    count,
                    min: b.min,
                    max: b.max,
                },
            );
        }
    }
    out
}

/// Un groupe de l'événement vu par [`participant_placement`], dans l'ordre des groupes (début de
/// créneau, puis occurrence).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GroupRef<'a> {
    pub id: &'a str,
    pub retained: bool,
}

/// Une ligne de placement de l'événement, avec le statut de sa candidature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Assigned<'a> {
    pub candidature_id: &'a str,
    pub status: CandidatureStatus,
    pub group_id: &'a str,
    pub seat: Seat,
}

/// Place d'une candidature dans le verrouillage courant, vue par son participant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParticipantPlacement<'a> {
    /// Rang du groupe parmi les groupes retenus, à partir de 1 (« G1 »).
    pub group_number: u32,
    pub group_id: &'a str,
    pub seat: Seat,
}

/// Placement de la candidature `me` (SPEC epic 3 CAP-6) : seulement si l'événement est `locked`,
/// que la candidature est `accepted` et qu'elle a une place dans un groupe retenu. Une place dans un
/// groupe en sommeil vaut « non placé ». `groups` dans l'ordre des groupes : le numéro « G1… » se
/// compte sur les seuls retenus, comme côté organisateur.
pub fn participant_placement<'a>(
    me: &str,
    status: CandidatureStatus,
    event_locked: bool,
    groups: &[GroupRef<'a>],
    assignments: &[Assigned<'a>],
) -> Option<ParticipantPlacement<'a>> {
    if !event_locked || status != CandidatureStatus::Accepted {
        return None;
    }
    let mine = assignments.iter().find(|a| a.candidature_id == me)?;
    let number = groups
        .iter()
        .filter(|g| g.retained)
        .position(|g| g.id == mine.group_id)?;
    Some(ParticipantPlacement {
        group_number: u32::try_from(number).ok()? + 1,
        group_id: mine.group_id,
        seat: mine.seat,
    })
}

/// Les coéquipiers visibles d'une équipe : ses titulaires, triés par slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyMates<'a> {
    pub party: u32,
    pub members: Vec<&'a str>,
}

/// Coéquipiers visibles depuis `placement`, la place de `me`, dans un groupe à `parties` équipes :
/// pour un titulaire, une entrée, son équipe, avec ses autres titulaires ; pour un joueur au banc,
/// une entrée par équipe du groupe, dans l'ordre, avec ses titulaires. Membres triés par slot,
/// éventuellement aucun. Jamais `me`, jamais un joueur au banc, jamais un autre groupe, jamais une
/// candidature non `accepted`.
pub fn teammates<'a>(
    me: &str,
    placement: &ParticipantPlacement<'_>,
    parties: u32,
    assignments: &[Assigned<'a>],
) -> Vec<PartyMates<'a>> {
    let shown: Vec<u32> = match placement.seat {
        Seat::Starter { party, .. } => vec![party],
        Seat::Bench { .. } => (1..=parties).collect(),
    };
    let mut seated: Vec<(u32, u32, &'a str)> = assignments
        .iter()
        .filter(|a| {
            a.group_id == placement.group_id
                && a.candidature_id != me
                && a.status == CandidatureStatus::Accepted
        })
        .filter_map(|a| match a.seat {
            Seat::Starter { party, index } => Some((party, index, a.candidature_id)),
            Seat::Bench { .. } => None,
        })
        .collect();
    seated.sort_unstable();
    shown
        .into_iter()
        .map(|party| PartyMates {
            party,
            members: seated
                .iter()
                .filter(|&&(p, _, _)| p == party)
                .map(|&(_, _, id)| id)
                .collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use CandidatureStatus::*;
    use PlacementError as P;

    fn starter(c: &str, g: &str, party: i64, index: i64) -> PlacementInput {
        PlacementInput {
            candidature_id: c.into(),
            group_id: g.into(),
            party: Some(party),
            index: Some(index),
            bench_index: None,
        }
    }

    fn bench(c: &str, g: &str, bench_index: i64) -> PlacementInput {
        PlacementInput {
            candidature_id: c.into(),
            group_id: g.into(),
            party: None,
            index: None,
            bench_index: Some(bench_index),
        }
    }

    fn raw(c: &str, party: Option<i64>, index: Option<i64>, b: Option<i64>) -> PlacementInput {
        PlacementInput {
            candidature_id: c.into(),
            group_id: "g1".into(),
            party,
            index,
            bench_index: b,
        }
    }

    const CANDIDATURES: &[(&str, CandidatureStatus)] = &[
        ("a", Accepted),
        ("b", Accepted),
        ("c", Accepted),
        ("d", Accepted),
        ("p", Pending),
        ("r", Rejected),
        ("w", Withdrawn),
    ];

    fn groups() -> Vec<LayoutGroup<'static>> {
        vec![
            LayoutGroup {
                id: "g1",
                retained: true,
                parties: 2,
                party_size: 5,
            },
            LayoutGroup {
                id: "g2",
                retained: true,
                parties: 1,
                party_size: 4,
            },
            LayoutGroup {
                id: "zz",
                retained: false,
                parties: 2,
                party_size: 5,
            },
        ]
    }

    fn check(entries: &[PlacementInput]) -> Result<Vec<Placement>, Vec<(String, PlacementError)>> {
        validate(entries, CANDIDATURES, &groups())
    }

    fn errs(list: &[(&str, PlacementError)]) -> Vec<(String, PlacementError)> {
        list.iter().map(|(p, e)| ((*p).to_owned(), *e)).collect()
    }

    fn placed(c: &str, g: &str, seat: Seat) -> Placement {
        Placement {
            candidature_id: c.into(),
            group_id: g.into(),
            seat,
        }
    }

    #[test]
    fn a_nominal_placement_keeps_the_order_and_renumbers_the_bench_per_group() {
        let got = check(&[
            starter("a", "g1", 1, 1),
            bench("b", "g1", 7),
            starter("c", "g2", 1, 4),
            bench("d", "g1", 3),
        ])
        .unwrap();
        assert_eq!(
            got,
            vec![
                placed("a", "g1", Seat::Starter { party: 1, index: 1 }),
                placed("b", "g1", Seat::Bench { bench_index: 1 }),
                placed("c", "g2", Seat::Starter { party: 1, index: 4 }),
                placed("d", "g1", Seat::Bench { bench_index: 2 }),
            ]
        );
        assert_eq!(check(&[]), Ok(vec![]));
    }

    #[test]
    fn only_an_accepted_candidature_of_the_event_is_placeable() {
        for id in ["p", "r", "w", "unknown"] {
            assert_eq!(
                check(&[starter(id, "g1", 1, 1)]),
                Err(errs(&[(
                    "placements.0.candidatureId",
                    P::CandidatureNotPlaceable
                )])),
                "{id}"
            );
        }
    }

    #[test]
    fn a_candidature_twice_is_a_duplicate_on_the_second_entry() {
        assert_eq!(
            check(&[starter("a", "g1", 1, 1), bench("a", "g2", 1)]),
            Err(errs(&[(
                "placements.1.candidatureId",
                P::DuplicateCandidature
            )]))
        );
    }

    #[test]
    fn an_unknown_or_sleeping_group_is_unknown() {
        for g in ["zz", "nope"] {
            assert_eq!(
                check(&[starter("a", g, 1, 1)]),
                Err(errs(&[("placements.0.groupId", P::UnknownGroup)])),
                "{g}"
            );
        }
    }

    #[test]
    fn a_seat_outside_the_frozen_layout_names_its_field() {
        for (entry, field) in [
            (raw("a", Some(3), Some(1), None), "party"),
            (raw("a", Some(0), Some(1), None), "party"),
            (raw("a", Some(-1), Some(1), None), "party"),
            (raw("a", Some(1), Some(6), None), "index"),
            (raw("a", Some(1), Some(0), None), "index"),
            (raw("a", Some(1), Some(i64::MAX), None), "index"),
            (raw("a", None, None, Some(0)), "benchIndex"),
            (raw("a", None, None, Some(i64::MAX)), "benchIndex"),
            (
                raw("a", None, None, Some(CANDIDATURES_MAX + 1)),
                "benchIndex",
            ),
            (raw("a", Some(1), Some(1), Some(1)), "benchIndex"),
            (raw("a", Some(1), None, None), "index"),
            (raw("a", Some(1), None, Some(1)), "index"),
            (raw("a", None, Some(1), None), "party"),
            (raw("a", None, None, None), "party"),
        ] {
            assert_eq!(
                check(std::slice::from_ref(&entry)),
                Err(vec![(format!("placements.0.{field}"), P::OutOfLayout)]),
                "{entry:?}"
            );
        }
        // Disposition du groupe lui-même : 1 × 4 pour `g2`.
        assert_eq!(
            check(&[starter("a", "g2", 2, 1), starter("b", "g2", 1, 5)]),
            Err(errs(&[
                ("placements.0.party", P::OutOfLayout),
                ("placements.1.index", P::OutOfLayout),
            ]))
        );
    }

    #[test]
    fn two_entries_on_the_same_slot_or_bench_rank_collide() {
        assert_eq!(
            check(&[
                starter("a", "g1", 1, 1),
                starter("b", "g1", 1, 1),
                bench("c", "g1", 2),
                bench("d", "g1", 2),
            ]),
            Err(errs(&[
                ("placements.1.index", P::SlotTaken),
                ("placements.3.benchIndex", P::SlotTaken),
            ]))
        );
        // Même place dans deux groupes : permis.
        assert!(check(&[starter("a", "g1", 1, 1), starter("b", "g2", 1, 1)]).is_ok());
    }

    #[test]
    fn every_error_comes_back_in_list_order() {
        assert_eq!(
            check(&[
                starter("p", "g1", 1, 1),
                starter("a", "g1", 3, 1),
                starter("a", "zz", 1, 1),
                starter("b", "g1", 1, 1),
            ]),
            Err(errs(&[
                ("placements.0.candidatureId", P::CandidatureNotPlaceable),
                ("placements.1.party", P::OutOfLayout),
                ("placements.2.candidatureId", P::DuplicateCandidature),
                ("placements.2.groupId", P::UnknownGroup),
                ("placements.3.index", P::SlotTaken),
            ]))
        );
    }

    fn cand<'a>(id: &'a str, class: &'a str, slot_ids: &'a [&'a str]) -> Placed<'a> {
        Placed {
            id,
            class,
            slot_ids,
        }
    }

    #[test]
    fn warnings_flag_the_unavailable_and_the_classes_out_of_bounds() {
        let candidatures = [
            cand("a", "cleric", &["s1"]),
            cand("b", "cleric", &["s1", "s2"]),
            cand("c", "cleric", &["s2"]),
            cand("d", "gladiator", &["s1"]),
            cand("e", "templar", &[]),
        ];
        let seats = [
            ("c", Seat::Starter { party: 2, index: 1 }),
            ("a", Seat::Starter { party: 1, index: 2 }),
            ("e", Seat::Bench { bench_index: 2 }),
            ("b", Seat::Starter { party: 1, index: 1 }),
            ("d", Seat::Bench { bench_index: 1 }),
        ];
        let bounds = [
            Bound {
                class: "cleric",
                min: 0,
                max: 2,
            },
            Bound {
                class: "chanter",
                min: 1,
                max: 3,
            },
            Bound {
                class: "gladiator",
                min: 0,
                max: 10,
            },
            Bound {
                class: "templar",
                min: 1,
                max: 1,
            },
        ];
        let w = warnings("s1", &seats, &candidatures, &bounds);
        let starter = |id: &str, party, index, unavailable| StarterSeat {
            candidature_id: id.into(),
            party,
            index,
            unavailable,
        };
        assert_eq!(
            w.starters,
            vec![
                starter("b", 1, 1, false),
                starter("a", 1, 2, false),
                starter("c", 2, 1, true),
            ]
        );
        let benched = |id: &str, bench_index, unavailable| BenchSeat {
            candidature_id: id.into(),
            bench_index,
            unavailable,
        };
        assert_eq!(w.bench, vec![benched("d", 1, false), benched("e", 2, true)]);
        // 3 Clercs pour un max de 2, aucun Chanteur pour un min de 1 ; le Templier au banc ne
        // compte pas ; une classe dans ses bornes n'apparaît pas.
        assert_eq!(
            w.class_warnings,
            BTreeMap::from([
                (
                    "chanter".into(),
                    ClassWarning {
                        count: 0,
                        min: 1,
                        max: 3
                    }
                ),
                (
                    "cleric".into(),
                    ClassWarning {
                        count: 3,
                        min: 0,
                        max: 2
                    }
                ),
                (
                    "templar".into(),
                    ClassWarning {
                        count: 0,
                        min: 1,
                        max: 1
                    }
                ),
            ])
        );
    }

    #[test]
    fn an_empty_group_only_warns_on_minimums_and_a_missing_candidature_is_unavailable() {
        let bounds = [
            Bound {
                class: "cleric",
                min: 1,
                max: 2,
            },
            Bound {
                class: "gladiator",
                min: 0,
                max: 2,
            },
        ];
        // Groupe vide : seul le minimum non nul avertit.
        assert_eq!(
            warnings("s1", &[], &[], &bounds),
            GroupWarnings {
                class_warnings: BTreeMap::from([(
                    "cleric".into(),
                    ClassWarning {
                        count: 0,
                        min: 1,
                        max: 2
                    }
                )]),
                ..GroupWarnings::default()
            }
        );
        let w = warnings(
            "s1",
            &[("ghost", Seat::Starter { party: 1, index: 1 })],
            &[],
            &bounds,
        );
        assert!(w.starters[0].unavailable);
        // Sans candidature, pas de classe : le Clerc manque toujours.
        assert_eq!(w.class_warnings.keys().collect::<Vec<_>>(), ["cleric"]);
    }

    fn at<'a>(c: &'a str, g: &'a str, seat: Seat) -> Assigned<'a> {
        Assigned {
            candidature_id: c,
            status: Accepted,
            group_id: g,
            seat,
        }
    }

    const fn st(party: u32, index: u32) -> Seat {
        Seat::Starter { party, index }
    }

    const fn bn(bench_index: u32) -> Seat {
        Seat::Bench { bench_index }
    }

    /// G1 = `g1`, `zz` en sommeil entre les deux, G2 = `g2`.
    const GROUPS: &[GroupRef<'static>] = &[
        GroupRef {
            id: "g1",
            retained: true,
        },
        GroupRef {
            id: "zz",
            retained: false,
        },
        GroupRef {
            id: "g2",
            retained: true,
        },
    ];

    fn roster() -> Vec<Assigned<'static>> {
        let mut pending = at("p", "g1", st(2, 5));
        pending.status = Pending;
        vec![
            at("a1", "g1", st(1, 2)),
            at("a2", "g1", st(1, 1)),
            at("b4", "g1", st(2, 4)),
            at("b1", "g1", st(2, 1)),
            at("b2", "g1", st(2, 2)),
            at("bench1", "g1", bn(1)),
            at("bench2", "g1", bn(2)),
            pending,
            at("other", "g2", st(2, 3)),
            at("other_bench", "g2", bn(1)),
            at("asleep", "zz", st(2, 1)),
        ]
    }

    fn place(me: &str) -> Option<ParticipantPlacement<'static>> {
        participant_placement(me, Accepted, true, GROUPS, &roster())
    }

    #[test]
    fn the_placement_numbers_the_group_among_retained_groups_only() {
        assert_eq!(
            place("b4"),
            Some(ParticipantPlacement {
                group_number: 1,
                group_id: "g1",
                seat: st(2, 4),
            })
        );
        // `zz` en sommeil ne compte pas : `g2` est G2.
        assert_eq!(
            place("other_bench"),
            Some(ParticipantPlacement {
                group_number: 2,
                group_id: "g2",
                seat: bn(1),
            })
        );
    }

    #[test]
    fn no_placement_unless_locked_accepted_and_seated_in_a_retained_group() {
        // Sans ligne, ou dans un groupe en sommeil : non placé.
        assert_eq!(place("nobody"), None);
        assert_eq!(place("asleep"), None);
        // Événement non verrouillé.
        assert_eq!(
            participant_placement("b4", Accepted, false, GROUPS, &roster()),
            None
        );
        // Candidature non acceptée, même avec une ligne.
        for status in [Pending, Rejected, Withdrawn] {
            assert_eq!(
                participant_placement("b4", status, true, GROUPS, &roster()),
                None,
                "{status:?}"
            );
        }
    }

    fn mates(me: &str) -> Vec<(u32, Vec<&'static str>)> {
        let roster = roster();
        let placement = place(me).unwrap();
        teammates(me, &placement, 2, &roster)
            .into_iter()
            .map(|t| (t.party, t.members))
            .collect()
    }

    #[test]
    fn a_starter_sees_the_other_starters_of_their_party_by_slot() {
        // Ni lui-même, ni l'autre équipe, ni le banc, ni le `pending`, ni l'autre groupe.
        assert_eq!(mates("b4"), [(2, vec!["b1", "b2"])]);
        assert_eq!(mates("a1"), [(1, vec!["a2"])]);
        // Seul de son équipe dans G2 : son équipe, vide.
        assert_eq!(mates("other"), [(2, vec![])]);
    }

    #[test]
    fn a_benched_player_sees_the_starters_of_every_party_in_order() {
        assert_eq!(
            mates("bench1"),
            [(1, vec!["a2", "a1"]), (2, vec!["b1", "b2", "b4"])]
        );
        // Équipe 1 de G2 vide : présente quand même.
        assert_eq!(mates("other_bench"), [(1, vec![]), (2, vec!["other"])]);
    }
}
