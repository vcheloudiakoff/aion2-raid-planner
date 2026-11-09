-- Équipes d'un groupe à plusieurs Équipes (Ludra : 2 × 5), décision de Vincent du 2026-11-09.
--
-- `events.team_roles` : rôles demandés par Équipe, en JSON `[{"healer": 1, "tank": 1}, {...}]`, la
-- première entrée pour l'Équipe 1. Un signal pour l'organisateur, jamais un refus de placement.
--
-- `candidatures.teams` : Équipes souhaitées par le joueur, en masque (bit 0 = Équipe 1), 0 = peu
-- importe ; `preferred_team` : l'Équipe préférée (l'étoile) quand plusieurs sont cochées.

ALTER TABLE events ADD COLUMN team_roles TEXT NOT NULL DEFAULT '[]';

ALTER TABLE candidatures ADD COLUMN teams INTEGER NOT NULL DEFAULT 0 CHECK (teams >= 0);

ALTER TABLE candidatures ADD COLUMN preferred_team INTEGER CHECK (preferred_team >= 1);
