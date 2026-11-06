//! Lecture des cinq fichiers de référence (AD-6) : I/O seulement, les règles vivent dans
//! `domain::reference::validate`.
//!
//! Le serveur ne fait que lire `/data/reference/`, jamais écrire. Le jeu embarqué dans le binaire ne
//! sert que si le dossier ne contient aucun des cinq fichiers.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::SystemTime;

use domain::reference::{
    ClassesFile, InstancesFile, ReferenceFile, ReferenceIssue, ReferenceSet, RegionsFile,
    RolesFile, ServersFile, validate,
};
use serde::de::DeserializeOwned;

/// Dossier fixe dans le conteneur ; non configurable.
pub const REFERENCE_DIR: &str = "/data/reference";

/// Copie de `reference/` au moment du build, dans l'ordre de `ReferenceFile::ALL`.
pub const EMBEDDED: [&str; 5] = [
    include_str!("../../../../reference/regions.toml"),
    include_str!("../../../../reference/servers.toml"),
    include_str!("../../../../reference/instances.toml"),
    include_str!("../../../../reference/classes.toml"),
    include_str!("../../../../reference/roles.toml"),
];

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("no reference file found")]
    Empty,
    #[error("reference files missing: {}", .0.iter().map(|f| f.file_name()).collect::<Vec<_>>().join(", "))]
    Missing(Vec<ReferenceFile>),
    #[error("{} problem(s) in reference files", .0.len())]
    Invalid(Vec<Problem>),
}

impl LoadError {
    /// Une ligne par problème, pour `stderr` et `tracing`.
    pub fn problems(&self) -> Vec<Problem> {
        match self {
            Self::Empty => vec![Problem::whole("no reference file found")],
            Self::Missing(files) => files
                .iter()
                .map(|&f| Problem {
                    file: Some(f),
                    entry: String::new(),
                    cause: "file is missing".to_owned(),
                })
                .collect(),
            Self::Invalid(problems) => problems.clone(),
        }
    }
}

/// Un problème lisible : fichier, entrée (vide si sans objet), cause.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub file: Option<ReferenceFile>,
    pub entry: String,
    pub cause: String,
}

impl Problem {
    fn whole(cause: &str) -> Self {
        Self {
            file: None,
            entry: String::new(),
            cause: cause.to_owned(),
        }
    }

    pub fn file_name(&self) -> &'static str {
        self.file.map(ReferenceFile::file_name).unwrap_or("")
    }
}

impl From<ReferenceIssue> for Problem {
    fn from(i: ReferenceIssue) -> Self {
        Self {
            file: Some(i.file),
            entry: i.entry,
            cause: i.cause.to_string(),
        }
    }
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.file, self.entry.is_empty()) {
            (None, _) => f.write_str(&self.cause),
            (Some(file), true) => write!(f, "{file}: {}", self.cause),
            (Some(file), false) => write!(f, "{file}: {}: {}", self.entry, self.cause),
        }
    }
}

/// Analyse et valide cinq textes TOML (ordre de `ReferenceFile::ALL`) ; tous les problèmes à la fois.
pub fn parse(texts: [&str; 5]) -> Result<ReferenceSet, LoadError> {
    let [regions, servers, instances, classes, roles] = texts;
    let mut problems = Vec::new();
    let regions = parse_file::<RegionsFile>(ReferenceFile::Regions, regions, &mut problems);
    let servers = parse_file::<ServersFile>(ReferenceFile::Servers, servers, &mut problems);
    let instances = parse_file::<InstancesFile>(ReferenceFile::Instances, instances, &mut problems);
    let classes = parse_file::<ClassesFile>(ReferenceFile::Classes, classes, &mut problems);
    let roles = parse_file::<RolesFile>(ReferenceFile::Roles, roles, &mut problems);
    let (Some(regions), Some(servers), Some(instances), Some(classes), Some(roles)) =
        (regions, servers, instances, classes, roles)
    else {
        return Err(LoadError::Invalid(problems));
    };
    let set = ReferenceSet::from_files(regions, servers, instances, classes, roles);
    validate(&set)
        .map_err(|issues| LoadError::Invalid(issues.into_iter().map(Problem::from).collect()))?;
    Ok(set)
}

fn parse_file<T: DeserializeOwned>(
    file: ReferenceFile,
    text: &str,
    problems: &mut Vec<Problem>,
) -> Option<T> {
    match toml::from_str(text) {
        Ok(v) => Some(v),
        Err(e) => {
            let entry = e
                .span()
                .map(|span| {
                    let before = &text[..span.start.min(text.len())];
                    let line = before.matches('\n').count() + 1;
                    let column = before.rsplit('\n').next().unwrap_or("").chars().count() + 1;
                    format!("line {line}, column {column}")
                })
                .unwrap_or_default();
            problems.push(Problem {
                file: Some(file),
                entry,
                cause: e.message().trim().to_owned(),
            });
            None
        }
    }
}

/// Le jeu embarqué, validé comme les autres.
pub fn embedded() -> Result<ReferenceSet, LoadError> {
    parse(EMBEDDED)
}

/// Lit les cinq fichiers de `dir`, sans repli : dossier vide = `Empty`, partiel = `Missing`.
pub fn load_dir(dir: &Path) -> Result<ReferenceSet, LoadError> {
    let mut texts: Vec<String> = Vec::with_capacity(5);
    let mut missing = Vec::new();
    let mut problems = Vec::new();
    for file in ReferenceFile::ALL {
        match std::fs::read_to_string(dir.join(file.file_name())) {
            Ok(t) => texts.push(t),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                missing.push(file);
                texts.push(String::new());
            }
            Err(e) => {
                problems.push(Problem {
                    file: Some(file),
                    entry: String::new(),
                    cause: format!("cannot read: {e}"),
                });
                texts.push(String::new());
            }
        }
    }
    if missing.len() == ReferenceFile::ALL.len() {
        return Err(LoadError::Empty);
    }
    if !missing.is_empty() {
        return Err(LoadError::Missing(missing));
    }
    if !problems.is_empty() {
        return Err(LoadError::Invalid(problems));
    }
    parse(std::array::from_fn(|i| texts[i].as_str()))
}

/// État d'un fichier pour décider d'un rechargement : mtime et taille, lus avant le contenu.
#[derive(Debug, Clone, PartialEq, Eq)]
enum FileState {
    Missing,
    Present {
        modified: Option<SystemTime>,
        len: u64,
    },
    Unreadable(io::ErrorKind),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Fingerprint([FileState; 5]);

impl Fingerprint {
    fn take(dir: &Path) -> Self {
        Self(
            ReferenceFile::ALL.map(|f| match std::fs::metadata(dir.join(f.file_name())) {
                Ok(m) => FileState::Present {
                    modified: m.modified().ok(),
                    len: m.len(),
                },
                Err(e) if e.kind() == io::ErrorKind::NotFound => FileState::Missing,
                Err(e) => FileState::Unreadable(e.kind()),
            }),
        )
    }

    fn is_empty(&self) -> bool {
        self.0.iter().all(|s| *s == FileState::Missing)
    }
}

/// Le jeu courant, partagé par toutes les requêtes. Chaque requête prend l'`Arc` une fois.
#[derive(Debug, Clone, Default)]
pub struct ReferenceStore(Arc<RwLock<Option<Arc<ReferenceSet>>>>);

impl ReferenceStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Magasin déjà chargé, pour les tests et les outils.
    pub fn with(set: ReferenceSet) -> Self {
        let store = Self::new();
        store.replace(Arc::new(set));
        store
    }

    pub fn get(&self) -> Option<Arc<ReferenceSet>> {
        self.0.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Remplacement atomique : une requête voit l'ancien jeu ou le nouveau, jamais un mélange.
    pub fn replace(&self, set: Arc<ReferenceSet>) {
        *self.0.write().unwrap_or_else(|e| e.into_inner()) = Some(set);
    }
}

/// Résultat d'un `poll`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PollOutcome {
    /// Empreinte identique au dernier chargement réussi : rien lu.
    Unchanged,
    /// Nouveau jeu en service.
    Loaded,
    /// Ancien jeu conservé ; `logged` est faux si cet état d'échec était déjà journalisé.
    Failed { logged: bool },
}

/// Chargeur : compare l'empreinte du dossier et recharge quand elle change.
#[derive(Debug)]
pub struct Loader {
    dir: PathBuf,
    store: ReferenceStore,
    /// Empreinte du dernier chargement réussi seulement.
    loaded: Option<Fingerprint>,
    /// Dernier état d'échec journalisé, pour ne pas répéter la même erreur à chaque tick.
    failed: Option<(Fingerprint, Vec<Problem>)>,
}

impl Loader {
    pub fn new(dir: impl Into<PathBuf>, store: ReferenceStore) -> Self {
        Self {
            dir: dir.into(),
            store,
            loaded: None,
            failed: None,
        }
    }

    pub fn store(&self) -> &ReferenceStore {
        &self.store
    }

    /// Un tick : bloquant (lecture de fichiers), à lancer hors du runtime asynchrone.
    pub fn poll(&mut self) -> PollOutcome {
        // Empreinte avant lecture : une écriture concurrente change le mtime et sera relue au tick
        // suivant.
        let fingerprint = Fingerprint::take(&self.dir);
        if self.loaded.as_ref() == Some(&fingerprint) {
            return PollOutcome::Unchanged;
        }
        let (result, source) = if fingerprint.is_empty() {
            (embedded(), "embedded")
        } else {
            (load_dir(&self.dir), "directory")
        };
        match result {
            Ok(set) => {
                self.store.replace(Arc::new(set));
                self.loaded = Some(fingerprint);
                self.failed = None;
                tracing::info!(source, "reference loaded");
                PollOutcome::Loaded
            }
            Err(e) => {
                let problems = e.problems();
                let state = (fingerprint, problems);
                let logged = self.failed.as_ref() != Some(&state);
                if logged {
                    for p in &state.1 {
                        tracing::error!(
                            file = p.file_name(),
                            entry = %p.entry,
                            cause = %p.cause,
                            "reference rejected, previous set kept"
                        );
                    }
                }
                self.failed = Some(state);
                PollOutcome::Failed { logged }
            }
        }
    }
}
