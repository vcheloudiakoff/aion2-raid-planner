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

/** `GET /api/form-token?class=create|candidature` : jeton opaque, renvoyé tel quel dans le champ
 * `formToken` du POST, à côté du honeypot `website` laissé vide. */
export interface FormToken {
  token: string;
}

/** Corps `application/problem+json` de toute erreur `/api/*` (RFC 9457). `title` est le libellé HTTP
 * standard, jamais un texte à afficher : le client traduit `errors.<code>`. `fields`, présent sur une
 * erreur de validation seulement, associe un chemin JSON camelCase (`slots.2.startsAt`) à son code. */
export interface Problem {
  type: string;
  title: string;
  status: number;
  code: string;
  fields?: Record<string, string>;
}

/** Portée de faction d'un événement. */
export type FactionScope = "all" | Faction;

export type Visibility = "listed" | "unlisted";

/** Corps de `POST /api/events`, à côté de `formToken` et du honeypot `website`. */
export interface CreateEvent {
  instance: string;
  region: string;
  factionScope: FactionScope;
  /** `startsAt` en UTC, forme `YYYY-MM-DDTHH:MM:SSZ` ; `durationMin` absente = durée de l'instance. */
  slots: { startsAt: string; durationMin?: number }[];
  note?: string;
  discordUrl?: string;
  visibility?: Visibility;
  minCp?: number;
  /** `classId → bornes` ; une classe omise prend ses bornes par défaut. */
  classBounds?: Record<string, { min: number; max: number }>;
}

/** Réponse 201 de `POST /api/events`, rendue une seule fois. Chemins relatifs, à préfixer par
 * `location.origin`. */
export interface CreatedEvent {
  eventId: string;
  organizerSecret: string;
  joinSecret: string;
  links: { organizer: string; join: string; public: string };
}

export type EventStatus = "open" | "locked" | "cancelled";

/** Créneau d'un événement : `startsAt` en UTC, forme `YYYY-MM-DDTHH:MM:SSZ`. */
export interface PublicSlot {
  id: string;
  startsAt: string;
  durationMin: number;
}

/** `GET /api/events/{eventId}` et `GET /api/join` : l'événement vu par le public. Jamais de pseudo,
 * de serveur de candidat, d'id de candidature ni de lien Discord ; comptes calculés par le serveur. */
export interface PublicEvent {
  id: string;
  instance: string;
  region: string;
  factionScope: FactionScope;
  visibility: Visibility;
  status: EventStatus;
  minCp?: number;
  note: string;
  /** Triés par `startsAt`. */
  slots: PublicSlot[];
  /** `classId → n`, une entrée par classe bornée de l'événement, zéros compris. */
  countsByClass: Record<string, number>;
  /** `roleId → n`. */
  countsByRole: Record<string, number>;
  /** Manques strictement positifs, triés par id de rôle. */
  missingRoles: { role: string; missing: number }[];
}

export type CandidatureStatus = "pending" | "accepted" | "rejected" | "withdrawn";

/** Corps de `POST /api/events/{eventId}/candidatures` et de `POST /api/join/candidatures`, à côté de
 * `formToken` et du honeypot `website`. Une entrée d'`availabilities` par créneau coché. */
export interface SubmitCandidature {
  pseudo: string;
  server: string;
  class: string;
  cp: number;
  availabilities: { slotId: string; preferred: boolean }[];
}

/** Réponse 201 d'une candidature, rendue une seule fois. Chemin relatif, à préfixer par
 * `location.origin`. */
export interface CreatedCandidature {
  eventId: string;
  candidatureId: string;
  participantSecret: string;
  status: CandidatureStatus;
  links: { participant: string };
}

/** `GET /api/participant` et réponse des commandes du participant, sous son lien `/p/` : sa seule
 * candidature et l'événement public. `discordUrl` présent seulement si `accepted` et que l'événement
 * a un lien. Placement, salle et confirmation arrivent avec les epics 3 et 5. */
export interface ParticipantView {
  eventId: string;
  candidatureId: string;
  pseudo: string;
  server: string;
  class: string;
  cp: number;
  status: CandidatureStatus;
  /** Triées par début de créneau, créneaux passés compris. */
  availabilities: { slotId: string; preferred: boolean }[];
  event: PublicEvent;
  discordUrl?: string;
}

/** Lien par lequel une candidature est arrivée : `/j/` direct, `/e/` public. */
export type Origin = "direct" | "public";

/** Un créneau coché par une candidature, et sa préférence. */
export interface Availability {
  slotId: string;
  preferred: boolean;
}

/** Une candidature vue par l'organisateur. `pseudoSrv` est composé par le serveur, jamais par le
 * client (AD-6) ; `role` est le rôle de la classe, ou l'id de la classe si la référence ne la connaît
 * plus. `duplicate` signale un autre `pending` ou `accepted` au même pseudo et au même serveur. */
export interface OrganizerCandidature {
  id: string;
  pseudo: string;
  server: string;
  pseudoSrv: string;
  class: string;
  role: string;
  cp: number;
  status: CandidatureStatus;
  origin: Origin;
  /** Triées par début de créneau, créneaux passés compris. */
  availabilities: Availability[];
  duplicate: boolean;
}

/** `GET /api/organizer` et réponse des commandes de l'organisateur, sous son lien `/o/`. `discordUrl`
 * présent seulement si l'événement a un lien ; `slot.hasGroups` arrive avec les groupes (epic 3). */
export interface OrganizerEvent {
  event: PublicEvent;
  discordUrl?: string;
  /** `classId → bornes`, une entrée par classe bornée de l'événement. */
  classBounds: Record<string, { min: number; max: number }>;
  /** Ordre de création, jamais réordonnées. */
  candidatures: OrganizerCandidature[];
}

/** Statut que l'organisateur peut demander. */
export type Decision = "accepted" | "rejected";

/** Corps de `POST /api/organizer/candidature-status`. */
export interface SetCandidatureStatus {
  candidatureId: string;
  status: Decision;
}
