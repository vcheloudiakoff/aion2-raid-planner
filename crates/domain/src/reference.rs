//! Jeu de référence du jeu (AD-6) : types désérialisés depuis les cinq fichiers TOML et règles de
//! validation pures, partagées par le démarrage, le rechargement et `validate-reference`.
//!
//! Aucune valeur de référence ici : seuls la forme et les contrôles de `reference-schema.md`. Les
//! factions sont l'exception documentée : deux valeurs du domaine, pas des données.

use std::collections::HashSet;
use std::fmt;

use serde::Deserialize;

/// Longueur maximale d'un id de référence.
pub const ID_MAX: usize = 32;
/// Longueur maximale d'un `tag` de serveur, en caractères.
pub const TAG_MAX: usize = 16;

/// Libellé bilingue : `en` obligatoire, `fr` facultatif avec repli sur `en`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Label {
    pub en: String,
    #[serde(default)]
    pub fr: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Faction {
    Elyos,
    Asmodian,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstanceKind {
    Sanctuary,
    Expedition,
}

fn enabled_by_default() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Region {
    pub id: String,
    pub label: Label,
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
}

/// Libellé d'une faction, sous `[factions.<faction>]`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FactionEntry {
    pub label: Label,
}

/// `[factions]` en tête de `servers.toml` : exactement les deux factions.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Factions {
    pub elyos: FactionEntry,
    pub asmodian: FactionEntry,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Server {
    pub id: String,
    pub region: String,
    pub faction: Faction,
    pub tag: String,
    pub label: Label,
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Instance {
    pub id: String,
    pub kind: InstanceKind,
    pub parties: u32,
    pub party_size: u32,
    pub duration_min: u32,
    pub cross_faction: bool,
    pub room_label: String,
    pub label: Label,
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Class {
    pub id: String,
    pub role: String,
    pub default_min: u32,
    pub default_max: u32,
    pub label: Label,
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Role {
    pub id: String,
    pub label: Label,
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
}

/// Contenu de `regions.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegionsFile {
    #[serde(default)]
    pub region: Vec<Region>,
}

/// Contenu de `servers.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServersFile {
    pub factions: Factions,
    #[serde(default)]
    pub server: Vec<Server>,
}

/// Contenu de `instances.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstancesFile {
    pub room_label_charset: String,
    pub room_label_max: u32,
    #[serde(default)]
    pub instance: Vec<Instance>,
}

/// Contenu de `classes.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClassesFile {
    #[serde(default)]
    pub class: Vec<Class>,
}

/// Contenu de `roles.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RolesFile {
    #[serde(default)]
    pub role: Vec<Role>,
}

/// Jeu complet, immuable une fois validé. Entrées désactivées comprises.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceSet {
    pub regions: Vec<Region>,
    pub factions: Factions,
    pub servers: Vec<Server>,
    pub room_label_charset: String,
    pub room_label_max: u32,
    pub instances: Vec<Instance>,
    pub classes: Vec<Class>,
    pub roles: Vec<Role>,
}

impl ReferenceSet {
    pub fn from_files(
        regions: RegionsFile,
        servers: ServersFile,
        instances: InstancesFile,
        classes: ClassesFile,
        roles: RolesFile,
    ) -> Self {
        Self {
            regions: regions.region,
            factions: servers.factions,
            servers: servers.server,
            room_label_charset: instances.room_label_charset,
            room_label_max: instances.room_label_max,
            instances: instances.instance,
            classes: classes.class,
            roles: roles.role,
        }
    }
}

/// Les cinq fichiers, dans l'ordre de lecture ; les seuls lus (AD-6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReferenceFile {
    Regions,
    Servers,
    Instances,
    Classes,
    Roles,
}

impl ReferenceFile {
    pub const ALL: [Self; 5] = [
        Self::Regions,
        Self::Servers,
        Self::Instances,
        Self::Classes,
        Self::Roles,
    ];

    pub fn file_name(self) -> &'static str {
        match self {
            Self::Regions => "regions.toml",
            Self::Servers => "servers.toml",
            Self::Instances => "instances.toml",
            Self::Classes => "classes.toml",
            Self::Roles => "roles.toml",
        }
    }
}

impl fmt::Display for ReferenceFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.file_name())
    }
}

/// Une violation : fichier, entrée (id, ou `#<rang>` si l'id est vide, ou nom du champ de tête) et
/// cause.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceIssue {
    pub file: ReferenceFile,
    pub entry: String,
    pub cause: IssueCause,
}

impl fmt::Display for ReferenceIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}: {}", self.file, self.entry, self.cause)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IssueCause {
    #[error("id must match [a-z0-9_-]{{1,{ID_MAX}}}")]
    InvalidId,
    #[error("duplicate id")]
    DuplicateId,
    #[error("label.en is missing or blank")]
    MissingEnglishLabel,
    #[error("label.fr is blank (omit it to fall back to en)")]
    BlankFrenchLabel,
    #[error("unknown region `{0}`")]
    UnknownRegion(String),
    #[error("unknown role `{0}`")]
    UnknownRole(String),
    #[error("tag is blank")]
    BlankTag,
    #[error("tag must not contain `[` or `]`")]
    TagBrackets,
    #[error("tag is longer than {TAG_MAX} characters")]
    TagTooLong,
    #[error("tag `{0}` is already used in this region")]
    DuplicateTag(String),
    #[error("parties must be at least 1")]
    NoParties,
    #[error("party_size must be at least 1")]
    EmptyParty,
    #[error("duration_min must be greater than 0")]
    ZeroDuration,
    #[error("default_min is greater than default_max")]
    MinAboveMax,
    #[error("room_label_charset is empty")]
    EmptyRoomLabelCharset,
    #[error("room_label_max must be at least 1")]
    ZeroRoomLabelMax,
    #[error("room_label is blank")]
    BlankRoomLabel,
    #[error("room_label is longer than room_label_max ({0})")]
    RoomLabelTooLong(u32),
    #[error("room_label contains `{0}`, outside room_label_charset")]
    RoomLabelOutsideCharset(char),
}

/// Vérifie tous les contrôles de `reference-schema.md` et rend la liste complète des violations.
pub fn validate(set: &ReferenceSet) -> Result<(), Vec<ReferenceIssue>> {
    let mut issues = Vec::new();
    let mut push = |file, entry: &str, cause| {
        issues.push(ReferenceIssue {
            file,
            entry: entry.to_owned(),
            cause,
        })
    };

    let region_ids = check_entries(
        ReferenceFile::Regions,
        set.regions.iter().map(|r| (r.id.as_str(), &r.label)),
        &mut push,
    );
    let role_ids = check_entries(
        ReferenceFile::Roles,
        set.roles.iter().map(|r| (r.id.as_str(), &r.label)),
        &mut push,
    );

    for (name, faction) in [
        ("factions.elyos", &set.factions.elyos),
        ("factions.asmodian", &set.factions.asmodian),
    ] {
        check_label(ReferenceFile::Servers, name, &faction.label, &mut push);
    }
    check_entries(
        ReferenceFile::Servers,
        set.servers.iter().map(|s| (s.id.as_str(), &s.label)),
        &mut push,
    );
    let mut tags = HashSet::new();
    for (i, s) in set.servers.iter().enumerate() {
        let entry = entry_name(&s.id, i);
        let f = ReferenceFile::Servers;
        if !region_ids.contains(s.region.as_str()) {
            push(f, &entry, IssueCause::UnknownRegion(s.region.clone()));
        }
        if s.tag.trim().is_empty() {
            push(f, &entry, IssueCause::BlankTag);
        } else {
            if s.tag.contains(['[', ']']) {
                push(f, &entry, IssueCause::TagBrackets);
            }
            if s.tag.chars().count() > TAG_MAX {
                push(f, &entry, IssueCause::TagTooLong);
            }
            if !tags.insert((s.region.as_str(), s.tag.as_str())) {
                push(f, &entry, IssueCause::DuplicateTag(s.tag.clone()));
            }
        }
    }

    let f = ReferenceFile::Instances;
    let charset: HashSet<char> = set.room_label_charset.chars().collect();
    if charset.is_empty() {
        push(f, "room_label_charset", IssueCause::EmptyRoomLabelCharset);
    }
    if set.room_label_max == 0 {
        push(f, "room_label_max", IssueCause::ZeroRoomLabelMax);
    }
    check_entries(
        f,
        set.instances.iter().map(|i| (i.id.as_str(), &i.label)),
        &mut push,
    );
    for (i, inst) in set.instances.iter().enumerate() {
        let entry = entry_name(&inst.id, i);
        if inst.parties < 1 {
            push(f, &entry, IssueCause::NoParties);
        }
        if inst.party_size < 1 {
            push(f, &entry, IssueCause::EmptyParty);
        }
        if inst.duration_min == 0 {
            push(f, &entry, IssueCause::ZeroDuration);
        }
        if inst.room_label.trim().is_empty() {
            push(f, &entry, IssueCause::BlankRoomLabel);
            continue;
        }
        if inst.room_label.chars().count() > set.room_label_max as usize {
            push(f, &entry, IssueCause::RoomLabelTooLong(set.room_label_max));
        }
        if !charset.is_empty()
            && let Some(c) = inst.room_label.chars().find(|c| !charset.contains(c))
        {
            push(f, &entry, IssueCause::RoomLabelOutsideCharset(c));
        }
    }

    let f = ReferenceFile::Classes;
    check_entries(
        f,
        set.classes.iter().map(|c| (c.id.as_str(), &c.label)),
        &mut push,
    );
    for (i, c) in set.classes.iter().enumerate() {
        let entry = entry_name(&c.id, i);
        if !role_ids.contains(c.role.as_str()) {
            push(f, &entry, IssueCause::UnknownRole(c.role.clone()));
        }
        if c.default_min > c.default_max {
            push(f, &entry, IssueCause::MinAboveMax);
        }
    }

    if issues.is_empty() {
        Ok(())
    } else {
        Err(issues)
    }
}

/// `[a-z0-9_-]{1,32}`.
pub fn is_valid_id(id: &str) -> bool {
    (1..=ID_MAX).contains(&id.len())
        && id
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_' || c == b'-')
}

fn entry_name(id: &str, index: usize) -> String {
    if id.is_empty() {
        format!("#{}", index + 1)
    } else {
        id.to_owned()
    }
}

/// Contrôles communs : forme de l'id, unicité, libellés. Rend l'ensemble des ids vus.
fn check_entries<'a>(
    file: ReferenceFile,
    entries: impl Iterator<Item = (&'a str, &'a Label)>,
    push: &mut impl FnMut(ReferenceFile, &str, IssueCause),
) -> HashSet<&'a str> {
    let mut seen = HashSet::new();
    for (i, (id, label)) in entries.enumerate() {
        let entry = entry_name(id, i);
        if !is_valid_id(id) {
            push(file, &entry, IssueCause::InvalidId);
        }
        if !seen.insert(id) {
            push(file, &entry, IssueCause::DuplicateId);
        }
        check_label(file, &entry, label, push);
    }
    seen
}

fn check_label(
    file: ReferenceFile,
    entry: &str,
    label: &Label,
    push: &mut impl FnMut(ReferenceFile, &str, IssueCause),
) {
    if label.en.trim().is_empty() {
        push(file, entry, IssueCause::MissingEnglishLabel);
    }
    if label.fr.as_deref().is_some_and(|fr| fr.trim().is_empty()) {
        push(file, entry, IssueCause::BlankFrenchLabel);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn label(en: &str) -> Label {
        Label {
            en: en.to_owned(),
            fr: None,
        }
    }

    /// Jeu minimal valide, valeurs de test seulement.
    fn sample() -> ReferenceSet {
        ReferenceSet {
            regions: vec![Region {
                id: "r1".into(),
                label: label("Region One"),
                enabled: true,
            }],
            factions: Factions {
                elyos: FactionEntry { label: label("E") },
                asmodian: FactionEntry { label: label("A") },
            },
            servers: vec![
                Server {
                    id: "s1".into(),
                    region: "r1".into(),
                    faction: Faction::Elyos,
                    tag: "One".into(),
                    label: label("Server One"),
                    enabled: true,
                },
                Server {
                    id: "s2".into(),
                    region: "r1".into(),
                    faction: Faction::Asmodian,
                    tag: "Two".into(),
                    label: label("Server Two"),
                    enabled: false,
                },
            ],
            room_label_charset: "ABC123".into(),
            room_label_max: 4,
            instances: vec![Instance {
                id: "i1".into(),
                kind: InstanceKind::Sanctuary,
                parties: 2,
                party_size: 5,
                duration_min: 60,
                cross_faction: true,
                room_label: "AB12".into(),
                label: label("Instance"),
                enabled: true,
            }],
            classes: vec![Class {
                id: "c1".into(),
                role: "ro1".into(),
                default_min: 0,
                default_max: 0,
                label: label("Class"),
                enabled: true,
            }],
            roles: vec![Role {
                id: "ro1".into(),
                label: label("Role"),
                enabled: true,
            }],
        }
    }

    fn causes(set: &ReferenceSet) -> Vec<(ReferenceFile, String, IssueCause)> {
        validate(set)
            .err()
            .unwrap_or_default()
            .into_iter()
            .map(|i| (i.file, i.entry, i.cause))
            .collect()
    }

    fn one(
        file: ReferenceFile,
        entry: &str,
        cause: IssueCause,
    ) -> Vec<(ReferenceFile, String, IssueCause)> {
        vec![(file, entry.to_owned(), cause)]
    }

    #[test]
    fn sample_is_valid() {
        assert_eq!(validate(&sample()), Ok(()));
    }

    #[test]
    fn ids_must_be_short_lowercase_slugs() {
        for ok in ["a", "na_west", "x-1", &"a".repeat(ID_MAX)] {
            assert!(is_valid_id(ok), "{ok}");
        }
        for bad in ["", "Europe", "é", "a b", "a.b", &"a".repeat(ID_MAX + 1)] {
            assert!(!is_valid_id(bad), "{bad}");
        }
        let mut s = sample();
        s.roles[0].id = "Role".into();
        s.classes[0].role = "Role".into();
        assert_eq!(
            causes(&s),
            one(ReferenceFile::Roles, "Role", IssueCause::InvalidId)
        );
        let mut s = sample();
        s.regions[0].id = String::new();
        let found = causes(&s);
        assert!(found.contains(&(ReferenceFile::Regions, "#1".into(), IssueCause::InvalidId)));
    }

    #[test]
    fn duplicate_ids_are_rejected_in_every_file() {
        let mut s = sample();
        s.regions.push(s.regions[0].clone());
        s.servers.push(Server {
            tag: "Three".into(),
            ..s.servers[0].clone()
        });
        let mut inst = s.instances[0].clone();
        inst.enabled = false;
        s.instances.push(inst);
        s.classes.push(s.classes[0].clone());
        s.roles.push(s.roles[0].clone());
        let found = causes(&s);
        for (file, id) in [
            (ReferenceFile::Regions, "r1"),
            (ReferenceFile::Servers, "s1"),
            (ReferenceFile::Instances, "i1"),
            (ReferenceFile::Classes, "c1"),
            (ReferenceFile::Roles, "ro1"),
        ] {
            assert!(
                found.contains(&(file, id.into(), IssueCause::DuplicateId)),
                "{file}: {found:?}"
            );
        }
        assert_eq!(found.len(), 5);
    }

    #[test]
    fn english_label_is_required_and_french_never_blank() {
        let mut s = sample();
        s.classes[0].label.en = "  ".into();
        assert_eq!(
            causes(&s),
            one(
                ReferenceFile::Classes,
                "c1",
                IssueCause::MissingEnglishLabel
            )
        );
        let mut s = sample();
        s.factions.asmodian.label.en = String::new();
        assert_eq!(
            causes(&s),
            one(
                ReferenceFile::Servers,
                "factions.asmodian",
                IssueCause::MissingEnglishLabel
            )
        );
        let mut s = sample();
        s.regions[0].label.fr = Some(String::new());
        assert_eq!(
            causes(&s),
            one(ReferenceFile::Regions, "r1", IssueCause::BlankFrenchLabel)
        );
        let mut s = sample();
        s.regions[0].label.fr = Some("Région".into());
        assert_eq!(validate(&s), Ok(()));
    }

    #[test]
    fn cross_references_must_exist() {
        let mut s = sample();
        s.servers[1].region = "nowhere".into();
        s.classes[0].role = "nobody".into();
        assert_eq!(
            causes(&s),
            vec![
                (
                    ReferenceFile::Servers,
                    "s2".into(),
                    IssueCause::UnknownRegion("nowhere".into())
                ),
                (
                    ReferenceFile::Classes,
                    "c1".into(),
                    IssueCause::UnknownRole("nobody".into())
                ),
            ]
        );
    }

    #[test]
    fn disabled_entries_still_count_as_existing() {
        let mut s = sample();
        s.regions[0].enabled = false;
        s.roles[0].enabled = false;
        assert_eq!(validate(&s), Ok(()));
    }

    #[test]
    fn tag_rules() {
        let check = |tag: &str| {
            let mut s = sample();
            s.servers[0].tag = tag.into();
            causes(&s)
                .into_iter()
                .map(|(_, _, c)| c)
                .collect::<Vec<_>>()
        };
        assert_eq!(check(""), vec![IssueCause::BlankTag]);
        assert_eq!(check("   "), vec![IssueCause::BlankTag]);
        assert_eq!(check("[One]"), vec![IssueCause::TagBrackets]);
        assert_eq!(check("On]e"), vec![IssueCause::TagBrackets]);
        assert_eq!(check(&"é".repeat(TAG_MAX)), vec![]);
        assert_eq!(
            check(&"x".repeat(TAG_MAX + 1)),
            vec![IssueCause::TagTooLong]
        );
        assert_eq!(check("Two"), vec![IssueCause::DuplicateTag("Two".into())]);
    }

    #[test]
    fn same_tag_in_another_region_is_allowed() {
        let mut s = sample();
        s.regions.push(Region {
            id: "r2".into(),
            label: label("Region Two"),
            enabled: false,
        });
        s.servers[1].region = "r2".into();
        s.servers[1].tag = "One".into();
        assert_eq!(validate(&s), Ok(()));
    }

    #[test]
    fn instance_bounds() {
        let mut s = sample();
        s.instances[0].parties = 0;
        s.instances[0].party_size = 0;
        s.instances[0].duration_min = 0;
        assert_eq!(
            causes(&s)
                .into_iter()
                .map(|(_, _, c)| c)
                .collect::<Vec<_>>(),
            vec![
                IssueCause::NoParties,
                IssueCause::EmptyParty,
                IssueCause::ZeroDuration
            ]
        );
        let mut s = sample();
        s.instances[0].parties = 1;
        s.instances[0].party_size = 1;
        s.instances[0].duration_min = 1;
        assert_eq!(validate(&s), Ok(()));
    }

    #[test]
    fn room_label_follows_declared_charset_and_length() {
        let check = |room: &str| {
            let mut s = sample();
            s.instances[0].room_label = room.into();
            causes(&s)
                .into_iter()
                .map(|(_, _, c)| c)
                .collect::<Vec<_>>()
        };
        assert_eq!(check("CBA3"), vec![]);
        assert_eq!(check(""), vec![IssueCause::BlankRoomLabel]);
        assert_eq!(check("ABC12"), vec![IssueCause::RoomLabelTooLong(4)]);
        assert_eq!(check("ab"), vec![IssueCause::RoomLabelOutsideCharset('a')]);
        assert_eq!(check("A B"), vec![IssueCause::RoomLabelOutsideCharset(' ')]);

        let mut s = sample();
        s.room_label_charset = String::new();
        s.room_label_max = 0;
        assert_eq!(
            causes(&s),
            vec![
                (
                    ReferenceFile::Instances,
                    "room_label_charset".into(),
                    IssueCause::EmptyRoomLabelCharset
                ),
                (
                    ReferenceFile::Instances,
                    "room_label_max".into(),
                    IssueCause::ZeroRoomLabelMax
                ),
                (
                    ReferenceFile::Instances,
                    "i1".into(),
                    IssueCause::RoomLabelTooLong(0)
                ),
            ]
        );
    }

    #[test]
    fn class_bounds() {
        let mut s = sample();
        s.classes[0].default_min = 3;
        s.classes[0].default_max = 2;
        assert_eq!(
            causes(&s),
            one(ReferenceFile::Classes, "c1", IssueCause::MinAboveMax)
        );
        s.classes[0].default_max = 3;
        assert_eq!(validate(&s), Ok(()));
    }

    #[test]
    fn issues_display_file_entry_and_cause() {
        let mut s = sample();
        s.servers[0].region = "zz".into();
        let issue = &validate(&s).unwrap_err()[0];
        assert_eq!(issue.to_string(), "servers.toml: s1: unknown region `zz`");
    }

    #[test]
    fn enabled_defaults_to_true_and_unknown_fields_are_refused() {
        let role: Role = serde_json::from_str(r#"{"id":"ro1","label":{"en":"Role"}}"#).unwrap();
        assert!(role.enabled);
        assert_eq!(role.label.fr, None);
        assert!(
            serde_json::from_str::<Role>(r#"{"id":"ro1","label":{"en":"R"},"enabeld":false}"#)
                .is_err()
        );
        assert!(serde_json::from_str::<Label>(r#"{"en":"R","de":"R"}"#).is_err());
        assert!(serde_json::from_str::<Label>(r#"{"fr":"R"}"#).is_err());
        assert!(serde_json::from_str::<Faction>(r#""neutral""#).is_err());
    }
}
