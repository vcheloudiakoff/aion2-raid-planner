-- Liens secrets (AD-3) : seul `SHA-256(secret)` est stocké, jamais la valeur en clair. Une table
-- unique pour les trois droits : une seule recherche par hash distingue « autre droit » (403) de
-- « inconnu » (404). Pas de clé étrangère : les tables métier n'existent pas encore ; supprimer un
-- événement supprimera ses liens dans la même transaction, d'où l'index sur `event_id`.
CREATE TABLE links (
    secret_hash BLOB PRIMARY KEY CHECK (length(secret_hash) = 32),
    link_right TEXT NOT NULL CHECK (link_right IN ('organizer', 'join', 'participant')),
    event_id TEXT NOT NULL CHECK (length(event_id) = 22),
    candidature_id TEXT CHECK (length(candidature_id) = 22),
    CHECK ((link_right = 'participant') = (candidature_id IS NOT NULL))
) STRICT, WITHOUT ROWID;

CREATE INDEX links_event_id ON links (event_id);
