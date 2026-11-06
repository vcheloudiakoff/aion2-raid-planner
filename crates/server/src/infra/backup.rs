//! Sauvegardes de la base (AD-2, AD-14) : `VACUUM INTO` dans `/data/backups/`, rotation et
//! restauration, sans `sqlite3` ni shell (l'image est `scratch`).
//!
//! Noms : `daily-YYYYMMDDTHHMMSSZ.db` et `pre-deploy-YYYYMMDDTHHMMSSZ.db`, en UTC. Une sauvegarde est
//! écrite dans `<nom>.tmp` puis renommée : un fichier `.db` de ce dossier est toujours complet.

use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

use chrono::NaiveDateTime;
use domain::Timestamp;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool};
use sqlx::{ConnectOptions, Connection};

/// Dossier fixe des sauvegardes dans le conteneur.
pub const BACKUP_DIR: &str = "/data/backups";

/// Nombre de sauvegardes quotidiennes conservées.
pub const DAILY_KEPT: usize = 2;
/// Âge à partir duquel une sauvegarde quotidienne est due.
pub const DAILY_EVERY_SECS: i64 = 24 * 3600;
/// Âge au-delà duquel une sauvegarde `pre-deploy` (ou un `.tmp` abandonné) est supprimée.
pub const PRE_DEPLOY_MAX_AGE_SECS: i64 = 2 * 24 * 3600;

const STAMP_FORMAT: &str = "%Y%m%dT%H%M%SZ";
const STAMP_LEN: usize = "YYYYMMDDTHHMMSSZ".len();
const EXT: &str = ".db";
const TMP_EXT: &str = ".tmp";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Daily,
    PreDeploy,
}

impl Kind {
    pub fn prefix(self) -> &'static str {
        match self {
            Kind::Daily => "daily",
            Kind::PreDeploy => "pre-deploy",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        [Kind::Daily, Kind::PreDeploy]
            .into_iter()
            .find(|k| k.prefix() == s)
    }
}

/// Une sauvegarde reconnue par son nom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Backup {
    pub name: String,
    pub kind: Kind,
    pub at: Timestamp,
}

#[derive(Debug, thiserror::Error)]
pub enum BackupError {
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("i/o error: {0}")]
    Io(#[from] io::Error),
    #[error("unknown backup: {0}")]
    Unknown(String),
    #[error("backup {name} failed the integrity check: {detail}")]
    Corrupt { name: String, detail: String },
}

/// Nom de fichier d'une sauvegarde.
pub fn file_name(kind: Kind, at: Timestamp) -> String {
    format!(
        "{}-{}{EXT}",
        kind.prefix(),
        at.as_datetime().format(STAMP_FORMAT)
    )
}

/// Reconnaît un nom `<préfixe>-<horodatage>.db` ; tout autre nom (chemin, `.tmp`...) vaut `None`.
pub fn parse_name(name: &str) -> Option<Backup> {
    let stem = name.strip_suffix(EXT)?;
    let (prefix, stamp) = stem.split_at_checked(stem.len().checked_sub(STAMP_LEN + 1)?)?;
    let stamp = stamp.strip_prefix('-')?;
    let kind = Kind::parse(prefix)?;
    let at = NaiveDateTime::parse_from_str(stamp, STAMP_FORMAT).ok()?;
    let at = Timestamp::from_datetime(at.and_utc()).ok()?;
    // Forme canonique seulement : refuse ce que chrono tolère (chiffres en trop, espaces...).
    (file_name(kind, at) == name).then(|| Backup {
        name: name.to_owned(),
        kind,
        at,
    })
}

/// Sauvegardes présentes dans `dir`, triées par date croissante. Dossier absent = liste vide.
pub fn list(dir: &Path) -> io::Result<Vec<Backup>> {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut out = Vec::new();
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        if let Some(b) = entry.file_name().to_str().and_then(parse_name) {
            out.push(b);
        }
    }
    out.sort_by(|a, b| a.at.cmp(&b.at).then_with(|| a.name.cmp(&b.name)));
    Ok(out)
}

/// Vrai si aucune sauvegarde quotidienne n'a moins de 24 h.
pub fn daily_due(dir: &Path, now: Timestamp) -> io::Result<bool> {
    let latest = list(dir)?
        .into_iter()
        .filter(|b| b.kind == Kind::Daily)
        .map(|b| b.at)
        .max();
    Ok(match latest {
        None => true,
        Some(at) => now.unix_seconds() - at.unix_seconds() >= DAILY_EVERY_SECS,
    })
}

/// Rotation : garde les deux `daily` les plus récentes, supprime les `pre-deploy` et les `.tmp`
/// abandonnés de plus de deux jours. Renvoie les noms supprimés.
pub fn prune(dir: &Path, now: Timestamp) -> io::Result<Vec<String>> {
    let too_old = |at: Timestamp| now.unix_seconds() - at.unix_seconds() > PRE_DEPLOY_MAX_AGE_SECS;
    let mut removed = Vec::new();

    let backups = list(dir)?;
    let dailies: Vec<&Backup> = backups.iter().filter(|b| b.kind == Kind::Daily).collect();
    let extra_dailies = dailies.len().saturating_sub(DAILY_KEPT);
    let doomed = dailies.into_iter().take(extra_dailies).chain(
        backups
            .iter()
            .filter(|b| b.kind == Kind::PreDeploy && too_old(b.at)),
    );
    for b in doomed {
        remove_if_present(&dir.join(&b.name))?;
        removed.push(b.name.clone());
    }

    // Un `.tmp` n'est supprimé que bien après sa création : jamais celui qu'un autre processus
    // (`backup pre-deploy` lancé par le déploiement) est en train d'écrire.
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(removed),
        Err(e) => return Err(e),
    };
    for entry in entries {
        let entry = entry?;
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if let Some(b) = name.strip_suffix(TMP_EXT).and_then(parse_name)
            && too_old(b.at)
        {
            remove_if_present(&entry.path())?;
            removed.push(name);
        }
    }
    Ok(removed)
}

/// `VACUUM INTO` vers `dir/<nom>.tmp`, `fsync`, puis renommage en `dir/<nom>`. Renvoie le nom.
pub async fn vacuum_into(
    pool: &SqlitePool,
    dir: &Path,
    kind: Kind,
    now: Timestamp,
) -> Result<String, BackupError> {
    let name = file_name(kind, now);
    let target = dir.join(&name);
    let tmp = dir.join(format!("{name}{TMP_EXT}"));
    // `VACUUM INTO` refuse un fichier cible non vide : reste d'un essai interrompu.
    remove_if_present(&tmp)?;
    let tmp_str = tmp
        .to_str()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "non UTF-8 backup path"))?
        .to_owned();
    let result = sqlx::query("VACUUM INTO ?")
        .bind(&tmp_str)
        .execute(pool)
        .await;
    if let Err(e) = result {
        let _ = fs::remove_file(&tmp);
        return Err(e.into());
    }
    let dir = dir.to_owned();
    tokio::task::spawn_blocking(move || -> io::Result<()> {
        File::open(&tmp)?.sync_all()?;
        fs::rename(&tmp, &target)?;
        sync_dir(&dir)
    })
    .await
    .map_err(io::Error::other)??;
    Ok(name)
}

/// Restaure `dir/<name>` sur `db_path`, service arrêté : contrôle d'intégrité de la sauvegarde,
/// copie vers `<db_path>.tmp` et `fsync`, suppression de `-wal` et `-shm`, renommage atomique.
/// Nom inconnu ou fichier corrompu : erreur, `db_path` et ses fichiers intacts.
pub async fn restore(dir: &Path, name: &str, db_path: &Path) -> Result<(), BackupError> {
    // Seuls les noms listés sont acceptés : ni chemin, ni `..`, ni fichier hors du dossier.
    let backup = list(dir)?
        .into_iter()
        .find(|b| b.name == name)
        .ok_or_else(|| BackupError::Unknown(name.to_owned()))?;
    let source = dir.join(&backup.name);

    integrity_check(&source)
        .await
        .map_err(|detail| BackupError::Corrupt {
            name: backup.name.clone(),
            detail,
        })?;

    let db_path = db_path.to_owned();
    tokio::task::spawn_blocking(move || -> io::Result<()> {
        let tmp = with_suffix(&db_path, ".tmp");
        // Copie d'abord : un disque plein laisse la base en place et son WAL intacts.
        fs::copy(&source, &tmp)?;
        File::open(&tmp)?.sync_all()?;
        remove_if_present(&with_suffix(&db_path, "-wal"))?;
        remove_if_present(&with_suffix(&db_path, "-shm"))?;
        fs::rename(&tmp, &db_path)?;
        sync_dir(db_path.parent().unwrap_or(Path::new(".")))
    })
    .await
    .map_err(io::Error::other)??;
    Ok(())
}

/// En-tête SQLite puis `PRAGMA integrity_check` en lecture seule `immutable` : n'écrit ni `-wal` ni
/// `-shm` à côté de la sauvegarde.
async fn integrity_check(path: &Path) -> Result<(), String> {
    // SQLite ouvre comme base vide un fichier trop court : l'en-tête est exigé avant tout.
    let mut header = [0u8; 16];
    File::open(path)
        .and_then(|mut f| io::Read::read_exact(&mut f, &mut header))
        .map_err(|e| format!("cannot read header: {e}"))?;
    if &header != b"SQLite format 3\0" {
        return Err("not an SQLite database".to_owned());
    }
    let mut conn = SqliteConnectOptions::new()
        .filename(path)
        .read_only(true)
        .immutable(true)
        .connect()
        .await
        .map_err(|e| e.to_string())?;
    let rows: Result<Vec<(String,)>, _> = sqlx::query_as("PRAGMA integrity_check")
        .fetch_all(&mut conn)
        .await;
    let _ = conn.close().await;
    let rows = rows.map_err(|e| e.to_string())?;
    match rows.as_slice() {
        [(ok,)] if ok == "ok" => Ok(()),
        [] => Err("no result".to_owned()),
        _ => Err(rows
            .into_iter()
            .map(|(r,)| r)
            .collect::<Vec<_>>()
            .join("; ")),
    }
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(suffix);
    PathBuf::from(s)
}

fn remove_if_present(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// Rend durable un renommage dans `dir`.
fn sync_dir(dir: &Path) -> io::Result<()> {
    #[cfg(unix)]
    File::open(dir)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = dir;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::sqlite::{self, Db};

    fn ts(s: &str) -> Timestamp {
        s.parse().unwrap()
    }

    fn touch(dir: &Path, name: &str) {
        fs::write(dir.join(name), b"x").unwrap();
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        v.sort();
        v
    }

    async fn db_with_rows(path: &Path, rows: &[&str]) -> Db {
        let db = Db::open(sqlite::file_options(path)).await.unwrap();
        sqlx::query("CREATE TABLE IF NOT EXISTS t (v TEXT NOT NULL)")
            .execute(&db.write)
            .await
            .unwrap();
        for r in rows {
            sqlx::query("INSERT INTO t (v) VALUES (?)")
                .bind(*r)
                .execute(&db.write)
                .await
                .unwrap();
        }
        db
    }

    async fn rows_of(path: &Path) -> Vec<String> {
        let db = Db::open(sqlite::file_options(path)).await.unwrap();
        let rows: Vec<(String,)> = sqlx::query_as("SELECT v FROM t ORDER BY rowid")
            .fetch_all(&db.read)
            .await
            .unwrap();
        db.close().await;
        rows.into_iter().map(|(v,)| v).collect()
    }

    #[test]
    fn names_round_trip_and_reject_anything_else() {
        let at = ts("2026-11-06T03:04:05Z");
        assert_eq!(file_name(Kind::Daily, at), "daily-20261106T030405Z.db");
        assert_eq!(
            file_name(Kind::PreDeploy, at),
            "pre-deploy-20261106T030405Z.db"
        );
        let b = parse_name("pre-deploy-20261106T030405Z.db").unwrap();
        assert_eq!((b.kind, b.at), (Kind::PreDeploy, at));
        for bad in [
            "daily-20261106T030405Z.db.tmp",
            "daily-20261106T030405Z",
            "weekly-20261106T030405Z.db",
            "../daily-20261106T030405Z.db",
            "daily-2026116T030405Z.db",
            "daily-20261306T030405Z.db",
            "daily20261106T030405Z.db",
            "app.db",
            "",
        ] {
            assert!(parse_name(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn daily_is_due_when_absent_or_older_than_a_day() {
        let dir = tempfile::tempdir().unwrap();
        let now = ts("2026-11-06T12:00:00Z");
        assert!(daily_due(dir.path(), now).unwrap());
        // Une `pre-deploy` récente ne compte pas.
        touch(dir.path(), "pre-deploy-20261106T110000Z.db");
        assert!(daily_due(dir.path(), now).unwrap());
        touch(dir.path(), "daily-20261105T120001Z.db");
        assert!(!daily_due(dir.path(), now).unwrap());
        assert!(daily_due(dir.path(), ts("2026-11-06T12:00:01Z")).unwrap());
        // Dossier absent : due, sans erreur.
        assert!(daily_due(&dir.path().join("missing"), now).unwrap());
    }

    #[test]
    fn prune_keeps_two_dailies_and_recent_pre_deploys() {
        let dir = tempfile::tempdir().unwrap();
        for n in [
            "daily-20261101T000000Z.db",
            "daily-20261104T000000Z.db",
            "daily-20261105T000000Z.db",
            "pre-deploy-20261103T235959Z.db",
            "pre-deploy-20261104T000000Z.db",
            "pre-deploy-20261106T000000Z.db",
            "daily-20261101T000000Z.db.tmp",
            "daily-20261106T000000Z.db.tmp",
            "app.db",
            "notes.txt",
        ] {
            touch(dir.path(), n);
        }
        let mut removed = prune(dir.path(), ts("2026-11-06T00:00:00Z")).unwrap();
        removed.sort();
        assert_eq!(
            removed,
            [
                "daily-20261101T000000Z.db",
                "daily-20261101T000000Z.db.tmp",
                "pre-deploy-20261103T235959Z.db",
            ]
        );
        assert_eq!(
            names(dir.path()),
            [
                "app.db",
                "daily-20261104T000000Z.db",
                "daily-20261105T000000Z.db",
                "daily-20261106T000000Z.db.tmp",
                "notes.txt",
                "pre-deploy-20261104T000000Z.db",
                "pre-deploy-20261106T000000Z.db",
            ]
        );
    }

    #[tokio::test]
    async fn vacuum_into_then_restore_brings_data_back() {
        let tmp = tempfile::tempdir().unwrap();
        let backups = tmp.path().join("backups");
        fs::create_dir(&backups).unwrap();
        let db_path = tmp.path().join("app.db");
        let db = db_with_rows(&db_path, &["before"]).await;

        let at = ts("2026-11-06T10:00:00Z");
        let name = vacuum_into(&db.read, &backups, Kind::PreDeploy, at)
            .await
            .unwrap();
        assert_eq!(name, "pre-deploy-20261106T100000Z.db");
        assert_eq!(
            names(&backups),
            std::slice::from_ref(&name),
            "no .tmp left behind"
        );

        sqlx::query("INSERT INTO t (v) VALUES ('after')")
            .execute(&db.write)
            .await
            .unwrap();
        db.close().await;
        // Simule un arrêt brutal : des fichiers `-wal` et `-shm` restent à côté de la base.
        fs::write(with_suffix(&db_path, "-wal"), b"stale").unwrap();
        fs::write(with_suffix(&db_path, "-shm"), b"stale").unwrap();

        restore(&backups, &name, &db_path).await.unwrap();
        assert!(!with_suffix(&db_path, "-wal").exists());
        assert!(!with_suffix(&db_path, "-shm").exists());
        assert!(!with_suffix(&db_path, ".tmp").exists());
        assert_eq!(rows_of(&db_path).await, ["before"]);
        // La sauvegarde reste en place, sans fichier annexe.
        assert_eq!(names(&backups), [name]);
    }

    #[tokio::test]
    async fn restore_refuses_unknown_names_and_corrupt_files() {
        let tmp = tempfile::tempdir().unwrap();
        let backups = tmp.path().join("backups");
        fs::create_dir(&backups).unwrap();
        let db_path = tmp.path().join("app.db");
        // Base ouverte : son `-wal` existe et porte la ligne.
        let live = db_with_rows(&db_path, &["live"]).await;
        assert!(with_suffix(&db_path, "-wal").exists());

        // Fichier qui n'est pas une base SQLite.
        touch(&backups, "daily-20261106T000000Z.db");
        // Base valide dont une page est écrasée.
        let damaged = backups.join("daily-20261106T010000Z.db");
        {
            let big = "x".repeat(200);
            let rows = vec![big.as_str(); 200];
            let db = db_with_rows(&tmp.path().join("src.db"), &rows).await;
            vacuum_into(&db.read, &backups, Kind::Daily, ts("2026-11-06T01:00:00Z"))
                .await
                .unwrap();
            db.close().await;
            let mut bytes = fs::read(&damaged).unwrap();
            let page = 4096;
            assert!(bytes.len() > 2 * page);
            bytes[page..2 * page].fill(0xA5);
            fs::write(&damaged, bytes).unwrap();
        }
        let unlisted = tmp.path().join("pre-deploy-20261106T000000Z.db");
        fs::copy(&db_path, &unlisted).unwrap();

        for (name, unknown) in [
            ("daily-20261106T000000Z.db", false),
            ("daily-20261106T010000Z.db", false),
            ("pre-deploy-20261106T000000Z.db", true),
            ("../pre-deploy-20261106T000000Z.db", true),
            ("app.db", true),
        ] {
            let err = restore(&backups, name, &db_path).await.unwrap_err();
            match err {
                BackupError::Unknown(_) => assert!(unknown, "{name}: {err}"),
                BackupError::Corrupt { .. } => assert!(!unknown, "{name}: {err}"),
                other => panic!("{name}: {other}"),
            }
        }
        assert!(with_suffix(&db_path, "-wal").exists(), "-wal untouched");
        assert!(!with_suffix(&db_path, ".tmp").exists());
        live.close().await;
        assert_eq!(rows_of(&db_path).await, ["live"], "app.db untouched");
        assert!(!backups.join("daily-20261106T000000Z.db-shm").exists());
    }
}
