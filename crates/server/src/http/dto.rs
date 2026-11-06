//! DTO du contrat HTTP (AD-5) : `camelCase`, un champ absent n'existe pas. Miroir TypeScript dans
//! `web/src/api/types.ts`, modifié dans le même commit.

use domain::reference as r;
use serde::Serialize;

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
