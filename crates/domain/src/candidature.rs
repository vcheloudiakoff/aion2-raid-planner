//! Candidature (AD-4) : règles pures sur la saisie brute d'un joueur, contre un événement donné.
//!
//! [`validate_candidature`] prend le `ReferenceSet` une seule fois (AD-6), l'événement visé
//! ([`EventContext`]) et `now` de l'horloge injectée (AD-11). Toutes les erreurs de champ sont rendues
//! ensemble, dans l'ordre des champs du formulaire, avec des chemins JSON camelCase. Faction et rôle
//! ne sont jamais stockés : ils se déduisent du serveur et de la classe.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::Timestamp;
use crate::event::{EventStatus, FactionScope};
use crate::reference::ReferenceSet;
use crate::validate;

/// Candidatures permises par événement, tous statuts confondus (SPEC de l'epic 2, *Assumptions*).
pub const CANDIDATURES_MAX: i64 = 200;

/// Statut d'une candidature (AD-4). `withdrawn` est terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CandidatureStatus {
    Pending,
    Accepted,
    Rejected,
    Withdrawn,
}

impl CandidatureStatus {
    /// Valeur de la colonne `status`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
            Self::Withdrawn => "withdrawn",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        [
            Self::Pending,
            Self::Accepted,
            Self::Rejected,
            Self::Withdrawn,
        ]
        .into_iter()
        .find(|v| v.as_str() == s)
    }

    /// Comptée sur la page publique : `rejected` et `withdrawn` n'y apparaissent pas, pour que la
    /// page ne trahisse aucune décision de l'organisateur.
    pub const fn is_counted(self) -> bool {
        matches!(self, Self::Pending | Self::Accepted)
    }
}

/// Lien par lequel la candidature est arrivée : `/j/` direct, `/e/` public.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Origin {
    Direct,
    Public,
}

impl Origin {
    /// Valeur de la colonne `origin`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::Public => "public",
        }
    }

    /// Valeur lue dans la colonne `origin`.
    pub fn parse(s: &str) -> Option<Self> {
        [Self::Direct, Self::Public]
            .into_iter()
            .find(|v| v.as_str() == s)
    }

    /// Statut initial (AD-4) : accepté d'office par `/j/`, à valider par `/e/`.
    pub const fn initial_status(self) -> CandidatureStatus {
        match self {
            Self::Direct => CandidatureStatus::Accepted,
            Self::Public => CandidatureStatus::Pending,
        }
    }
}

/// Une disponibilité cochée : le créneau et sa préférence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewAvailability {
    pub slot_id: String,
    pub preferred: bool,
}

/// Équipes souhaitées par le joueur dans un groupe à plusieurs Équipes (Ludra : 2 × 5). `teams`
/// vide = peu importe ; `preferred` (l'étoile) n'a de sens que si au moins deux Équipes sont
/// cochées. Numéros à partir de 1.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TeamChoice {
    pub teams: Vec<u32>,
    pub preferred: Option<u32>,
}

impl TeamChoice {
    /// Masque stocké en base : bit `n - 1` pour l'Équipe `n`.
    pub fn mask(&self) -> i64 {
        self.teams
            .iter()
            .filter(|&&t| (1..=62).contains(&t))
            .fold(0, |m, t| m | (1i64 << (t - 1)))
    }

    /// Relit le masque stocké ; `None` si négatif.
    pub fn from_mask(mask: i64, preferred: Option<i64>) -> Option<Self> {
        if mask < 0 {
            return None;
        }
        let teams = (1..=62u32)
            .filter(|t| mask & (1i64 << (t - 1)) != 0)
            .collect();
        let preferred = preferred.and_then(|p| u32::try_from(p).ok());
        Some(Self { teams, preferred })
    }
}

/// Saisie brute du joueur, avant toute règle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewCandidature {
    pub pseudo: String,
    pub server: String,
    pub class: String,
    pub cp: i64,
    pub availabilities: Vec<NewAvailability>,
    pub teams: TeamChoice,
}

/// Créneau de l'événement visé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotRef {
    pub id: String,
    pub starts_at: Timestamp,
}

/// Ce que la validation doit savoir de l'événement visé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventContext {
    pub region: String,
    pub faction_scope: FactionScope,
    pub slots: Vec<SlotRef>,
    /// Classes qui ont une ligne `class_bounds` dans l'événement.
    pub classes: Vec<String>,
    /// Nombre d'Équipes par groupe de l'instance (1 si l'instance est inconnue).
    pub parties: u32,
}

/// Candidature prête à écrire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidCandidature {
    pub pseudo: String,
    pub server: String,
    pub class: String,
    pub cp: u32,
    pub availabilities: Vec<NewAvailability>,
    pub teams: TeamChoice,
}

/// Cas d'erreur d'un champ ; la couche `http` le traduit en code `<entité>.<cas>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CandidatureError {
    PseudoInvalid,
    ReferenceUnknown,
    ServerOutOfRegion,
    FactionOutOfScope,
    CpOutOfRange,
    NoAvailability,
    SlotUnknown,
    SlotInPast,
    TeamInvalid,
}

/// Chemin JSON camelCase du champ fautif (`availabilities.2.slotId`) et son cas.
pub type CandidatureErrors = Vec<(String, CandidatureError)>;

/// Applique toutes les règles de candidature ; rend toutes les erreurs, dans l'ordre des champs.
/// `min_cp` n'est qu'un signal : il ne refuse rien et n'est donc pas lu ici.
pub fn validate_candidature(
    reference: &ReferenceSet,
    event: &EventContext,
    now: Timestamp,
    input: NewCandidature,
) -> Result<ValidCandidature, CandidatureErrors> {
    validate(reference, event, now, input, None).map(|(v, _)| v)
}

/// Contenu enregistré d'une candidature, lu dans la transaction de la modification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentCandidature {
    pub server: String,
    pub class: String,
    /// Disponibilités enregistrées, créneaux passés compris.
    pub availabilities: Vec<NewAvailability>,
}

/// Modification prête à écrire : le nouveau contenu, dont les disponibilités à venir, et le sort
/// des disponibilités enregistrées.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidUpdate {
    /// `availabilities` ne porte que des créneaux à venir.
    pub candidature: ValidCandidature,
    /// Disponibilités enregistrées sur un créneau passé : elles restent telles quelles.
    pub kept: Vec<NewAvailability>,
    /// Créneaux des disponibilités enregistrées à remplacer : tout ce qui n'est pas gardé.
    pub replaced: Vec<String>,
}

/// Mode modification de [`validate_candidature`] (AD-4, AD-6) : la saisie remplace tout le contenu,
/// mais ses disponibilités ne visent que des créneaux à venir ; une disponibilité enregistrée sur un
/// créneau passé reste telle quelle et compte pour la règle « au moins une disponibilité ». Les
/// contrôles « existe et `enabled` » du serveur et de la classe ne portent que sur une valeur changée.
pub fn validate_update(
    reference: &ReferenceSet,
    event: &EventContext,
    now: Timestamp,
    current: &CurrentCandidature,
    input: NewCandidature,
) -> Result<ValidUpdate, CandidatureErrors> {
    let (candidature, kept) = validate(reference, event, now, input, Some(current))?;
    let replaced = current
        .availabilities
        .iter()
        .filter(|a| !kept.contains(a))
        .map(|a| a.slot_id.clone())
        .collect();
    Ok(ValidUpdate {
        candidature,
        kept,
        replaced,
    })
}

/// Disponibilités enregistrées sur un créneau passé de l'événement.
fn kept_availabilities(
    event: &EventContext,
    now: Timestamp,
    current: &CurrentCandidature,
) -> Vec<NewAvailability> {
    current
        .availabilities
        .iter()
        .filter(|a| {
            event
                .slots
                .iter()
                .any(|s| s.id == a.slot_id && s.starts_at <= now)
        })
        .cloned()
        .collect()
}

/// Règles communes à la création (`current` absent) et à la modification.
fn validate(
    reference: &ReferenceSet,
    event: &EventContext,
    now: Timestamp,
    input: NewCandidature,
    current: Option<&CurrentCandidature>,
) -> Result<(ValidCandidature, Vec<NewAvailability>), CandidatureErrors> {
    let mut errors: CandidatureErrors = Vec::new();
    let mut fail = |path: String, e: CandidatureError| errors.push((path, e));

    if validate::pseudo(&input.pseudo).is_err() {
        fail("pseudo".into(), CandidatureError::PseudoInvalid);
    }

    // Une valeur inchangée n'est pas revalidée contre `enabled` (AD-6) ; les invariants de région
    // et de portée restent vérifiés tant que le serveur est connu de la référence.
    let server_kept = current.is_some_and(|c| c.server == input.server);
    match reference
        .servers
        .iter()
        .find(|s| s.id == input.server && (s.enabled || server_kept))
    {
        None if server_kept => {}
        None => fail("server".into(), CandidatureError::ReferenceUnknown),
        Some(s) if s.region != event.region => {
            fail("server".into(), CandidatureError::ServerOutOfRegion)
        }
        Some(s) if !event.faction_scope.admits(s.faction) => {
            fail("server".into(), CandidatureError::FactionOutOfScope)
        }
        Some(_) => {}
    }

    let class_kept = current.is_some_and(|c| c.class == input.class);
    let class_known = class_kept
        || reference
            .classes
            .iter()
            .any(|c| c.enabled && c.id == input.class);
    if !class_known || !event.classes.contains(&input.class) {
        fail("class".into(), CandidatureError::ReferenceUnknown);
    }

    let cp = match validate::cp(input.cp) {
        Ok(cp) => Some(cp),
        Err(_) => {
            fail("cp".into(), CandidatureError::CpOutOfRange);
            None
        }
    };

    let kept = current
        .map(|c| kept_availabilities(event, now, c))
        .unwrap_or_default();
    if input.availabilities.is_empty() && kept.is_empty() {
        fail("availabilities".into(), CandidatureError::NoAvailability);
    }
    let mut seen = HashSet::new();
    for (n, a) in input.availabilities.iter().enumerate() {
        let path = format!("availabilities.{n}.slotId");
        match event.slots.iter().find(|s| s.id == a.slot_id) {
            None => fail(path, CandidatureError::SlotUnknown),
            Some(_) if !seen.insert(a.slot_id.as_str()) => {
                fail(path, CandidatureError::SlotUnknown)
            }
            Some(s) if s.starts_at <= now => fail(path, CandidatureError::SlotInPast),
            Some(_) => {}
        }
    }

    let mut teams = input.teams.teams.clone();
    teams.sort_unstable();
    teams.dedup();
    if teams.len() != input.teams.teams.len() || teams.iter().any(|t| *t == 0 || *t > event.parties)
    {
        fail("teams".into(), CandidatureError::TeamInvalid);
    }
    if let Some(p) = input.teams.preferred
        && (teams.len() < 2 || !teams.contains(&p))
    {
        fail("preferredTeam".into(), CandidatureError::TeamInvalid);
    }

    match cp {
        Some(cp) if errors.is_empty() => Ok((
            ValidCandidature {
                pseudo: input.pseudo,
                server: input.server,
                class: input.class,
                cp,
                availabilities: input.availabilities,
                teams: TeamChoice {
                    teams,
                    preferred: input.teams.preferred,
                },
            },
            kept,
        )),
        _ => Err(errors),
    }
}

/// Geste du participant sur sa candidature (AD-4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParticipantAction {
    Update,
    Withdraw,
}

/// Refus d'un geste du participant, dans l'ordre où les règles sont vérifiées.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParticipantRefusal {
    /// L'événement est annulé.
    EventClosed,
    /// La candidature est déjà retirée.
    Withdrawn,
    /// L'événement est verrouillé : le contenu ne se modifie plus.
    EventLocked,
    /// Le retrait n'existe qu'à partir de `pending` et `accepted`.
    TransitionForbidden,
}

/// Règles d'état d'un geste du participant (AD-4), à vérifier après relecture de l'agrégat :
/// événement annulé, puis candidature retirée, puis la règle propre au geste. Le contenu se modifie
/// en `open` quel que soit le statut ; le retrait vaut en `open` comme en `locked`, depuis `pending`
/// ou `accepted` seulement.
pub fn participant_may(
    event: EventStatus,
    status: CandidatureStatus,
    action: ParticipantAction,
) -> Result<(), ParticipantRefusal> {
    if event == EventStatus::Cancelled {
        return Err(ParticipantRefusal::EventClosed);
    }
    if status == CandidatureStatus::Withdrawn {
        return Err(ParticipantRefusal::Withdrawn);
    }
    match action {
        ParticipantAction::Update if event == EventStatus::Locked => {
            Err(ParticipantRefusal::EventLocked)
        }
        ParticipantAction::Update => Ok(()),
        ParticipantAction::Withdraw => match status {
            CandidatureStatus::Pending | CandidatureStatus::Accepted => Ok(()),
            _ => Err(ParticipantRefusal::TransitionForbidden),
        },
    }
}

/// Statut que l'organisateur demande pour une candidature (AD-4) : il accepte ou il refuse, rien
/// d'autre. `pending` et `withdrawn` ne se demandent pas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Decision {
    Accepted,
    Rejected,
}

impl Decision {
    /// Statut écrit par la décision.
    pub const fn status(self) -> CandidatureStatus {
        match self {
            Self::Accepted => CandidatureStatus::Accepted,
            Self::Rejected => CandidatureStatus::Rejected,
        }
    }
}

/// Refus d'une décision de l'organisateur, dans l'ordre où les règles sont vérifiées.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrganizerRefusal {
    /// L'événement est annulé.
    EventClosed,
    /// La candidature est retirée : plus aucune transition n'en part.
    TransitionForbidden,
}

/// Effet d'une décision permise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transition {
    /// Le statut change : la commande l'écrit.
    Change,
    /// Le statut demandé est déjà le statut courant : rien à écrire.
    Unchanged,
}

/// Règles d'état d'une décision de l'organisateur (AD-4), à vérifier après relecture de l'agrégat :
/// événement annulé, puis candidature retirée, puis les transitions `pending → accepted|rejected` et
/// `accepted ↔ rejected`, en `open` comme en `locked`. Redemander le statut courant est permis et
/// n'écrit rien.
pub fn organizer_may(
    event: EventStatus,
    current: CandidatureStatus,
    decision: Decision,
) -> Result<Transition, OrganizerRefusal> {
    if event == EventStatus::Cancelled {
        return Err(OrganizerRefusal::EventClosed);
    }
    match current {
        CandidatureStatus::Withdrawn => Err(OrganizerRefusal::TransitionForbidden),
        from if from == decision.status() => Ok(Transition::Unchanged),
        CandidatureStatus::Pending | CandidatureStatus::Accepted | CandidatureStatus::Rejected => {
            Ok(Transition::Change)
        }
    }
}

/// Liste des acceptés vue par une candidature (SPEC epic 3 CAP-6, AD-4 *Données exposées*) : les ids
/// de toutes les candidatures `accepted` de l'événement, dans l'ordre reçu (l'ordre de création), et
/// le rang de la candidature lectrice parmi elles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedList<'a> {
    pub ids: Vec<&'a str>,
    /// Rang de la candidature lectrice dans `ids`, à partir de 0.
    pub own: usize,
}

/// Liste des acceptés vue par la candidature `me` : seulement si `me` figure parmi `candidatures`
/// avec le statut `accepted`, quel que soit le statut de l'événement. Elle y figure alors, à son
/// rang. Jamais un `pending`, un `rejected` ni un `withdrawn`.
pub fn accepted_list<'a>(
    me: &str,
    candidatures: &[(&'a str, CandidatureStatus)],
) -> Option<AcceptedList<'a>> {
    let ids: Vec<&'a str> = candidatures
        .iter()
        .filter(|&&(_, status)| status == CandidatureStatus::Accepted)
        .map(|&(id, _)| id)
        .collect();
    let own = ids.iter().position(|&id| id == me)?;
    Some(AcceptedList { ids, own })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference::{Class, Faction, FactionEntry, Factions, Label, Region, Role, Server};

    fn label(en: &str) -> Label {
        Label {
            en: en.into(),
            fr: None,
        }
    }

    fn server(id: &str, region: &str, faction: Faction, enabled: bool) -> Server {
        Server {
            id: id.into(),
            region: region.into(),
            faction,
            tag: id.into(),
            label: label(id),
            enabled,
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

    /// Jeu de test, valeurs fictives.
    fn reference() -> ReferenceSet {
        ReferenceSet {
            regions: vec![
                Region {
                    id: "r1".into(),
                    label: label("R1"),
                    enabled: true,
                },
                Region {
                    id: "r2".into(),
                    label: label("R2"),
                    enabled: true,
                },
            ],
            factions: Factions {
                elyos: FactionEntry { label: label("E") },
                asmodian: FactionEntry { label: label("A") },
            },
            servers: vec![
                server("se", "r1", Faction::Elyos, true),
                server("sa", "r1", Faction::Asmodian, true),
                server("s_far", "r2", Faction::Elyos, true),
                server("s_off", "r1", Faction::Elyos, false),
            ],
            room_label_charset: "R".into(),
            room_label_max: 4,
            instances: vec![],
            classes: vec![
                class("c1", true),
                class("c2", true),
                class("c_off", false),
                class("c_new", true),
            ],
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

    fn event() -> EventContext {
        EventContext {
            region: "r1".into(),
            faction_scope: FactionScope::All,
            slots: vec![
                SlotRef {
                    id: "s1".into(),
                    starts_at: at(24),
                },
                SlotRef {
                    id: "s2".into(),
                    starts_at: at(48),
                },
                SlotRef {
                    id: "past".into(),
                    starts_at: at(-1),
                },
            ],
            // `c_new` est active dans la référence mais n'a pas de borne dans cet événement.
            classes: vec!["c1".into(), "c2".into(), "c_off".into()],
            parties: 2,
        }
    }

    fn avail(slot: &str, preferred: bool) -> NewAvailability {
        NewAvailability {
            slot_id: slot.into(),
            preferred,
        }
    }

    fn input() -> NewCandidature {
        NewCandidature {
            pseudo: "Nyra".into(),
            server: "sa".into(),
            class: "c1".into(),
            cp: 1500,
            availabilities: vec![avail("s1", true), avail("s2", true)],
            teams: TeamChoice::default(),
        }
    }

    fn errors_with(ev: &EventContext, i: NewCandidature) -> CandidatureErrors {
        validate_candidature(&reference(), ev, now(), i).unwrap_err()
    }

    fn errors(i: NewCandidature) -> CandidatureErrors {
        errors_with(&event(), i)
    }

    #[test]
    fn team_choice_is_checked_against_the_parties() {
        let with = |teams: Vec<u32>, preferred: Option<u32>| {
            let mut i = input();
            i.teams = TeamChoice { teams, preferred };
            i
        };
        let ok =
            validate_candidature(&reference(), &event(), now(), with(vec![2, 1], Some(2))).unwrap();
        assert_eq!(ok.teams.teams, vec![1, 2]);
        assert_eq!(ok.teams.preferred, Some(2));
        assert!(validate_candidature(&reference(), &event(), now(), with(vec![], None)).is_ok());
        assert_eq!(
            errors(with(vec![3], None)),
            one("teams", CandidatureError::TeamInvalid)
        );
        assert_eq!(
            errors(with(vec![1, 1], None)),
            one("teams", CandidatureError::TeamInvalid)
        );
        assert_eq!(
            errors(with(vec![1], Some(1))),
            one("preferredTeam", CandidatureError::TeamInvalid)
        );
        let t = TeamChoice {
            teams: vec![1, 2],
            preferred: Some(1),
        };
        assert_eq!(TeamChoice::from_mask(t.mask(), Some(1)), Some(t));
    }

    fn one(path: &str, e: CandidatureError) -> CandidatureErrors {
        vec![(path.into(), e)]
    }

    #[test]
    fn nominal_keeps_every_preference() {
        let v = validate_candidature(&reference(), &event(), now(), input()).unwrap();
        assert_eq!(v.pseudo, "Nyra");
        assert_eq!(v.cp, 1500);
        assert_eq!(v.availabilities, vec![avail("s1", true), avail("s2", true)]);
    }

    #[test]
    fn pseudo_rule() {
        for bad in ["", "a b", &"a".repeat(17)] {
            let mut i = input();
            i.pseudo = bad.into();
            assert_eq!(
                errors(i),
                one("pseudo", CandidatureError::PseudoInvalid),
                "{bad}"
            );
        }
    }

    #[test]
    fn server_must_exist_be_in_region_and_in_scope() {
        for (id, e) in [
            ("nope", CandidatureError::ReferenceUnknown),
            ("s_off", CandidatureError::ReferenceUnknown),
            ("s_far", CandidatureError::ServerOutOfRegion),
        ] {
            let mut i = input();
            i.server = id.into();
            assert_eq!(errors(i), one("server", e), "{id}");
        }
        let mut ev = event();
        ev.faction_scope = FactionScope::Elyos;
        assert_eq!(
            errors_with(&ev, input()),
            one("server", CandidatureError::FactionOutOfScope)
        );
        let mut i = input();
        i.server = "se".into();
        assert!(validate_candidature(&reference(), &ev, now(), i).is_ok());
    }

    #[test]
    fn class_must_be_enabled_and_bounded_in_the_event() {
        for id in ["nope", "c_off", "c_new"] {
            let mut i = input();
            i.class = id.into();
            assert_eq!(
                errors(i),
                one("class", CandidatureError::ReferenceUnknown),
                "{id}"
            );
        }
    }

    #[test]
    fn cp_bounds() {
        for cp in [-1, 1_000_000] {
            let mut i = input();
            i.cp = cp;
            assert_eq!(errors(i), one("cp", CandidatureError::CpOutOfRange), "{cp}");
        }
        for cp in [0, 999_999] {
            let mut i = input();
            i.cp = cp;
            assert!(validate_candidature(&reference(), &event(), now(), i).is_ok());
        }
    }

    #[test]
    fn availabilities_rules() {
        let mut i = input();
        i.availabilities.clear();
        assert_eq!(
            errors(i),
            one("availabilities", CandidatureError::NoAvailability)
        );
        let mut i = input();
        i.availabilities.push(avail("elsewhere", false));
        assert_eq!(
            errors(i),
            one("availabilities.2.slotId", CandidatureError::SlotUnknown)
        );
        let mut i = input();
        i.availabilities.push(avail("past", false));
        assert_eq!(
            errors(i),
            one("availabilities.2.slotId", CandidatureError::SlotInPast)
        );
        let mut i = input();
        i.availabilities.push(avail("s1", false));
        assert_eq!(
            errors(i),
            one("availabilities.2.slotId", CandidatureError::SlotUnknown)
        );
        // Un créneau qui commence maintenant est passé.
        let mut ev = event();
        ev.slots[0].starts_at = now();
        assert_eq!(
            errors_with(&ev, input()),
            one("availabilities.0.slotId", CandidatureError::SlotInPast)
        );
    }

    #[test]
    fn all_errors_are_collected_in_field_order() {
        let i = NewCandidature {
            pseudo: "a".repeat(17),
            server: "s_far".into(),
            class: "nope".into(),
            cp: 1_000_000,
            availabilities: vec![avail("s1", false), avail("x", true)],
            teams: TeamChoice::default(),
        };
        assert_eq!(
            errors(i),
            vec![
                ("pseudo".into(), CandidatureError::PseudoInvalid),
                ("server".into(), CandidatureError::ServerOutOfRegion),
                ("class".into(), CandidatureError::ReferenceUnknown),
                ("cp".into(), CandidatureError::CpOutOfRange),
                (
                    "availabilities.1.slotId".into(),
                    CandidatureError::SlotUnknown
                ),
            ]
        );
    }

    #[test]
    fn statuses_and_origins() {
        for s in [
            CandidatureStatus::Pending,
            CandidatureStatus::Accepted,
            CandidatureStatus::Rejected,
            CandidatureStatus::Withdrawn,
        ] {
            assert_eq!(CandidatureStatus::parse(s.as_str()), Some(s));
        }
        assert!(CandidatureStatus::Pending.is_counted());
        assert!(CandidatureStatus::Accepted.is_counted());
        assert!(!CandidatureStatus::Rejected.is_counted());
        assert!(!CandidatureStatus::Withdrawn.is_counted());
        assert_eq!(Origin::Direct.initial_status(), CandidatureStatus::Accepted);
        assert_eq!(Origin::Public.initial_status(), CandidatureStatus::Pending);
        assert_eq!(Origin::Direct.as_str(), "direct");
        assert_eq!(Origin::Public.as_str(), "public");
        for o in [Origin::Direct, Origin::Public] {
            assert_eq!(Origin::parse(o.as_str()), Some(o));
        }
        assert_eq!(Origin::parse("other"), None);
    }

    fn current() -> CurrentCandidature {
        CurrentCandidature {
            server: "sa".into(),
            class: "c1".into(),
            availabilities: vec![avail("past", true), avail("s1", false)],
        }
    }

    fn update_errors(c: &CurrentCandidature, i: NewCandidature) -> CandidatureErrors {
        validate_update(&reference(), &event(), now(), c, i).unwrap_err()
    }

    #[test]
    fn update_keeps_past_availabilities_and_replaces_the_upcoming_ones() {
        let mut i = input();
        i.cp = 2000;
        i.availabilities = vec![avail("s2", true)];
        let u = validate_update(&reference(), &event(), now(), &current(), i).unwrap();
        assert_eq!(u.candidature.cp, 2000);
        assert_eq!(u.candidature.availabilities, vec![avail("s2", true)]);
        assert_eq!(u.kept, vec![avail("past", true)]);
        assert_eq!(u.replaced, vec!["s1".to_owned()]);
    }

    #[test]
    fn update_targets_upcoming_slots_only() {
        let mut i = input();
        i.availabilities = vec![avail("s1", false), avail("past", true)];
        assert_eq!(
            update_errors(&current(), i),
            one("availabilities.1.slotId", CandidatureError::SlotInPast)
        );
        let mut i = input();
        i.availabilities = vec![avail("gone", false)];
        assert_eq!(
            update_errors(&current(), i),
            one("availabilities.0.slotId", CandidatureError::SlotUnknown)
        );
    }

    #[test]
    fn update_counts_kept_availabilities_for_the_at_least_one_rule() {
        let mut i = input();
        i.availabilities.clear();
        let u = validate_update(&reference(), &event(), now(), &current(), i).unwrap();
        assert!(u.candidature.availabilities.is_empty());
        assert_eq!(u.kept, vec![avail("past", true)]);
        assert_eq!(u.replaced, vec!["s1".to_owned()]);
        let mut c = current();
        c.availabilities = vec![avail("s1", false)];
        let mut i = input();
        i.availabilities.clear();
        assert_eq!(
            update_errors(&c, i),
            one("availabilities", CandidatureError::NoAvailability)
        );
    }

    #[test]
    fn update_checks_enabled_only_on_a_changed_value() {
        // Serveur et classe désactivés depuis, inchangés : acceptés.
        let c = CurrentCandidature {
            server: "s_off".into(),
            class: "c_off".into(),
            availabilities: vec![avail("s1", false)],
        };
        let mut i = input();
        i.server = "s_off".into();
        i.class = "c_off".into();
        assert!(validate_update(&reference(), &event(), now(), &c, i).is_ok());
        // Les mêmes valeurs choisies par une modification : refusées.
        let mut i = input();
        i.server = "s_off".into();
        i.class = "c_off".into();
        assert_eq!(
            update_errors(&current(), i),
            vec![
                ("server".into(), CandidatureError::ReferenceUnknown),
                ("class".into(), CandidatureError::ReferenceUnknown),
            ]
        );
        // Inchangé mais hors région : l'invariant tient toujours.
        let c = CurrentCandidature {
            server: "s_far".into(),
            ..current()
        };
        let mut i = input();
        i.server = "s_far".into();
        assert_eq!(
            update_errors(&c, i),
            one("server", CandidatureError::ServerOutOfRegion)
        );
        // Inchangé et absent de la référence : toléré (AD-6).
        let c = CurrentCandidature {
            server: "retired".into(),
            ..current()
        };
        let mut i = input();
        i.server = "retired".into();
        assert!(validate_update(&reference(), &event(), now(), &c, i).is_ok());
        // Une classe inchangée doit toujours être bornée dans l'événement.
        let c = CurrentCandidature {
            class: "c_new".into(),
            ..current()
        };
        let mut i = input();
        i.class = "c_new".into();
        assert_eq!(
            update_errors(&c, i),
            one("class", CandidatureError::ReferenceUnknown)
        );
    }

    #[test]
    fn update_collects_every_field_error() {
        let i = NewCandidature {
            pseudo: "a b".into(),
            server: "s_far".into(),
            class: "c1".into(),
            cp: -1,
            availabilities: vec![avail("past", false)],
            teams: TeamChoice::default(),
        };
        assert_eq!(
            update_errors(&current(), i),
            vec![
                ("pseudo".into(), CandidatureError::PseudoInvalid),
                ("server".into(), CandidatureError::ServerOutOfRegion),
                ("cp".into(), CandidatureError::CpOutOfRange),
                (
                    "availabilities.0.slotId".into(),
                    CandidatureError::SlotInPast
                ),
            ]
        );
    }

    #[test]
    fn participant_rules_in_order() {
        use CandidatureStatus::*;
        use EventStatus::*;
        use ParticipantAction::*;
        use ParticipantRefusal as R;
        for action in [Update, Withdraw] {
            for status in [Pending, Accepted, Rejected, Withdrawn] {
                assert_eq!(
                    participant_may(Cancelled, status, action),
                    Err(R::EventClosed),
                    "{status:?} {action:?}"
                );
            }
            for event in [Open, Locked] {
                assert_eq!(
                    participant_may(event, Withdrawn, action),
                    Err(R::Withdrawn),
                    "{event:?} {action:?}"
                );
            }
        }
        for status in [Pending, Accepted, Rejected] {
            assert_eq!(participant_may(Open, status, Update), Ok(()), "{status:?}");
            assert_eq!(
                participant_may(Locked, status, Update),
                Err(R::EventLocked),
                "{status:?}"
            );
        }
        for event in [Open, Locked] {
            for status in [Pending, Accepted] {
                assert_eq!(participant_may(event, status, Withdraw), Ok(()));
            }
            assert_eq!(
                participant_may(event, Rejected, Withdraw),
                Err(R::TransitionForbidden)
            );
        }
    }

    #[test]
    fn organizer_rules_are_exhaustive_and_in_order() {
        use CandidatureStatus::*;
        use Decision as D;
        use EventStatus::*;
        use OrganizerRefusal as R;
        use Transition::*;
        // Chaque combinaison (événement, statut courant, décision), avec son résultat attendu.
        let expected = |event: EventStatus, from: CandidatureStatus, to: D| match (event, from, to)
        {
            (Cancelled, _, _) => Err(R::EventClosed),
            (_, Withdrawn, _) => Err(R::TransitionForbidden),
            (_, Pending, _) => Ok(Change),
            (_, Accepted, D::Rejected) | (_, Rejected, D::Accepted) => Ok(Change),
            (_, Accepted, D::Accepted) | (_, Rejected, D::Rejected) => Ok(Unchanged),
        };
        let mut seen = 0;
        for event in [Open, Locked, Cancelled] {
            for from in [Pending, Accepted, Rejected, Withdrawn] {
                for to in [D::Accepted, D::Rejected] {
                    assert_eq!(
                        organizer_may(event, from, to),
                        expected(event, from, to),
                        "{event:?} {from:?} -> {to:?}"
                    );
                    seen += 1;
                }
            }
        }
        assert_eq!(seen, 24);
        // Lecture directe d'AD-4, en `open` comme en `locked`.
        for event in [Open, Locked] {
            assert_eq!(organizer_may(event, Pending, D::Accepted), Ok(Change));
            assert_eq!(organizer_may(event, Pending, D::Rejected), Ok(Change));
            assert_eq!(organizer_may(event, Accepted, D::Rejected), Ok(Change));
            assert_eq!(organizer_may(event, Rejected, D::Accepted), Ok(Change));
            assert_eq!(organizer_may(event, Accepted, D::Accepted), Ok(Unchanged));
            assert_eq!(
                organizer_may(event, Withdrawn, D::Accepted),
                Err(R::TransitionForbidden)
            );
        }
        // L'annulation passe avant le statut retiré.
        assert_eq!(
            organizer_may(Cancelled, Withdrawn, D::Accepted),
            Err(R::EventClosed)
        );
        assert_eq!(D::Accepted.status(), Accepted);
        assert_eq!(D::Rejected.status(), Rejected);
    }

    const ROSTER: &[(&str, CandidatureStatus)] = &[
        ("a", CandidatureStatus::Accepted),
        ("p", CandidatureStatus::Pending),
        ("b", CandidatureStatus::Accepted),
        ("r", CandidatureStatus::Rejected),
        ("w", CandidatureStatus::Withdrawn),
        ("c", CandidatureStatus::Accepted),
    ];

    #[test]
    fn an_accepted_candidature_sees_every_accepted_one_in_order_and_its_own_rank() {
        for (me, own) in [("a", 0), ("b", 1), ("c", 2)] {
            assert_eq!(
                accepted_list(me, ROSTER),
                Some(AcceptedList {
                    ids: vec!["a", "b", "c"],
                    own
                }),
                "{me}"
            );
        }
        // Seule acceptée : elle se voit seule.
        assert_eq!(
            accepted_list(
                "solo",
                &[
                    ("solo", CandidatureStatus::Accepted),
                    ("q", CandidatureStatus::Pending)
                ]
            ),
            Some(AcceptedList {
                ids: vec!["solo"],
                own: 0
            })
        );
    }

    #[test]
    fn a_candidature_that_is_not_accepted_or_unknown_sees_no_list() {
        for me in ["p", "r", "w", "unknown"] {
            assert_eq!(accepted_list(me, ROSTER), None, "{me}");
        }
        assert_eq!(accepted_list("a", &[]), None);
    }
}
