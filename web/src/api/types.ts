// Miroir TypeScript des DTO de `crates/server/src/http/dto.rs` (AD-5), modifié dans le même commit.
// Aucune valeur de référence ici : seulement la forme du contrat.

/** Libellé bilingue : `fr` absent si non saisi, repli sur `en`. */
export interface Label {
  en: string;
  fr?: string;
}

/** Les deux factions, seules valeurs du domaine qui ne sont pas des données. */
export type Faction = "elyos" | "asmodian";

export type InstanceKind = "sanctuary" | "expedition";

export interface Region {
  id: string;
  label: Label;
  enabled: boolean;
}

export interface Server {
  id: string;
  region: string;
  faction: Faction;
  /** Le `[SRV]` tel que le client du jeu l'affiche, sans crochets. */
  tag: string;
  label: Label;
  enabled: boolean;
}

export interface Instance {
  id: string;
  kind: InstanceKind;
  parties: number;
  partySize: number;
  durationMin: number;
  crossFaction: boolean;
  roomLabel: string;
  label: Label;
  enabled: boolean;
}

export interface GameClass {
  id: string;
  role: string;
  defaultMin: number;
  defaultMax: number;
  label: Label;
  enabled: boolean;
}

export interface Role {
  id: string;
  label: Label;
  enabled: boolean;
}

/** `GET /api/reference`, entrées désactivées comprises. */
export interface ReferenceSet {
  regions: Region[];
  factions: Record<Faction, Label>;
  servers: Server[];
  instances: Instance[];
  classes: GameClass[];
  roles: Role[];
}

/** Corps `application/problem+json` de toute erreur `/api/*` (RFC 9457). `title` est le libellé HTTP
 * standard, jamais un texte à afficher : le client traduit `errors.<code>`. */
export interface Problem {
  type: string;
  title: string;
  status: number;
  code: string;
}
