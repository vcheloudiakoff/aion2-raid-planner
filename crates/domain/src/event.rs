//! Création d'un événement (AD-4) : règles pures sur la saisie brute de l'organisateur.
//!
//! [`validate_new_event`] prend le `ReferenceSet` une seule fois (AD-6) et `now` de l'horloge injectée
//! (AD-11). Toutes les erreurs de champ sont rendues ensemble, dans l'ordre des champs du formulaire :
//! la couche `http` prend la première pour `code` et les autres pour `fields`.

use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::Timestamp;
use crate::reference::ReferenceSet;
use crate::validate;

/// Nombre de créneaux permis par événement (SPEC de l'epic 2, *Assumptions*).
pub const SLOTS_MIN: usize = 1;
pub const SLOTS_MAX: usize = 14;
/// Durée d'un créneau en minutes. `[ASSUMPTION]` de la story 2-1.
pub const DURATION_MIN_MINUTES: i64 = 15;
pub const DURATION_MAX_MINUTES: i64 = 720;

/// Portée de faction : les deux factions ensemble, ou une seule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FactionScope {
    All,
    Elyos,
    Asmodian,
}

impl FactionScope {
    /// Valeur de la colonne `faction_scope`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Elyos => "elyos",
            Self::Asmodian => "asmodian",
        }
    }
}

/// Présence dans la liste publique, choisie par l'organisateur.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    #[default]
    Listed,
    Unlisted,
}

impl Visibility {
    /// Valeur de la colonne `visibility`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Listed => "listed",
            Self::Unlisted => "unlisted",
        }
    }
}

/// Créneau saisi : durée absente = durée de l'instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewSlot {
    pub starts_at: Timestamp,
    pub duration_min: Option<i64>,
}

/// Bornes saisies pour une classe, entiers bruts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewBound {
    pub min: i64,
    pub max: i64,
}

/// Saisie brute de l'organisateur, avant toute règle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewEvent {
    pub instance: String,
    pub region: String,
    pub faction_scope: FactionScope,
    pub slots: Vec<NewSlot>,
    pub note: String,
    /// Vide = absent.
    pub discord_url: Option<String>,
    pub visibility: Visibility,
    pub min_cp: Option<i64>,
    /// Classe omise = bornes par défaut de la référence.
    pub class_bounds: BTreeMap<String, NewBound>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidSlot {
    pub starts_at: Timestamp,
    pub duration_min: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassBound {
    pub class: String,
    pub min: u32,
    pub max: u32,
}

/// Événement prêt à écrire : une borne par classe active, dans l'ordre de la référence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidEvent {
    pub instance: String,
    pub region: String,
    pub faction_scope: FactionScope,
    pub slots: Vec<ValidSlot>,
    pub note: String,
    pub discord_url: Option<String>,
    pub visibility: Visibility,
    pub min_cp: Option<u32>,
    pub class_bounds: Vec<ClassBound>,
}

/// Cas d'erreur d'un champ ; la couche `http` le traduit en code `<entité>.<cas>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventError {
    ReferenceUnknown,
    FactionScopeNotAllowed,
    SlotInPast,
    SlotCountOutOfRange,
    SlotDuplicate,
    SlotDurationOutOfRange,
    NoteInvalid,
    DiscordUrlInvalid,
    MinCpOutOfRange,
    ClassBoundsInvalid,
}

/// Chemin JSON camelCase du champ fautif (`slots.2.startsAt`) et son cas.
pub type FieldErrors = Vec<(String, EventError)>;

/// Applique toutes les règles de création ; rend toutes les erreurs, dans l'ordre des champs.
pub fn validate_new_event(
    reference: &ReferenceSet,
    now: Timestamp,
    input: NewEvent,
) -> Result<ValidEvent, FieldErrors> {
    let mut errors: FieldErrors = Vec::new();
    let mut fail = |path: String, e: EventError| errors.push((path, e));

    let instance = reference
        .instances
        .iter()
        .find(|i| i.enabled && i.id == input.instance);
    if instance.is_none() {
        fail("instance".into(), EventError::ReferenceUnknown);
    }
    if !reference
        .regions
        .iter()
        .any(|r| r.enabled && r.id == input.region)
    {
        fail("region".into(), EventError::ReferenceUnknown);
    }
    if let Some(i) = instance
        && !i.cross_faction
        && input.faction_scope == FactionScope::All
    {
        fail("factionScope".into(), EventError::FactionScopeNotAllowed);
    }

    if !(SLOTS_MIN..=SLOTS_MAX).contains(&input.slots.len()) {
        fail("slots".into(), EventError::SlotCountOutOfRange);
    }
    let mut seen = HashSet::new();
    let mut slots = Vec::with_capacity(input.slots.len());
    for (n, slot) in input.slots.iter().enumerate() {
        if slot.starts_at <= now {
            fail(format!("slots.{n}.startsAt"), EventError::SlotInPast);
        } else if !seen.insert(slot.starts_at) {
            fail(format!("slots.{n}.startsAt"), EventError::SlotDuplicate);
        }
        let duration = slot
            .duration_min
            .or(instance.map(|i| i64::from(i.duration_min)));
        match duration {
            Some(d) if (DURATION_MIN_MINUTES..=DURATION_MAX_MINUTES).contains(&d) => {
                slots.push(ValidSlot {
                    starts_at: slot.starts_at,
                    // Borné ci-dessus : tient dans un `u32`.
                    duration_min: d as u32,
                });
            }
            // Durée de l'instance hors bornes : l'organisateur la corrige sur le créneau.
            Some(_) => fail(
                format!("slots.{n}.durationMin"),
                EventError::SlotDurationOutOfRange,
            ),
            // Instance inconnue, déjà signalée.
            None => {}
        }
    }

    if validate::note(&input.note).is_err() {
        fail("note".into(), EventError::NoteInvalid);
    }
    let discord_url = input.discord_url.filter(|u| !u.is_empty());
    if let Some(url) = &discord_url
        && validate::discord_url(url).is_err()
    {
        fail("discordUrl".into(), EventError::DiscordUrlInvalid);
    }
    let min_cp = match input.min_cp.map(validate::cp) {
        Some(Ok(cp)) => Some(cp),
        Some(Err(_)) => {
            fail("minCp".into(), EventError::MinCpOutOfRange);
            None
        }
        None => None,
    };

    let cap = instance.map(|i| i64::from(i.parties) * i64::from(i.party_size));
    for (class, bound) in &input.class_bounds {
        if !reference
            .classes
            .iter()
            .any(|c| c.enabled && &c.id == class)
        {
            fail(format!("classBounds.{class}"), EventError::ReferenceUnknown);
            continue;
        }
        if bound.min < 0 || bound.min > bound.max {
            fail(
                format!("classBounds.{class}.min"),
                EventError::ClassBoundsInvalid,
            );
        } else if cap.is_some_and(|cap| bound.max > cap) {
            fail(
                format!("classBounds.{class}.max"),
                EventError::ClassBoundsInvalid,
            );
        }
    }

    let Some(instance) = instance else {
        return Err(errors);
    };
    if !errors.is_empty() {
        return Err(errors);
    }
    let cap = instance.parties.saturating_mul(instance.party_size);
    let class_bounds = reference
        .classes
        .iter()
        .filter(|c| c.enabled)
        .map(|c| match input.class_bounds.get(&c.id) {
            // Validées ci-dessus : 0 ≤ min ≤ max ≤ cap, donc dans un `u32`.
            Some(b) => ClassBound {
                class: c.id.clone(),
                min: b.min as u32,
                max: b.max as u32,
            },
            None => default_bound(&c.id, c.default_min, c.default_max, cap),
        })
        .collect();
    Ok(ValidEvent {
        instance: input.instance,
        region: input.region,
        faction_scope: input.faction_scope,
        slots,
        note: input.note,
        discord_url,
        visibility: input.visibility,
        min_cp,
        class_bounds,
    })
}

/// Bornes par défaut d'une classe, ramenées dans la taille de l'instance : la référence donne des
/// défauts communs à toutes les instances, une instance plus petite ne doit pas les rendre invalides.
pub fn default_bound(class: &str, default_min: u32, default_max: u32, cap: u32) -> ClassBound {
    let max = default_max.min(cap);
    ClassBound {
        class: class.to_owned(),
        min: default_min.min(max),
        max,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference::{
        Class, FactionEntry, Factions, Instance, InstanceKind, Label, Region, Role,
    };

    fn label(en: &str) -> Label {
        Label {
            en: en.into(),
            fr: None,
        }
    }

    fn class(id: &str, enabled: bool) -> Class {
        Class {
            id: id.into(),
            role: "ro".into(),
            default_min: 0,
            default_max: 10,
            label: label(id),
            enabled,
        }
    }

    fn instance(id: &str, cross_faction: bool, enabled: bool) -> Instance {
        Instance {
            id: id.into(),
            kind: InstanceKind::Sanctuary,
            parties: 2,
            party_size: 5,
            duration_min: 60,
            cross_faction,
            room_label: "R".into(),
            label: label(id),
            enabled,
        }
    }

    /// Jeu de test, valeurs fictives.
    fn reference() -> ReferenceSet {
        let mut small = instance("small", true, true);
        small.parties = 1;
        small.party_size = 4;
        ReferenceSet {
            regions: vec![
                Region {
                    id: "r1".into(),
                    label: label("R1"),
                    enabled: true,
                },
                Region {
                    id: "r_off".into(),
                    label: label("Off"),
                    enabled: false,
                },
            ],
            factions: Factions {
                elyos: FactionEntry { label: label("E") },
                asmodian: FactionEntry { label: label("A") },
            },
            servers: vec![],
            room_label_charset: "R".into(),
            room_label_max: 4,
            instances: vec![
                instance("raid", true, true),
                instance("mono", false, true),
                instance("old", true, false),
                small,
            ],
            classes: vec![class("c1", true), class("c2", true), class("c_off", false)],
            roles: vec![Role {
                id: "ro".into(),
                label: label("Ro"),
                enabled: true,
            }],
        }
    }

    fn now() -> Timestamp {
        "2026-11-06T10:00:00Z".parse().unwrap()
    }

    fn at(hours: i64) -> Timestamp {
        Timestamp::from_unix_seconds(now().unix_seconds() + hours * 3600).unwrap()
    }

    fn slot(hours: i64) -> NewSlot {
        NewSlot {
            starts_at: at(hours),
            duration_min: None,
        }
    }

    fn input() -> NewEvent {
        NewEvent {
            instance: "raid".into(),
            region: "r1".into(),
            faction_scope: FactionScope::All,
            slots: vec![slot(24), slot(48), slot(72)],
            note: String::new(),
            discord_url: None,
            visibility: Visibility::Listed,
            min_cp: None,
            class_bounds: BTreeMap::new(),
        }
    }

    fn errors(i: NewEvent) -> Vec<(String, EventError)> {
        validate_new_event(&reference(), now(), i).unwrap_err()
    }

    fn one(path: &str, e: EventError) -> Vec<(String, EventError)> {
        vec![(path.into(), e)]
    }

    #[test]
    fn nominal_takes_instance_duration_and_one_bound_per_enabled_class() {
        let v = validate_new_event(&reference(), now(), input()).unwrap();
        assert_eq!(v.slots.len(), 3);
        assert!(v.slots.iter().all(|s| s.duration_min == 60));
        assert_eq!(
            v.class_bounds,
            vec![
                ClassBound {
                    class: "c1".into(),
                    min: 0,
                    max: 10
                },
                ClassBound {
                    class: "c2".into(),
                    min: 0,
                    max: 10
                },
            ]
        );
        assert_eq!(v.visibility, Visibility::Listed);
        assert_eq!(v.discord_url, None);
        assert_eq!(v.min_cp, None);
    }

    #[test]
    fn explicit_values_are_kept() {
        let mut i = input();
        i.slots[1].duration_min = Some(90);
        i.note = "Vocal prêt".into();
        i.discord_url = Some("https://discord.gg/abc".into());
        i.min_cp = Some(1200);
        i.class_bounds
            .insert("c2".into(), NewBound { min: 2, max: 3 });
        let v = validate_new_event(&reference(), now(), i).unwrap();
        assert_eq!(v.slots[1].duration_min, 90);
        assert_eq!(v.note, "Vocal prêt");
        assert_eq!(v.discord_url.as_deref(), Some("https://discord.gg/abc"));
        assert_eq!(v.min_cp, Some(1200));
        assert_eq!(v.class_bounds[1].min, 2);
        assert_eq!(v.class_bounds[1].max, 3);
    }

    #[test]
    fn empty_discord_url_is_absent() {
        let mut i = input();
        i.discord_url = Some(String::new());
        let v = validate_new_event(&reference(), now(), i).unwrap();
        assert_eq!(v.discord_url, None);
    }

    #[test]
    fn default_bounds_fit_a_small_instance() {
        let mut i = input();
        i.instance = "small".into();
        let v = validate_new_event(&reference(), now(), i).unwrap();
        assert!(v.class_bounds.iter().all(|b| b.min == 0 && b.max == 4));
        let b = default_bound("x", 6, 9, 4);
        assert_eq!((b.min, b.max), (4, 4));
    }

    #[test]
    fn unknown_or_disabled_references() {
        for id in ["nope", "old"] {
            let mut i = input();
            i.instance = id.into();
            assert_eq!(errors(i), one("instance", EventError::ReferenceUnknown));
        }
        for id in ["nope", "r_off"] {
            let mut i = input();
            i.region = id.into();
            assert_eq!(errors(i), one("region", EventError::ReferenceUnknown));
        }
        for id in ["nope", "c_off"] {
            let mut i = input();
            i.class_bounds
                .insert(id.into(), NewBound { min: 0, max: 1 });
            assert_eq!(
                errors(i),
                one(&format!("classBounds.{id}"), EventError::ReferenceUnknown)
            );
        }
    }

    #[test]
    fn slot_in_past_or_now() {
        let mut i = input();
        i.slots[1] = NewSlot {
            starts_at: now(),
            duration_min: None,
        };
        assert_eq!(errors(i), one("slots.1.startsAt", EventError::SlotInPast));
        let mut i = input();
        i.slots[2] = slot(-1);
        assert_eq!(errors(i), one("slots.2.startsAt", EventError::SlotInPast));
    }

    #[test]
    fn slot_count_bounds() {
        let mut i = input();
        i.slots.clear();
        assert_eq!(errors(i), one("slots", EventError::SlotCountOutOfRange));
        let mut i = input();
        i.slots = (1..=14).map(slot).collect();
        assert!(validate_new_event(&reference(), now(), i).is_ok());
        let mut i = input();
        i.slots = (1..=15).map(slot).collect();
        assert_eq!(errors(i), one("slots", EventError::SlotCountOutOfRange));
    }

    #[test]
    fn duplicate_start() {
        let mut i = input();
        i.slots[2] = slot(24);
        assert_eq!(
            errors(i),
            one("slots.2.startsAt", EventError::SlotDuplicate)
        );
    }

    #[test]
    fn duration_bounds() {
        for (d, ok) in [
            (14, false),
            (15, true),
            (720, true),
            (721, false),
            (-1, false),
        ] {
            let mut i = input();
            i.slots[0].duration_min = Some(d);
            let r = validate_new_event(&reference(), now(), i);
            if ok {
                assert!(r.is_ok(), "{d}");
            } else {
                assert_eq!(
                    r.unwrap_err(),
                    one("slots.0.durationMin", EventError::SlotDurationOutOfRange),
                    "{d}"
                );
            }
        }
    }

    #[test]
    fn faction_scope_all_needs_a_cross_faction_instance() {
        let mut i = input();
        i.instance = "mono".into();
        assert_eq!(
            errors(i.clone()),
            one("factionScope", EventError::FactionScopeNotAllowed)
        );
        i.faction_scope = FactionScope::Asmodian;
        assert!(validate_new_event(&reference(), now(), i).is_ok());
    }

    #[test]
    fn note_discord_and_cp_reuse_field_rules() {
        let mut i = input();
        i.note = "voir www.example.org".into();
        assert_eq!(errors(i), one("note", EventError::NoteInvalid));
        let mut i = input();
        i.note = "a".repeat(281);
        assert_eq!(errors(i), one("note", EventError::NoteInvalid));
        let mut i = input();
        i.discord_url = Some("https://evil.example/x".into());
        assert_eq!(errors(i), one("discordUrl", EventError::DiscordUrlInvalid));
        for cp in [-1, 1_000_000] {
            let mut i = input();
            i.min_cp = Some(cp);
            assert_eq!(errors(i), one("minCp", EventError::MinCpOutOfRange));
        }
    }

    #[test]
    fn class_bounds_rules() {
        for (min, max, path) in [
            (3, 2, Some("classBounds.c1.min")),
            (-1, 2, Some("classBounds.c1.min")),
            (0, 11, Some("classBounds.c1.max")),
            (0, 10, None),
            (10, 10, None),
            (0, 0, None),
        ] {
            let mut i = input();
            i.class_bounds.insert("c1".into(), NewBound { min, max });
            let r = validate_new_event(&reference(), now(), i);
            match path {
                Some(p) => assert_eq!(
                    r.unwrap_err(),
                    one(p, EventError::ClassBoundsInvalid),
                    "{min} {max}"
                ),
                None => assert!(r.is_ok(), "{min} {max}"),
            }
        }
    }

    #[test]
    fn all_errors_are_collected_in_field_order() {
        let mut i = input();
        i.note = "https://x".into();
        i.region = "nope".into();
        assert_eq!(
            errors(i),
            vec![
                ("region".into(), EventError::ReferenceUnknown),
                ("note".into(), EventError::NoteInvalid),
            ]
        );
    }

    #[test]
    fn unknown_instance_still_checks_the_rest() {
        let mut i = input();
        i.instance = "nope".into();
        i.slots[0] = slot(-2);
        i.class_bounds
            .insert("c1".into(), NewBound { min: 0, max: 99 });
        // Sans instance, ni durée par défaut ni plafond : seules les règles indépendantes jouent.
        assert_eq!(
            errors(i),
            vec![
                ("instance".into(), EventError::ReferenceUnknown),
                ("slots.0.startsAt".into(), EventError::SlotInPast),
            ]
        );
    }
}
