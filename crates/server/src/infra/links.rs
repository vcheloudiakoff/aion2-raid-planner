//! Seul SQL des liens secrets (AD-3). La base ne voit que `SHA-256(secret)` : une fuite de la base ne
//! donne aucun droit. Rien ici ne journalise la valeur ni le hash (AD-15).

use std::fmt;

use sha2::{Digest, Sha256};
use sqlx::SqliteConnection;
use sqlx::sqlite::SqlitePool;

use crate::infra::ids::Secret;

/// Longueur du hash stocké, en octets.
pub const HASH_LEN: usize = 32;

/// Droit porté par un lien : `/o/` organisateur, `/j/` candidature directe, `/p/` participant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Right {
    Organizer,
    Join,
    Participant,
}

impl Right {
    pub const ALL: [Right; 3] = [Right::Organizer, Right::Join, Right::Participant];

    /// Valeur de la colonne `link_right`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Right::Organizer => "organizer",
            Right::Join => "join",
            Right::Participant => "participant",
        }
    }
}

impl fmt::Display for Right {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, thiserror::Error)]
#[error("unknown link right: {0}")]
pub struct UnknownRight(String);

impl TryFrom<&str> for Right {
    type Error = UnknownRight;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Right::ALL
            .into_iter()
            .find(|r| r.as_str() == s)
            .ok_or_else(|| UnknownRight(s.to_owned()))
    }
}

/// `SHA-256` de la valeur en clair : la seule forme sous laquelle un secret entre en base.
pub fn hash(secret: &str) -> [u8; HASH_LEN] {
    Sha256::digest(secret.as_bytes()).into()
}

/// Ce qu'un secret autorise : un droit et sa portée. `candidature_id` existe si et seulement si le
/// droit est `Participant` (contrainte de la table).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkRow {
    pub right: Right,
    pub event_id: String,
    pub candidature_id: Option<String>,
}

/// Enregistre un lien par son hash. À appeler dans la transaction de la commande qui crée l'objet.
pub async fn insert(
    conn: &mut SqliteConnection,
    secret: &Secret,
    row: &LinkRow,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO links (secret_hash, link_right, event_id, candidature_id) VALUES (?, ?, ?, ?)",
    )
    .bind(hash(secret.expose()).as_slice())
    .bind(row.right.as_str())
    .bind(&row.event_id)
    .bind(&row.candidature_id)
    .execute(conn)
    .await?;
    Ok(())
}

/// Lien dont le hash vaut `secret_hash`, quel que soit son droit. Prend le pool de lecture.
pub async fn find(
    pool: &SqlitePool,
    secret_hash: &[u8; HASH_LEN],
) -> Result<Option<LinkRow>, sqlx::Error> {
    let row: Option<(String, String, Option<String>)> = sqlx::query_as(
        "SELECT link_right, event_id, candidature_id FROM links WHERE secret_hash = ?",
    )
    .bind(secret_hash.as_slice())
    .fetch_optional(pool)
    .await?;
    row.map(|(right, event_id, candidature_id)| {
        let right = Right::try_from(right.as_str()).map_err(|e| sqlx::Error::Decode(e.into()))?;
        Ok(LinkRow {
            right,
            event_id,
            candidature_id,
        })
    })
    .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::ids::{DeterministicIdGenerator, IdGenerator};
    use crate::infra::sqlite::{Db, shared_memory_options};

    async fn db(name: &str) -> Db {
        let db = Db::open(shared_memory_options(&format!("infra_links_{name}")))
            .await
            .unwrap();
        db.migrate().await.unwrap();
        db
    }

    fn row(right: Right, ids: &DeterministicIdGenerator) -> LinkRow {
        LinkRow {
            right,
            event_id: ids.public_id().as_str().to_owned(),
            candidature_id: (right == Right::Participant).then(|| ids.public_id().to_string()),
        }
    }

    #[test]
    fn right_round_trips() {
        for r in Right::ALL {
            assert_eq!(Right::try_from(r.as_str()).unwrap(), r);
        }
        assert!(Right::try_from("admin").is_err());
    }

    #[test]
    fn hash_is_sha256() {
        // Vecteur connu : SHA-256("abc").
        assert_eq!(
            data_encoding::HEXLOWER.encode(&hash("abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[tokio::test]
    async fn stores_the_hash_never_the_secret() {
        let db = db("stores_hash").await;
        let ids = DeterministicIdGenerator::new();
        let secret = ids.secret();
        let link = row(Right::Participant, &ids);
        let mut conn = db.write.acquire().await.unwrap();
        insert(&mut conn, &secret, &link).await.unwrap();
        drop(conn);

        let rows: Vec<(Vec<u8>, String, String, Option<String>)> =
            sqlx::query_as("SELECT secret_hash, link_right, event_id, candidature_id FROM links")
                .fetch_all(&db.read)
                .await
                .unwrap();
        assert_eq!(rows.len(), 1);
        let (stored, right, event_id, candidature_id) = &rows[0];
        assert_eq!(stored.len(), HASH_LEN);
        assert_eq!(stored.as_slice(), hash(secret.expose()).as_slice());
        let plain = secret.expose().as_bytes();
        assert!(!stored.windows(plain.len()).any(|w| w == plain));
        for text in [right, event_id].into_iter().chain(candidature_id) {
            assert!(!text.contains(secret.expose()));
        }

        assert_eq!(
            find(&db.read, &hash(secret.expose())).await.unwrap(),
            Some(link)
        );
        assert_eq!(find(&db.read, &hash("unknown")).await.unwrap(), None);
    }

    #[tokio::test]
    async fn table_rejects_inconsistent_rows() {
        let db = db("constraints").await;
        let ids = DeterministicIdGenerator::new();
        let mut conn = db.write.acquire().await.unwrap();
        let mut organizer_with_candidature = row(Right::Organizer, &ids);
        organizer_with_candidature.candidature_id = Some(ids.public_id().to_string());
        let mut participant_without = row(Right::Participant, &ids);
        participant_without.candidature_id = None;
        let mut short_event = row(Right::Join, &ids);
        short_event.event_id = "short".into();
        for bad in [organizer_with_candidature, participant_without, short_event] {
            assert!(
                insert(&mut conn, &ids.secret(), &bad).await.is_err(),
                "{bad:?}"
            );
        }
        let secret = ids.secret();
        insert(&mut conn, &secret, &row(Right::Join, &ids))
            .await
            .unwrap();
        assert!(
            insert(&mut conn, &secret, &row(Right::Join, &ids))
                .await
                .is_err(),
            "duplicate hash"
        );
    }
}
