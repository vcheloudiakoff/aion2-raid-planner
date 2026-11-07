-- Groupes et placements de l'epic 3 (AD-4 *Verrouillage* et *Placement*), au complet en une seule
-- migration : une migration poussée est immuable (AD-2). Instants au format unique
-- `domain::Timestamp` (20 caractères, AD-11), ids publics de 22 caractères (AD-3). Pas de table
-- `confirmations` : elle arrive avec l'epic 5.
--
-- Un groupe naît au verrouillage, apparié par `(slot_id, occurrence)`, et fige à sa création la
-- disposition de l'instance (`parties`, `party_size`, `room_label`) et son nom de salle (AD-18).
-- `retained` dit s'il fait partie du verrouillage courant : un groupe non réapparié au
-- re-verrouillage reste en sommeil (`retained = 0`), nom et placements gardés (décision de Vincent,
-- 2026-11-07). Supprimer le créneau, l'événement ou la purge sont les seules façons de le perdre.
--
-- Clés étrangères composites : le créneau d'un groupe, la candidature et le groupe d'un placement
-- sont toujours du même événement ; la base refuse une ligne qui mélangerait deux événements.

-- Cibles des clés composites : `(id, event_id)` est unique puisque `id` l'est.
CREATE UNIQUE INDEX slots_id_event ON slots (id, event_id);

CREATE UNIQUE INDEX candidatures_id_event ON candidatures (id, event_id);

CREATE TABLE groups (
    id TEXT PRIMARY KEY CHECK (length(id) = 22),
    event_id TEXT NOT NULL REFERENCES events (id) ON DELETE CASCADE,
    slot_id TEXT NOT NULL,
    occurrence INTEGER NOT NULL CHECK (occurrence >= 1),
    retained INTEGER NOT NULL CHECK (retained IN (0, 1)),
    parties INTEGER NOT NULL CHECK (parties >= 1),
    party_size INTEGER NOT NULL CHECK (party_size >= 1),
    room_label TEXT NOT NULL,
    room_name TEXT NOT NULL,
    created_at TEXT NOT NULL CHECK (length(created_at) = 20),
    UNIQUE (slot_id, occurrence),
    FOREIGN KEY (slot_id, event_id) REFERENCES slots (id, event_id) ON DELETE CASCADE
) STRICT;

CREATE UNIQUE INDEX groups_id_event ON groups (id, event_id);

-- Unique parmi les groupes non purgés : la purge supprime les lignes, l'index est donc global.
CREATE UNIQUE INDEX groups_room_name ON groups (room_name);

CREATE INDEX groups_event_id ON groups (event_id);

-- Une ligne par candidature placée, jamais plus (AD-4) : titulaire `(party, party_index)` ou banc
-- `bench_index`, l'un ou l'autre. Les bornes hautes (`parties`, `party_size` du groupe figé) sont
-- vérifiées par `apply_placement`, seul writer.
CREATE TABLE assignments (
    event_id TEXT NOT NULL,
    candidature_id TEXT NOT NULL,
    group_id TEXT NOT NULL,
    party INTEGER CHECK (party >= 1),
    party_index INTEGER CHECK (party_index >= 1),
    bench_index INTEGER CHECK (bench_index >= 1),
    CHECK ((party IS NULL) = (party_index IS NULL) AND (party IS NULL) <> (bench_index IS NULL)),
    FOREIGN KEY (candidature_id, event_id) REFERENCES candidatures (id, event_id)
        ON DELETE CASCADE,
    FOREIGN KEY (group_id, event_id) REFERENCES groups (id, event_id) ON DELETE CASCADE
) STRICT;

CREATE UNIQUE INDEX assignments_candidature_id ON assignments (candidature_id);

CREATE UNIQUE INDEX assignments_starter ON assignments (group_id, party, party_index);

CREATE UNIQUE INDEX assignments_bench ON assignments (group_id, bench_index);
