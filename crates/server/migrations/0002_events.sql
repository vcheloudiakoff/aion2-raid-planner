-- Modèle complet de l'epic 2 (AD-4), en une seule migration : une migration poussée est immuable
-- (AD-2). Instants au format unique `domain::Timestamp` (20 caractères, AD-11), ids publics de 22
-- caractères (AD-3). Pas de `CHECK` sur le pseudo ni sur les bornes du CP : ces règles vivent dans
-- `domain::validate` et peuvent encore bouger. `links` reste sans clé étrangère (0001) : la commande
-- qui supprime un événement supprime ses liens dans la même transaction.

CREATE TABLE events (
    id TEXT PRIMARY KEY CHECK (length(id) = 22),
    instance TEXT NOT NULL,
    region TEXT NOT NULL,
    faction_scope TEXT NOT NULL CHECK (faction_scope IN ('all', 'elyos', 'asmodian')),
    visibility TEXT NOT NULL CHECK (visibility IN ('listed', 'unlisted')),
    status TEXT NOT NULL CHECK (status IN ('open', 'locked', 'cancelled')),
    min_cp INTEGER,
    discord_url TEXT,
    note TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL CHECK (length(created_at) = 20),
    cancelled_at TEXT CHECK (length(cancelled_at) = 20),
    admin_unlisted_at TEXT CHECK (length(admin_unlisted_at) = 20)
) STRICT;

CREATE TABLE slots (
    id TEXT PRIMARY KEY CHECK (length(id) = 22),
    event_id TEXT NOT NULL REFERENCES events (id) ON DELETE CASCADE,
    starts_at TEXT NOT NULL CHECK (length(starts_at) = 20),
    duration_min INTEGER NOT NULL CHECK (duration_min > 0)
) STRICT;

CREATE INDEX slots_event_id ON slots (event_id);

-- Une ligne par classe active à la création : un changement ultérieur de la référence ne touche pas
-- les bornes d'un événement existant.
CREATE TABLE class_bounds (
    event_id TEXT NOT NULL REFERENCES events (id) ON DELETE CASCADE,
    class TEXT NOT NULL,
    min_count INTEGER NOT NULL,
    max_count INTEGER NOT NULL,
    CHECK (0 <= min_count AND min_count <= max_count),
    PRIMARY KEY (event_id, class)
) STRICT, WITHOUT ROWID;

-- Faction et rôle ne sont jamais stockés : déduits du serveur et de la classe (AD-6).
CREATE TABLE candidatures (
    id TEXT PRIMARY KEY CHECK (length(id) = 22),
    event_id TEXT NOT NULL REFERENCES events (id) ON DELETE CASCADE,
    pseudo TEXT NOT NULL,
    server TEXT NOT NULL,
    class TEXT NOT NULL,
    cp INTEGER NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'accepted', 'rejected', 'withdrawn')),
    origin TEXT NOT NULL CHECK (origin IN ('direct', 'public')),
    created_at TEXT NOT NULL CHECK (length(created_at) = 20),
    updated_at TEXT NOT NULL CHECK (length(updated_at) = 20)
) STRICT;

CREATE INDEX candidatures_event_id ON candidatures (event_id);

CREATE TABLE availabilities (
    candidature_id TEXT NOT NULL REFERENCES candidatures (id) ON DELETE CASCADE,
    slot_id TEXT NOT NULL REFERENCES slots (id) ON DELETE CASCADE,
    preferred INTEGER NOT NULL CHECK (preferred IN (0, 1)),
    PRIMARY KEY (candidature_id, slot_id)
) STRICT, WITHOUT ROWID;

CREATE INDEX availabilities_slot_id ON availabilities (slot_id);
