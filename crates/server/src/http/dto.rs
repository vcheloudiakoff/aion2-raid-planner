//! DTO du contrat HTTP (AD-5) : `camelCase`, un champ absent n'existe pas. Miroir TypeScript dans
//! `web/src/api/types.ts`, modifié dans le même commit.

use std::collections::BTreeMap;

use domain::Timestamp;
use domain::candidature::{self as cand, CandidatureStatus, Decision, Origin};
use domain::event::{self as ev, EventStatus, FactionScope, Visibility};
use domain::reference as r;
use serde::{Deserialize, Serialize};

/// Réponse de `GET /api/form-token` : jeton opaque à renvoyer tel quel dans `formToken`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormToken {
    pub token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Label {
    pub en: String,
    /// Absent si non saisi : le client retombe sur `en`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fr: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Faction {
    Elyos,
    Asmodian,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum InstanceKind {
    Sanctuary,
    Expedition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Region {
    pub id: String,
    pub label: Label,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Factions {
    pub elyos: Label,
    pub asmodian: Label,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Server {
    pub id: String,
    pub region: String,
    pub faction: Faction,
    pub tag: String,
    pub label: Label,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Instance {
    pub id: String,
    pub kind: InstanceKind,
    pub parties: u32,
    pub party_size: u32,
    pub duration_min: u32,
    pub cross_faction: bool,
    pub room_label: String,
    pub label: Label,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Class {
    pub id: String,
    pub role: String,
    pub default_min: u32,
    pub default_max: u32,
    pub label: Label,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Role {
    pub id: String,
    pub label: Label,
    pub enabled: bool,
}

/// `GET /api/reference` : le jeu entier, entrées désactivées comprises (`enabled: false`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceSet {
    pub regions: Vec<Region>,
    pub factions: Factions,
    pub servers: Vec<Server>,
    pub instances: Vec<Instance>,
    pub classes: Vec<Class>,
    pub roles: Vec<Role>,
}

impl From<&r::Label> for Label {
    fn from(l: &r::Label) -> Self {
        Self {
            en: l.en.clone(),
            fr: l.fr.clone(),
        }
    }
}

impl From<r::Faction> for Faction {
    fn from(f: r::Faction) -> Self {
        match f {
            r::Faction::Elyos => Self::Elyos,
            r::Faction::Asmodian => Self::Asmodian,
        }
    }
}

impl From<r::InstanceKind> for InstanceKind {
    fn from(k: r::InstanceKind) -> Self {
        match k {
            r::InstanceKind::Sanctuary => Self::Sanctuary,
            r::InstanceKind::Expedition => Self::Expedition,
        }
    }
}

impl From<&r::ReferenceSet> for ReferenceSet {
    fn from(s: &r::ReferenceSet) -> Self {
        Self {
            regions: s
                .regions
                .iter()
                .map(|x| Region {
                    id: x.id.clone(),
                    label: (&x.label).into(),
                    enabled: x.enabled,
                })
                .collect(),
            factions: Factions {
                elyos: (&s.factions.elyos.label).into(),
                asmodian: (&s.factions.asmodian.label).into(),
            },
            servers: s
                .servers
                .iter()
                .map(|x| Server {
                    id: x.id.clone(),
                    region: x.region.clone(),
                    faction: x.faction.into(),
                    tag: x.tag.clone(),
                    label: (&x.label).into(),
                    enabled: x.enabled,
                })
                .collect(),
            instances: s
                .instances
                .iter()
                .map(|x| Instance {
                    id: x.id.clone(),
                    kind: x.kind.into(),
                    parties: x.parties,
                    party_size: x.party_size,
                    duration_min: x.duration_min,
                    cross_faction: x.cross_faction,
                    room_label: x.room_label.clone(),
                    label: (&x.label).into(),
                    enabled: x.enabled,
                })
                .collect(),
            classes: s
                .classes
                .iter()
                .map(|x| Class {
                    id: x.id.clone(),
                    role: x.role.clone(),
                    default_min: x.default_min,
                    default_max: x.default_max,
                    label: (&x.label).into(),
                    enabled: x.enabled,
                })
                .collect(),
            roles: s
                .roles
                .iter()
                .map(|x| Role {
                    id: x.id.clone(),
                    label: (&x.label).into(),
                    enabled: x.enabled,
                })
                .collect(),
        }
    }
}

/// Corps de `POST /api/events`, à plat dans `FormEnvelope` à côté du jeton et du honeypot. Champs
/// numériques en `i64` : une valeur hors bornes est une erreur de champ, pas un corps illisible.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateEvent {
    pub instance: String,
    pub region: String,
    pub faction_scope: FactionScope,
    pub slots: Vec<NewSlot>,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub discord_url: Option<String>,
    #[serde(default)]
    pub visibility: Option<Visibility>,
    #[serde(default)]
    pub min_cp: Option<i64>,
    /// `classId → {min, max}` ; une classe omise prend ses bornes par défaut.
    #[serde(default)]
    pub class_bounds: Option<BTreeMap<String, NewBound>>,
}

/// Créneau saisi : un des `slots` de `POST /api/events`, ou le corps de `POST /api/organizer/slots`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewSlot {
    pub starts_at: Timestamp,
    /// Absente : durée de l'instance.
    #[serde(default)]
    pub duration_min: Option<i64>,
}

impl From<NewSlot> for ev::NewSlot {
    fn from(s: NewSlot) -> Self {
        Self {
            starts_at: s.starts_at,
            duration_min: s.duration_min,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewBound {
    pub min: i64,
    pub max: i64,
}

impl From<CreateEvent> for ev::NewEvent {
    fn from(c: CreateEvent) -> Self {
        Self {
            instance: c.instance,
            region: c.region,
            faction_scope: c.faction_scope,
            slots: c.slots.into_iter().map(Into::into).collect(),
            note: c.note.unwrap_or_default(),
            discord_url: c.discord_url,
            visibility: c.visibility.unwrap_or_default(),
            min_cp: c.min_cp,
            class_bounds: bounds(c.class_bounds.unwrap_or_default()),
        }
    }
}

fn bounds(all: BTreeMap<String, NewBound>) -> BTreeMap<String, ev::NewBound> {
    all.into_iter()
        .map(|(k, b)| {
            (
                k,
                ev::NewBound {
                    min: b.min,
                    max: b.max,
                },
            )
        })
        .collect()
}

/// Réponse 201 de `POST /api/events`, rendue une seule fois. Liens relatifs : la SPA les préfixe par
/// son origine. Les secrets bruts sont là pour `links.v1`, qui les garde sur l'appareil.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedEvent {
    pub event_id: String,
    pub organizer_secret: String,
    pub join_secret: String,
    pub links: CreatedLinks,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedLinks {
    /// `/o/<secret>`
    pub organizer: String,
    /// `/j/<secret>`
    pub join: String,
    /// `/e/<eventId>`
    pub public: String,
}

/// Réponse de `GET /api/events/{eventId}` et de `GET /api/join` : l'événement vu par le public.
/// Jamais de pseudo, de serveur de candidat, d'id de candidature, de `discordUrl`, d'
/// `adminUnlistedAt` ni de `createdAt` (AD-4 *Données exposées*) ; les comptes sont calculés ici.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicEvent {
    pub id: String,
    pub instance: String,
    pub region: String,
    pub faction_scope: FactionScope,
    pub visibility: Visibility,
    pub status: EventStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_cp: Option<u32>,
    pub note: String,
    /// Triés par `startsAt`.
    pub slots: Vec<PublicSlot>,
    /// `classId → n`, une entrée par classe bornée de l'événement, zéros compris.
    pub counts_by_class: BTreeMap<String, u32>,
    /// `roleId → n`.
    pub counts_by_role: BTreeMap<String, u32>,
    /// Manques strictement positifs, triés par id de rôle.
    pub missing_roles: Vec<MissingRole>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicSlot {
    pub id: String,
    pub starts_at: Timestamp,
    pub duration_min: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MissingRole {
    pub role: String,
    pub missing: u32,
}

/// Corps de `POST /api/events/{eventId}/candidatures` et de `POST /api/join/candidatures`, à plat
/// dans `FormEnvelope`. `cp` en `i64` : une valeur hors bornes est une erreur de champ.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitCandidature {
    pub pseudo: String,
    pub server: String,
    pub class: String,
    pub cp: i64,
    /// Une entrée par créneau coché ; `preferred` ne peut donc viser qu'un créneau coché.
    pub availabilities: Vec<AvailabilityInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailabilityInput {
    pub slot_id: String,
    #[serde(default)]
    pub preferred: bool,
}

impl From<SubmitCandidature> for cand::NewCandidature {
    fn from(c: SubmitCandidature) -> Self {
        Self {
            pseudo: c.pseudo,
            server: c.server,
            class: c.class,
            cp: c.cp,
            availabilities: c
                .availabilities
                .into_iter()
                .map(|a| cand::NewAvailability {
                    slot_id: a.slot_id,
                    preferred: a.preferred,
                })
                .collect(),
        }
    }
}

/// Réponse 201 d'une candidature, rendue une seule fois. Lien relatif, à préfixer par l'origine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedCandidature {
    pub event_id: String,
    pub candidature_id: String,
    pub participant_secret: String,
    pub status: CandidatureStatus,
    pub links: ParticipantLinks,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParticipantLinks {
    /// `/p/<secret>`
    pub participant: String,
}

/// Réponse de `GET /api/participant` et des commandes du participant : sa seule candidature et
/// l'événement public. Jamais une autre candidature ni un autre nom (AD-4 *Données exposées*).
/// `discordUrl` n'existe que si la candidature est `accepted` et que l'événement a un lien.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParticipantView {
    pub event_id: String,
    pub candidature_id: String,
    pub pseudo: String,
    pub server: String,
    pub class: String,
    pub cp: u32,
    pub status: CandidatureStatus,
    /// Triées par début de créneau, créneaux passés compris.
    pub availabilities: Vec<Availability>,
    pub event: PublicEvent,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discord_url: Option<String>,
}

/// Un créneau coché par une candidature, et sa préférence. Partagé par les vues participant et
/// organisateur.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Availability {
    pub slot_id: String,
    pub preferred: bool,
}

/// Corps de `POST /api/participant/withdrawal` : vide, `{}`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Withdrawal {}

/// Réponse de `GET /api/organizer` et des commandes de l'organisateur, sous son lien `/o/` : tout
/// le `PublicEvent`, plus ce que seul l'organisateur voit (AD-4 *Données exposées*). `discordUrl`
/// n'existe que si l'événement a un lien.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizerEvent {
    pub event: PublicEvent,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discord_url: Option<String>,
    /// `classId → {min, max}`, une entrée par classe bornée de l'événement.
    pub class_bounds: BTreeMap<String, ClassBound>,
    /// Une entrée par créneau, dans l'ordre de `event.slots` : disponibles parmi les `accepted`.
    pub slots: Vec<OrganizerSlot>,
    /// Groupes de l'événement, en sommeil compris (`retained` faux), triés par début de créneau
    /// puis par `occurrence`. Toujours présent, vide avant le premier verrouillage.
    pub groups: Vec<OrganizerGroup>,
    /// Ordre de création (`created_at` puis `id`), jamais réordonnées.
    pub candidatures: Vec<OrganizerCandidature>,
}

/// Couverture d'un créneau par les candidatures `accepted` (jamais `pending`, `rejected` ni
/// `withdrawn`) : leur nombre, et leur nombre par classe. `coverage` a une entrée par classe bornée
/// (zéros compris), plus toute classe `accepted` sans borne. `hasGroups` : un groupe existe sur ce
/// créneau, même en sommeil ; le supprimer le perdrait (AD-4 *Mutabilité*).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizerSlot {
    pub slot_id: String,
    pub available_accepted: u32,
    pub coverage: BTreeMap<String, u32>,
    pub has_groups: bool,
}

/// Un groupe vu par l'organisateur (AD-4 *Verrouillage*) : son créneau et son rang d'occurrence,
/// `retained` s'il fait partie du verrouillage courant, la disposition et le libellé figés à sa
/// création, et son nom de salle composé par le serveur (AD-18). Le libellé « G1 » est calculé par
/// le client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizerGroup {
    pub id: String,
    pub slot_id: String,
    pub occurrence: u32,
    pub retained: bool,
    pub parties: u32,
    pub party_size: u32,
    pub room_label: String,
    pub room_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassBound {
    pub min: u32,
    pub max: u32,
}

/// Une candidature vue par l'organisateur. `pseudoSrv` est composé par le serveur (AD-6) ; `role`
/// est le rôle de la classe, ou l'id de la classe si la référence ne la connaît plus. `duplicate`
/// signale un autre `pending` ou `accepted` au même pseudo (sans casse) et au même serveur.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizerCandidature {
    pub id: String,
    pub pseudo: String,
    pub server: String,
    pub pseudo_srv: String,
    pub class: String,
    pub role: String,
    pub cp: u32,
    pub status: CandidatureStatus,
    pub origin: Origin,
    /// Triées par début de créneau, créneaux passés compris.
    pub availabilities: Vec<Availability>,
    pub duplicate: bool,
}

/// Corps de `POST /api/organizer/candidature-status`. `status` ne vaut que `accepted` ou `rejected` :
/// toute autre valeur rend le corps illisible (`form.malformed`). L'id vise une candidature que la
/// commande cherche dans l'événement du lien, jamais ailleurs.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetCandidatureStatus {
    pub candidature_id: String,
    pub status: Decision,
}

/// Corps de `POST /api/organizer/settings` : il remplace tous les réglages. `discordUrl` absent ou
/// vide et `minCp` absent effacent la valeur ; une classe omise de `classBounds` garde ses bornes.
/// Nombres en `i64` : une valeur hors bornes est une erreur de champ, pas un corps illisible.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateEventSettings {
    pub instance: String,
    pub region: String,
    pub faction_scope: FactionScope,
    pub note: String,
    #[serde(default)]
    pub discord_url: Option<String>,
    pub visibility: Visibility,
    #[serde(default)]
    pub min_cp: Option<i64>,
    pub class_bounds: BTreeMap<String, NewBound>,
}

impl From<UpdateEventSettings> for ev::NewSettings {
    fn from(u: UpdateEventSettings) -> Self {
        Self {
            instance: u.instance,
            region: u.region,
            faction_scope: u.faction_scope,
            note: u.note,
            discord_url: u.discord_url,
            visibility: u.visibility,
            min_cp: u.min_cp,
            class_bounds: bounds(u.class_bounds),
        }
    }
}

/// Corps de `POST /api/organizer/slot-removal`. L'id vise un créneau que la commande cherche dans
/// l'événement du lien, jamais ailleurs.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlotRemoval {
    pub slot_id: String,
}

/// Corps de `POST /api/organizer/cancellation` : vide, `{}`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Cancellation {}

/// Corps de `POST /api/organizer/lock` : les créneaux retenus, un id par groupe, doublons permis
/// (deux groupes sur le même créneau). Chaque id vise un créneau que la commande cherche dans
/// l'événement du lien, jamais ailleurs.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LockEvent {
    pub slot_ids: Vec<String>,
}

/// Corps de `POST /api/organizer/unlock` : vide, `{}`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Unlock {}
