//! Événement (AD-4) : règles pures sur la saisie brute de l'organisateur, à la création puis sous
//! son lien (réglages, créneaux, annulation).
//!
//! [`validate_new_event`], [`validate_added_slot`] et [`validate_settings`] prennent le
//! `ReferenceSet` une seule fois (AD-6) et, quand il le faut, `now` de l'horloge injectée (AD-11).
//! Toutes les erreurs de champ sont rendues ensemble, dans l'ordre des champs du formulaire : la
//! couche `http` prend la première pour `code` et les autres pour `fields`. [`event_may`] porte la
//! mutabilité d'AD-4 : quel geste l'état de l'événement permet. [`validate_lock`] et [`lock_plan`]
//! portent le verrouillage d'AD-4 : entrées retenues, appariement aux groupes existants.

use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::Timestamp;
use crate::reference::{Faction, ReferenceSet};
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

    /// Valeur lue dans la colonne `faction_scope`.
    pub fn parse(s: &str) -> Option<Self> {
        [Self::All, Self::Elyos, Self::Asmodian]
            .into_iter()
            .find(|v| v.as_str() == s)
    }

    /// Vrai si un serveur de cette faction entre dans la portée.
    pub fn admits(self, faction: Faction) -> bool {
        match self {
            Self::All => true,
            Self::Elyos => faction == Faction::Elyos,
            Self::Asmodian => faction == Faction::Asmodian,
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

    /// Valeur lue dans la colonne `visibility`.
    pub fn parse(s: &str) -> Option<Self> {
        [Self::Listed, Self::Unlisted]
            .into_iter()
            .find(|v| v.as_str() == s)
    }
}

/// État de l'événement (AD-4) : `open → locked → open`, `open|locked → cancelled`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EventStatus {
    Open,
    Locked,
    Cancelled,
}

impl EventStatus {
    /// Valeur de la colonne `status`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Locked => "locked",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        [Self::Open, Self::Locked, Self::Cancelled]
            .into_iter()
            .find(|v| v.as_str() == s)
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
    TeamRolesInvalid,
    /// `instance`, `region` ou `factionScope` changé alors qu'une candidature existe : un conflit
    /// d'état (409), vérifié avant toute autre règle de champ.
    FieldImmutable,
}

/// Rôles demandés par Équipe : une entrée par Équipe (la première est l'Équipe 1),
/// `roleId → nombre`, sans zéro. Un signal pour l'organisateur, jamais un refus de placement.
pub type TeamRoles = Vec<BTreeMap<String, u32>>;

/// Valide les rôles demandés pour une instance à `parties` Équipes de `party_size` : au plus une
/// entrée par Équipe, des rôles connus de la référence, et au plus `party_size` joueurs demandés
/// par Équipe. Les zéros sont retirés.
pub fn validate_team_roles(
    reference: &ReferenceSet,
    parties: u32,
    party_size: u32,
    input: Vec<BTreeMap<String, i64>>,
) -> Result<TeamRoles, FieldErrors> {
    let mut errors: FieldErrors = Vec::new();
    if input.len() > parties as usize {
        errors.push(("teamRoles".into(), EventError::TeamRolesInvalid));
    }
    let mut out = Vec::with_capacity(input.len());
    for (n, team) in input.into_iter().enumerate() {
        let mut kept = BTreeMap::new();
        let mut total: i64 = 0;
        for (role, count) in team {
            let known = reference.roles.iter().any(|r| r.id == role);
            if !known || !(0..=i64::from(party_size)).contains(&count) {
                errors.push((
                    format!("teamRoles.{n}.{role}"),
                    EventError::TeamRolesInvalid,
                ));
                continue;
            }
            total += count;
            if count > 0 {
                kept.insert(role, count as u32);
            }
        }
        if total > i64::from(party_size) {
            errors.push((format!("teamRoles.{n}"), EventError::TeamRolesInvalid));
        }
        out.push(kept);
    }
    if errors.is_empty() {
        Ok(out)
    } else {
        Err(errors)
    }
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

    if !slot_count_allowed(input.slots.len()) {
        fail("slots".into(), EventError::SlotCountOutOfRange);
    }
    let mut taken = HashSet::new();
    let mut slots = Vec::with_capacity(input.slots.len());
    for (n, slot) in input.slots.iter().enumerate() {
        // Instance inconnue, déjà signalée : sans durée par défaut, le créneau n'est pas retenu.
        let checked = check_slot(
            slot,
            now,
            &mut taken,
            instance.map(|i| i.duration_min),
            |field, e| fail(format!("slots.{n}.{}", field.as_str()), e),
        );
        slots.extend(checked);
    }

    let (discord_url, min_cp) =
        check_details(&input.note, input.discord_url, input.min_cp, &mut fail);

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

/// Règles des champs libres, partagées par la création et les réglages (`domain::validate`) : note,
/// lien Discord (vide = absent) et CP minimum. Rend le lien et le CP prêts à écrire ; un CP refusé
/// est rendu absent, l'erreur étant signalée.
fn check_details(
    note: &str,
    discord_url: Option<String>,
    min_cp: Option<i64>,
    fail: &mut impl FnMut(String, EventError),
) -> (Option<String>, Option<u32>) {
    if validate::note(note).is_err() {
        fail("note".into(), EventError::NoteInvalid);
    }
    let discord_url = discord_url.filter(|u| !u.is_empty());
    if let Some(url) = &discord_url
        && validate::discord_url(url).is_err()
    {
        fail("discordUrl".into(), EventError::DiscordUrlInvalid);
    }
    let min_cp = match min_cp.map(validate::cp) {
        Some(Ok(cp)) => Some(cp),
        Some(Err(_)) => {
            fail("minCp".into(), EventError::MinCpOutOfRange);
            None
        }
        None => None,
    };
    (discord_url, min_cp)
}

/// Nombre de créneaux permis pour un événement, passés compris : vérifié à la création, à l'ajout
/// (le créneau en plus compris) et à la suppression (le créneau en moins décompté).
pub fn slot_count_allowed(n: usize) -> bool {
    (SLOTS_MIN..=SLOTS_MAX).contains(&n)
}

/// Champ d'un créneau saisi, pour le chemin de son erreur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SlotField {
    StartsAt,
    DurationMin,
}

impl SlotField {
    const fn as_str(self) -> &'static str {
        match self {
            Self::StartsAt => "startsAt",
            Self::DurationMin => "durationMin",
        }
    }
}

/// Règles d'un créneau saisi, partagées par la création et l'ajout : début strictement futur et
/// distinct des débuts de `taken` (où il entre s'il est futur), durée saisie ou, à défaut, celle de
/// l'instance, entre [`DURATION_MIN_MINUTES`] et [`DURATION_MAX_MINUTES`]. Rend le créneau s'il passe
/// toutes les règles ; sans durée du tout (`default_duration` absente et rien de saisi), rien n'est
/// signalé : à l'appelant de le faire.
fn check_slot(
    slot: &NewSlot,
    now: Timestamp,
    taken: &mut HashSet<Timestamp>,
    default_duration: Option<u32>,
    mut fail: impl FnMut(SlotField, EventError),
) -> Option<ValidSlot> {
    let mut start_ok = false;
    if slot.starts_at <= now {
        fail(SlotField::StartsAt, EventError::SlotInPast);
    } else if !taken.insert(slot.starts_at) {
        fail(SlotField::StartsAt, EventError::SlotDuplicate);
    } else {
        start_ok = true;
    }
    match slot.duration_min.or(default_duration.map(i64::from)) {
        Some(d) if (DURATION_MIN_MINUTES..=DURATION_MAX_MINUTES).contains(&d) => {
            start_ok.then_some(ValidSlot {
                starts_at: slot.starts_at,
                // Borné ci-dessus : tient dans un `u32`.
                duration_min: d as u32,
            })
        }
        // Durée de l'instance hors bornes : l'organisateur la corrige sur le créneau.
        Some(_) => {
            fail(SlotField::DurationMin, EventError::SlotDurationOutOfRange);
            None
        }
        None => None,
    }
}

/// Refus d'un ajout de créneau.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NewSlotError {
    /// L'événement a déjà [`SLOTS_MAX`] créneaux, passés compris : rien dans la saisie n'y peut rien.
    CountOutOfRange,
    /// Erreurs de champ du créneau saisi, chemins `startsAt` et `durationMin`.
    Fields(FieldErrors),
}

/// Ajout d'un créneau à un événement (AD-4) : les règles de la création ([`check_slot`],
/// [`slot_count_allowed`]) contre les débuts des créneaux existants, passés compris.
/// `instance_duration` est la durée de l'instance de l'événement, lue sans égard à `enabled` (la
/// valeur n'est pas posée par l'ajout, AD-6) ; absente si la référence ne la connaît plus, la durée
/// doit alors être saisie.
pub fn validate_added_slot(
    now: Timestamp,
    existing: &[Timestamp],
    instance_duration: Option<u32>,
    input: NewSlot,
) -> Result<ValidSlot, NewSlotError> {
    if !slot_count_allowed(existing.len() + 1) {
        return Err(NewSlotError::CountOutOfRange);
    }
    let mut errors: FieldErrors = Vec::new();
    let mut taken: HashSet<Timestamp> = existing.iter().copied().collect();
    let slot = check_slot(&input, now, &mut taken, instance_duration, |field, e| {
        errors.push((field.as_str().into(), e))
    });
    if input.duration_min.is_none() && instance_duration.is_none() {
        errors.push(("durationMin".into(), EventError::SlotDurationOutOfRange));
    }
    match slot {
        Some(slot) if errors.is_empty() => Ok(slot),
        _ => Err(NewSlotError::Fields(errors)),
    }
}

/// Réglages saisis par l'organisateur : le corps remplace tout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewSettings {
    pub instance: String,
    pub region: String,
    pub faction_scope: FactionScope,
    pub note: String,
    /// Vide ou absent : le lien est effacé.
    pub discord_url: Option<String>,
    pub visibility: Visibility,
    /// Absent : le CP minimum est effacé.
    pub min_cp: Option<i64>,
    /// Bornes des classes déjà bornées par l'événement ; une classe omise garde les siennes.
    pub class_bounds: BTreeMap<String, NewBound>,
}

/// Ce que les réglages doivent savoir de l'événement, relu dans la transaction de la commande.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentSettings {
    pub instance: String,
    pub region: String,
    pub faction_scope: FactionScope,
    /// Vrai dès qu'une ligne `candidatures` existe, quel que soit son statut.
    pub has_candidatures: bool,
    /// Lignes `class_bounds` de l'événement.
    pub class_bounds: Vec<ClassBound>,
}

/// Réglages prêts à écrire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidSettings {
    pub instance: String,
    pub region: String,
    pub faction_scope: FactionScope,
    pub note: String,
    pub discord_url: Option<String>,
    pub visibility: Visibility,
    pub min_cp: Option<u32>,
    /// Une borne par ligne existante, dans l'ordre de [`CurrentSettings::class_bounds`] : jamais une
    /// ligne de plus ni de moins.
    pub class_bounds: Vec<ClassBound>,
}

/// Réglages de l'organisateur (AD-4 *Mutabilité*, AD-6), dans cet ordre :
///
/// 1. dès qu'une candidature existe, `instance`, `region` ou `factionScope` différent de la valeur
///    actuelle est [`EventError::FieldImmutable`] sur son chemin, seul rendu (un 409 passe avant les
///    400) ; une valeur renvoyée inchangée passe toujours ;
/// 2. les règles de la création, où « existe et `enabled` » ne porte que sur une valeur changée ;
///    la portée n'est confrontée à l'instance que si l'une des deux change ;
/// 3. les bornes : une classe sans ligne est inconnue, et chaque ligne (saisie ou gardée) respecte
///    `0 ≤ min ≤ max ≤ parties × party_size` de l'instance résultante.
pub fn validate_settings(
    reference: &ReferenceSet,
    current: &CurrentSettings,
    input: NewSettings,
) -> Result<ValidSettings, FieldErrors> {
    let instance_changed = input.instance != current.instance;
    let region_changed = input.region != current.region;
    let scope_changed = input.faction_scope != current.faction_scope;
    if current.has_candidatures {
        let frozen: FieldErrors = [
            ("instance", instance_changed),
            ("region", region_changed),
            ("factionScope", scope_changed),
        ]
        .into_iter()
        .filter(|(_, changed)| *changed)
        .map(|(path, _)| (path.to_owned(), EventError::FieldImmutable))
        .collect();
        if !frozen.is_empty() {
            return Err(frozen);
        }
    }

    let mut errors: FieldErrors = Vec::new();
    let mut fail = |path: String, e: EventError| errors.push((path, e));

    let instance = reference
        .instances
        .iter()
        .find(|i| i.id == input.instance && (i.enabled || !instance_changed));
    if instance.is_none() && instance_changed {
        fail("instance".into(), EventError::ReferenceUnknown);
    }
    if region_changed
        && !reference
            .regions
            .iter()
            .any(|r| r.enabled && r.id == input.region)
    {
        fail("region".into(), EventError::ReferenceUnknown);
    }
    if (instance_changed || scope_changed)
        && let Some(i) = instance
        && !i.cross_faction
        && input.faction_scope == FactionScope::All
    {
        fail("factionScope".into(), EventError::FactionScopeNotAllowed);
    }

    let (discord_url, min_cp) =
        check_details(&input.note, input.discord_url, input.min_cp, &mut fail);

    for class in input.class_bounds.keys() {
        if !current.class_bounds.iter().any(|b| &b.class == class) {
            fail(format!("classBounds.{class}"), EventError::ReferenceUnknown);
        }
    }
    // Instance absente de la référence (inchangée, donc tolérée) : pas de plafond connu, seulement
    // celui de la colonne.
    let cap = instance.map_or(i64::from(u32::MAX), |i| {
        i64::from(i.parties.saturating_mul(i.party_size))
    });
    let mut class_bounds = Vec::with_capacity(current.class_bounds.len());
    for row in &current.class_bounds {
        let (min, max) = match input.class_bounds.get(&row.class) {
            Some(b) => (b.min, b.max),
            None => (i64::from(row.min), i64::from(row.max)),
        };
        if min < 0 || min > max {
            fail(
                format!("classBounds.{}.min", row.class),
                EventError::ClassBoundsInvalid,
            );
        } else if max > cap {
            fail(
                format!("classBounds.{}.max", row.class),
                EventError::ClassBoundsInvalid,
            );
        } else {
            class_bounds.push(ClassBound {
                class: row.class.clone(),
                // Validées ci-dessus : 0 ≤ min ≤ max ≤ cap ≤ `u32::MAX`.
                min: min as u32,
                max: max as u32,
            });
        }
    }

    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(ValidSettings {
        instance: input.instance,
        region: input.region,
        faction_scope: input.faction_scope,
        note: input.note,
        discord_url,
        visibility: input.visibility,
        min_cp,
        class_bounds,
    })
}

/// Geste de l'organisateur sur son événement (AD-4 *Mutabilité*).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventAction {
    /// Réglages : note, lien Discord, visibilité, CP minimum, bornes, et les champs figés dès la
    /// première candidature.
    Settings,
    AddSlot,
    RemoveSlot,
    Cancel,
    /// `open → locked` : les créneaux retenus deviennent des groupes.
    Lock,
    /// `locked → open`, tant qu'un créneau est à venir ([`has_upcoming_slot`]).
    Unlock,
    /// Placement des groupes retenus (`apply_placement`), en `locked` seulement ; un créneau
    /// commencé ne l'empêche pas.
    Place,
    /// Nouveau nom de salle d'un groupe retenu (`regenerate_room_name`), même règle que
    /// [`EventAction::Place`] : en `locked` seulement, même si le créneau a commencé.
    RegenerateRoom,
}

/// Refus d'un geste de l'organisateur, dans l'ordre où les règles sont vérifiées.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventRefusal {
    /// L'événement est annulé : plus aucune écriture, pas même une seconde annulation.
    Closed,
    /// L'événement est verrouillé : les créneaux ne bougent plus, et il ne se reverrouille pas.
    Locked,
    /// L'événement est en préparation : rien à déverrouiller.
    NotLocked,
}

/// Mutabilité d'AD-4, à vérifier après relecture de l'agrégat : un événement `cancelled` refuse
/// tout ; les réglages et l'annulation valent en `open` comme en `locked` ; l'ajout et la
/// suppression de créneau et le verrouillage en `open` seulement ; le déverrouillage, le placement
/// et la régénération du nom de salle en `locked` seulement.
pub fn event_may(status: EventStatus, action: EventAction) -> Result<(), EventRefusal> {
    match (status, action) {
        (EventStatus::Cancelled, _) => Err(EventRefusal::Closed),
        (
            EventStatus::Locked,
            EventAction::AddSlot | EventAction::RemoveSlot | EventAction::Lock,
        ) => Err(EventRefusal::Locked),
        (
            EventStatus::Open,
            EventAction::Unlock | EventAction::Place | EventAction::RegenerateRoom,
        ) => Err(EventRefusal::NotLocked),
        (EventStatus::Open | EventStatus::Locked, _) => Ok(()),
    }
}

/// Nombre de groupes d'un verrouillage, doublons compris `[ASSUMPTION]` : autant que de créneaux
/// permis.
pub const GROUPS_MIN: usize = 1;
pub const GROUPS_MAX: usize = SLOTS_MAX;

/// Vrai si un créneau commence après `now` : le déverrouillage n'est permis qu'à cette condition.
pub fn has_upcoming_slot(now: Timestamp, starts: &[Timestamp]) -> bool {
    starts.iter().any(|&s| s > now)
}

/// Refus d'une liste de créneaux retenus, dans l'ordre où les règles sont vérifiées.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LockError {
    /// Liste vide ou de plus de [`GROUPS_MAX`] entrées.
    GroupCountOutOfRange,
    /// Un id ne désigne aucun créneau de l'événement.
    SlotNotFound,
    /// Rangs (dans la liste) des entrées dont le créneau a commencé : chemins `slotIds.N`.
    SlotInPast(Vec<usize>),
}

/// Une entrée retenue : le créneau et son rang d'occurrence dans la liste (1 pour la première fois
/// qu'il y figure, 2 pour la deuxième…). C'est la clé d'appariement d'un groupe (AD-4).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LockEntry {
    pub slot_id: String,
    pub occurrence: u32,
}

/// Vérifie la liste des créneaux retenus contre les créneaux de l'événement (`id`, début) : nombre
/// d'entrées, appartenance, puis créneaux à venir. Rend les entrées dans l'ordre de la liste, chacune
/// avec son rang d'occurrence. Les disponibilités ne comptent pas : un groupe incomplet est permis.
pub fn validate_lock(
    now: Timestamp,
    slots: &[(&str, Timestamp)],
    requested: &[String],
) -> Result<Vec<LockEntry>, LockError> {
    if !(GROUPS_MIN..=GROUPS_MAX).contains(&requested.len()) {
        return Err(LockError::GroupCountOutOfRange);
    }
    let mut starts = Vec::with_capacity(requested.len());
    for id in requested {
        let start = slots
            .iter()
            .find(|(s, _)| s == id)
            .map(|&(_, start)| start)
            .ok_or(LockError::SlotNotFound)?;
        starts.push(start);
    }
    let past: Vec<usize> = starts
        .iter()
        .enumerate()
        .filter(|&(_, &start)| start <= now)
        .map(|(i, _)| i)
        .collect();
    if !past.is_empty() {
        return Err(LockError::SlotInPast(past));
    }
    let mut seen: BTreeMap<&str, u32> = BTreeMap::new();
    Ok(requested
        .iter()
        .map(|id| {
            let n = seen.entry(id.as_str()).or_insert(0);
            *n += 1;
            LockEntry {
                slot_id: id.clone(),
                occurrence: *n,
            }
        })
        .collect())
}

/// Groupe déjà en base, vu par l'appariement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExistingGroup {
    pub id: String,
    pub slot_id: String,
    pub occurrence: u32,
    /// Son créneau est à venir. Un créneau commencé ne peut plus être retenu : son groupe n'est
    /// ni réapparié ni mis en sommeil, il garde son `retained`.
    pub upcoming: bool,
}

/// Issue de l'appariement d'un verrouillage.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LockPlan {
    /// Groupes réappariés, gardés tels quels (disposition, nom, placements) et retenus.
    pub reuse: Vec<String>,
    /// Entrées sans groupe : un groupe neuf chacune, à nommer.
    pub create: Vec<LockEntry>,
    /// Groupes non réappariés des créneaux à venir : mis en sommeil, jamais supprimés (décision de
    /// Vincent, 2026-11-07).
    pub sleep: Vec<String>,
}

/// Apparie chaque entrée au groupe existant de même `(slot_id, occurrence)` (AD-4). Seuls les
/// créneaux à venir sont replanifiés : un groupe d'un créneau commencé n'est dans aucune liste. Les
/// listes gardent l'ordre des entrées, puis celui de `existing`.
pub fn lock_plan(entries: &[LockEntry], existing: &[ExistingGroup]) -> LockPlan {
    let mut plan = LockPlan::default();
    for e in entries {
        match existing
            .iter()
            .find(|g| g.slot_id == e.slot_id && g.occurrence == e.occurrence)
        {
            Some(g) => plan.reuse.push(g.id.clone()),
            None => plan.create.push(e.clone()),
        }
    }
    plan.sleep = existing
        .iter()
        .filter(|g| g.upcoming && !plan.reuse.contains(&g.id))
        .map(|g| g.id.clone())
        .collect();
    plan
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
            icon: None,
            color: None,
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
    fn column_values_round_trip() {
        for v in [
            FactionScope::All,
            FactionScope::Elyos,
            FactionScope::Asmodian,
        ] {
            assert_eq!(FactionScope::parse(v.as_str()), Some(v));
        }
        for v in [Visibility::Listed, Visibility::Unlisted] {
            assert_eq!(Visibility::parse(v.as_str()), Some(v));
        }
        for v in [
            EventStatus::Open,
            EventStatus::Locked,
            EventStatus::Cancelled,
        ] {
            assert_eq!(EventStatus::parse(v.as_str()), Some(v));
        }
        assert_eq!(FactionScope::parse("ALL"), None);
        assert_eq!(EventStatus::parse(""), None);
    }

    #[test]
    fn scope_admits_factions() {
        assert!(FactionScope::All.admits(Faction::Elyos));
        assert!(FactionScope::All.admits(Faction::Asmodian));
        assert!(FactionScope::Elyos.admits(Faction::Elyos));
        assert!(!FactionScope::Elyos.admits(Faction::Asmodian));
        assert!(!FactionScope::Asmodian.admits(Faction::Elyos));
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
    fn slot_count_bounds_are_shared() {
        assert!(!slot_count_allowed(0));
        assert!(slot_count_allowed(1));
        assert!(slot_count_allowed(SLOTS_MAX));
        assert!(!slot_count_allowed(SLOTS_MAX + 1));
    }

    fn added(
        existing: &[Timestamp],
        duration: Option<u32>,
        slot: NewSlot,
    ) -> Result<ValidSlot, NewSlotError> {
        validate_added_slot(now(), existing, duration, slot)
    }

    fn added_fields(e: Result<ValidSlot, NewSlotError>) -> Vec<(String, EventError)> {
        match e {
            Err(NewSlotError::Fields(f)) => f,
            other => panic!("expected field errors, got {other:?}"),
        }
    }

    #[test]
    fn an_added_slot_takes_the_instance_duration_by_default() {
        let existing = [at(-48), at(24)];
        assert_eq!(
            added(&existing, Some(60), slot(48)),
            Ok(ValidSlot {
                starts_at: at(48),
                duration_min: 60
            })
        );
        let mut s = slot(48);
        s.duration_min = Some(90);
        assert_eq!(added(&existing, Some(60), s).unwrap().duration_min, 90);
        // Instance inconnue de la référence : la durée saisie suffit.
        let mut s = slot(48);
        s.duration_min = Some(45);
        assert_eq!(added(&existing, None, s).unwrap().duration_min, 45);
    }

    #[test]
    fn an_added_slot_follows_the_creation_rules() {
        let existing = [at(-48), at(24)];
        assert_eq!(
            added_fields(added(&existing, Some(60), slot(-1))),
            one("startsAt", EventError::SlotInPast)
        );
        let now_slot = NewSlot {
            starts_at: now(),
            duration_min: None,
        };
        assert_eq!(
            added_fields(added(&existing, Some(60), now_slot)),
            one("startsAt", EventError::SlotInPast)
        );
        assert_eq!(
            added_fields(added(&existing, Some(60), slot(24))),
            one("startsAt", EventError::SlotDuplicate)
        );
        for (d, ok) in [(14, false), (15, true), (720, true), (721, false)] {
            let mut s = slot(48);
            s.duration_min = Some(d);
            let r = added(&existing, Some(60), s);
            if ok {
                assert!(r.is_ok(), "{d}");
            } else {
                assert_eq!(
                    added_fields(r),
                    one("durationMin", EventError::SlotDurationOutOfRange),
                    "{d}"
                );
            }
        }
        // Durée de l'instance hors bornes, ou instance inconnue sans durée saisie.
        assert_eq!(
            added_fields(added(&existing, Some(5), slot(48))),
            one("durationMin", EventError::SlotDurationOutOfRange)
        );
        assert_eq!(
            added_fields(added(&existing, None, slot(-1))),
            vec![
                ("startsAt".into(), EventError::SlotInPast),
                ("durationMin".into(), EventError::SlotDurationOutOfRange),
            ]
        );
    }

    #[test]
    fn at_most_fourteen_slots_past_ones_included() {
        let thirteen: Vec<_> = (-6..7).map(|h| at(h * 10 + 1)).collect();
        assert_eq!(thirteen.len(), 13);
        assert!(added(&thirteen, Some(60), slot(200)).is_ok());
        let fourteen: Vec<_> = (-7..7).map(|h| at(h * 10 + 1)).collect();
        assert_eq!(
            added(&fourteen, Some(60), slot(200)),
            Err(NewSlotError::CountOutOfRange)
        );
    }

    fn bound(class: &str, min: u32, max: u32) -> ClassBound {
        ClassBound {
            class: class.into(),
            min,
            max,
        }
    }

    fn current(has_candidatures: bool) -> CurrentSettings {
        CurrentSettings {
            instance: "raid".into(),
            region: "r1".into(),
            faction_scope: FactionScope::All,
            has_candidatures,
            class_bounds: vec![bound("c1", 0, 10), bound("c2", 1, 4)],
        }
    }

    fn settings() -> NewSettings {
        NewSettings {
            instance: "raid".into(),
            region: "r1".into(),
            faction_scope: FactionScope::All,
            note: String::new(),
            discord_url: None,
            visibility: Visibility::Listed,
            min_cp: None,
            class_bounds: BTreeMap::new(),
        }
    }

    fn settings_errors(c: &CurrentSettings, i: NewSettings) -> Vec<(String, EventError)> {
        validate_settings(&reference(), c, i).unwrap_err()
    }

    #[test]
    fn settings_replace_every_value() {
        let mut i = settings();
        i.note = "Vocal prêt".into();
        i.discord_url = Some("https://discord.gg/abc".into());
        i.visibility = Visibility::Unlisted;
        i.min_cp = Some(1200);
        i.class_bounds
            .insert("c2".into(), NewBound { min: 2, max: 3 });
        let v = validate_settings(&reference(), &current(true), i).unwrap();
        assert_eq!(v.note, "Vocal prêt");
        assert_eq!(v.discord_url.as_deref(), Some("https://discord.gg/abc"));
        assert_eq!(v.visibility, Visibility::Unlisted);
        assert_eq!(v.min_cp, Some(1200));
        // Une borne par ligne existante : la classe omise garde les siennes.
        assert_eq!(v.class_bounds, vec![bound("c1", 0, 10), bound("c2", 2, 3)]);
        // Vide ou absent : effacé.
        let mut i = settings();
        i.discord_url = Some(String::new());
        let v = validate_settings(&reference(), &current(true), i).unwrap();
        assert_eq!((v.discord_url, v.min_cp), (None, None));
    }

    #[test]
    fn frozen_fields_once_a_candidature_exists_and_before_any_400() {
        for (path, patch) in [
            (
                "instance",
                (|i: &mut NewSettings| i.instance = "small".into()) as fn(&mut _),
            ),
            ("region", |i| i.region = "r_off".into()),
            ("factionScope", |i| i.faction_scope = FactionScope::Elyos),
        ] {
            let mut i = settings();
            patch(&mut i);
            // Une erreur 400 ailleurs ne passe pas devant le refus.
            i.note = "https://x".into();
            assert_eq!(
                settings_errors(&current(true), i),
                one(path, EventError::FieldImmutable),
                "{path}"
            );
        }
        let mut i = settings();
        i.instance = "small".into();
        i.region = "nope".into();
        assert_eq!(
            settings_errors(&current(true), i),
            vec![
                ("instance".into(), EventError::FieldImmutable),
                ("region".into(), EventError::FieldImmutable),
            ]
        );
        // Renvoyées inchangées : elles passent.
        assert!(validate_settings(&reference(), &current(true), settings()).is_ok());
    }

    #[test]
    fn without_candidature_the_raid_fields_change_under_the_creation_rules() {
        let mut i = settings();
        i.instance = "small".into();
        i.faction_scope = FactionScope::Asmodian;
        i.class_bounds
            .insert("c1".into(), NewBound { min: 0, max: 4 });
        i.class_bounds
            .insert("c2".into(), NewBound { min: 1, max: 4 });
        let v = validate_settings(&reference(), &current(false), i).unwrap();
        assert_eq!(v.instance, "small");
        assert_eq!(v.faction_scope, FactionScope::Asmodian);
        // Valeurs changées : existe et `enabled`.
        let mut i = settings();
        i.instance = "old".into();
        assert_eq!(
            settings_errors(&current(false), i),
            one("instance", EventError::ReferenceUnknown)
        );
        let mut i = settings();
        i.region = "r_off".into();
        assert_eq!(
            settings_errors(&current(false), i),
            one("region", EventError::ReferenceUnknown)
        );
        // Portée confrontée à l'instance résultante.
        let mut i = settings();
        i.instance = "mono".into();
        assert_eq!(
            settings_errors(&current(false), i),
            one("factionScope", EventError::FactionScopeNotAllowed)
        );
    }

    #[test]
    fn unchanged_values_are_not_checked_against_enabled() {
        let c = CurrentSettings {
            instance: "old".into(),
            region: "r_off".into(),
            ..current(true)
        };
        let mut i = settings();
        i.instance = "old".into();
        i.region = "r_off".into();
        assert!(validate_settings(&reference(), &c, i).is_ok());
        // Instance mono-faction et portée « all » déjà là : ni l'une ni l'autre ne change.
        let c = CurrentSettings {
            instance: "mono".into(),
            ..current(true)
        };
        let mut i = settings();
        i.instance = "mono".into();
        assert!(validate_settings(&reference(), &c, i).is_ok());
        // Instance retirée de la référence, inchangée : tolérée, sans plafond.
        let c = CurrentSettings {
            instance: "gone".into(),
            ..current(true)
        };
        let mut i = settings();
        i.instance = "gone".into();
        i.class_bounds
            .insert("c1".into(), NewBound { min: 0, max: 40 });
        assert!(validate_settings(&reference(), &c, i).is_ok());
    }

    #[test]
    fn settings_reuse_the_field_rules() {
        for (patch, path, e) in [
            (
                (|i: &mut NewSettings| i.note = "a".repeat(281)) as fn(&mut _),
                "note",
                EventError::NoteInvalid,
            ),
            (
                |i| i.discord_url = Some("https://evil.example/x".into()),
                "discordUrl",
                EventError::DiscordUrlInvalid,
            ),
            (
                |i| i.min_cp = Some(-1),
                "minCp",
                EventError::MinCpOutOfRange,
            ),
            (
                |i| i.min_cp = Some(1_000_000),
                "minCp",
                EventError::MinCpOutOfRange,
            ),
        ] {
            let mut i = settings();
            patch(&mut i);
            assert_eq!(settings_errors(&current(true), i), one(path, e), "{path}");
        }
    }

    #[test]
    fn settings_only_update_existing_bounds_within_the_resulting_instance() {
        for (class, min, max, path, e) in [
            (
                "c_new",
                0,
                1,
                "classBounds.c_new",
                EventError::ReferenceUnknown,
            ),
            (
                "c1",
                3,
                2,
                "classBounds.c1.min",
                EventError::ClassBoundsInvalid,
            ),
            (
                "c1",
                -1,
                2,
                "classBounds.c1.min",
                EventError::ClassBoundsInvalid,
            ),
            (
                "c1",
                0,
                11,
                "classBounds.c1.max",
                EventError::ClassBoundsInvalid,
            ),
        ] {
            let mut i = settings();
            i.class_bounds.insert(class.into(), NewBound { min, max });
            assert_eq!(
                settings_errors(&current(true), i),
                one(path, e),
                "{class} {min} {max}"
            );
        }
        // Une classe active de la référence sans ligne dans l'événement reste inconnue.
        let c = CurrentSettings {
            class_bounds: vec![bound("c1", 0, 10)],
            ..current(true)
        };
        let mut i = settings();
        i.class_bounds
            .insert("c2".into(), NewBound { min: 0, max: 1 });
        assert_eq!(
            settings_errors(&c, i),
            one("classBounds.c2", EventError::ReferenceUnknown)
        );
        // Instance plus petite : une ligne gardée telle quelle dépasse le nouveau plafond.
        let mut i = settings();
        i.instance = "small".into();
        i.class_bounds
            .insert("c2".into(), NewBound { min: 1, max: 4 });
        assert_eq!(
            settings_errors(&current(false), i),
            one("classBounds.c1.max", EventError::ClassBoundsInvalid)
        );
    }

    #[test]
    fn mutability_is_exhaustive_and_in_order() {
        use EventAction::*;
        use EventRefusal as R;
        use EventStatus::*;
        // AD-4 *Mutabilité* et *États*, ligne par ligne : réglages toujours modifiables, créneaux
        // en `open` seulement, `open|locked → cancelled`, et un événement annulé refuse tout.
        // `open → locked` par le verrouillage, `locked → open` par le déverrouillage, et le placement
        // et la régénération du nom de salle en `locked` seulement.
        let table = [
            (Open, Settings, Ok(())),
            (Open, AddSlot, Ok(())),
            (Open, RemoveSlot, Ok(())),
            (Open, Cancel, Ok(())),
            (Open, Lock, Ok(())),
            (Open, Unlock, Err(R::NotLocked)),
            (Open, Place, Err(R::NotLocked)),
            (Open, RegenerateRoom, Err(R::NotLocked)),
            (Locked, Settings, Ok(())),
            (Locked, AddSlot, Err(R::Locked)),
            (Locked, RemoveSlot, Err(R::Locked)),
            (Locked, Cancel, Ok(())),
            (Locked, Lock, Err(R::Locked)),
            (Locked, Unlock, Ok(())),
            (Locked, Place, Ok(())),
            (Locked, RegenerateRoom, Ok(())),
            (Cancelled, Settings, Err(R::Closed)),
            (Cancelled, AddSlot, Err(R::Closed)),
            (Cancelled, RemoveSlot, Err(R::Closed)),
            (Cancelled, Cancel, Err(R::Closed)),
            (Cancelled, Lock, Err(R::Closed)),
            (Cancelled, Unlock, Err(R::Closed)),
            (Cancelled, Place, Err(R::Closed)),
            (Cancelled, RegenerateRoom, Err(R::Closed)),
        ];
        for (status, action, expected) in table {
            assert_eq!(event_may(status, action), expected, "{status:?} {action:?}");
        }
        // Chaque couple (état, geste) figure une fois dans la table.
        let pairs: HashSet<_> = table.iter().map(|(s, a, _)| (*s, *a)).collect();
        assert_eq!(pairs.len(), 3 * 8);
    }

    fn ids(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    fn entry(slot: &str, occurrence: u32) -> LockEntry {
        LockEntry {
            slot_id: slot.into(),
            occurrence,
        }
    }

    #[test]
    fn a_lock_numbers_each_repeated_slot() {
        let slots = [("a", at(2)), ("b", at(5))];
        assert_eq!(
            validate_lock(now(), &slots, &ids(&["a", "a", "b"])),
            Ok(vec![entry("a", 1), entry("a", 2), entry("b", 1)])
        );
        assert_eq!(
            validate_lock(now(), &slots, &ids(&["b", "a", "b", "b"])),
            Ok(vec![
                entry("b", 1),
                entry("a", 1),
                entry("b", 2),
                entry("b", 3)
            ])
        );
    }

    #[test]
    fn a_lock_checks_count_then_membership_then_time() {
        let slots = [("a", at(2)), ("past", at(-1)), ("starting", at(0))];
        assert_eq!(
            validate_lock(now(), &slots, &[]),
            Err(LockError::GroupCountOutOfRange)
        );
        assert_eq!(
            validate_lock(now(), &slots, &ids(&["a"; GROUPS_MAX + 1])),
            Err(LockError::GroupCountOutOfRange)
        );
        assert!(validate_lock(now(), &slots, &ids(&["a"; GROUPS_MAX])).is_ok());
        // L'appartenance passe avant le temps, et le nombre avant tout.
        assert_eq!(
            validate_lock(now(), &slots, &ids(&["past", "x"])),
            Err(LockError::SlotNotFound)
        );
        assert_eq!(
            validate_lock(now(), &slots, &ids(&["x"; GROUPS_MAX + 1])),
            Err(LockError::GroupCountOutOfRange)
        );
        // Un créneau qui commence maintenant a commencé.
        assert_eq!(
            validate_lock(now(), &slots, &ids(&["a", "past", "a", "starting"])),
            Err(LockError::SlotInPast(vec![1, 3]))
        );
    }

    #[test]
    fn a_lock_pairs_by_slot_and_occurrence_and_never_drops_a_group() {
        let existing = |list: &[(&str, &str, u32)]| -> Vec<ExistingGroup> {
            list.iter()
                .map(|&(id, slot, occurrence)| ExistingGroup {
                    id: id.into(),
                    slot_id: slot.into(),
                    occurrence,
                    upcoming: slot != "past",
                })
                .collect()
        };
        // Premier verrouillage : tout est neuf.
        let entries = vec![entry("a", 1), entry("a", 2), entry("b", 1)];
        assert_eq!(
            lock_plan(&entries, &[]),
            LockPlan {
                reuse: vec![],
                create: entries.clone(),
                sleep: vec![],
            }
        );
        // Moins : A#2 en sommeil.
        let groups = existing(&[("g1", "a", 1), ("g2", "a", 2)]);
        assert_eq!(
            lock_plan(&[entry("a", 1)], &groups),
            LockPlan {
                reuse: ids(&["g1"]),
                create: vec![],
                sleep: ids(&["g2"]),
            }
        );
        // Plus : seul A#2 est neuf ; un groupe en sommeil repris est réapparié.
        let groups = existing(&[("g1", "a", 1), ("g3", "b", 1)]);
        assert_eq!(
            lock_plan(&[entry("a", 1), entry("a", 2), entry("b", 1)], &groups),
            LockPlan {
                reuse: ids(&["g1", "g3"]),
                create: vec![entry("a", 2)],
                sleep: vec![],
            }
        );
        // Un créneau commencé n'est plus replanifié : son groupe reste tel quel.
        let groups = existing(&[("gp", "past", 1), ("g1", "a", 1), ("g2", "a", 2)]);
        assert_eq!(
            lock_plan(&[entry("a", 1)], &groups),
            LockPlan {
                reuse: ids(&["g1"]),
                create: vec![],
                sleep: ids(&["g2"]),
            }
        );
    }

    #[test]
    fn unlock_needs_an_upcoming_slot() {
        assert!(has_upcoming_slot(now(), &[at(-3), at(1)]));
        assert!(!has_upcoming_slot(now(), &[at(-3), at(0)]));
        assert!(!has_upcoming_slot(now(), &[]));
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
