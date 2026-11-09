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
  /** Équipes souhaitées (numéros à partir de 1), vide = peu importe. */
  teams?: number[];
  /** Équipe préférée, seulement si au moins deux Équipes sont cochées. */
  preferredTeam?: number;
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

/** `GET /api/participant` et réponse des commandes du participant, sous son lien `/p/` : sa
 * candidature et l'événement public. `discordUrl` présent seulement si `accepted` et que l'événement
 * a un lien. `placement` et `teammates` présents seulement en `locked`, pour une candidature
 * `accepted` placée dans un groupe retenu. `backInPreparation` présent (et vrai) seulement pour une
 * candidature `accepted` d'un événement `open` après un déverrouillage. `acceptedRoster` et
 * `acceptedRosterSelf` présents ensemble, seulement pour une candidature `accepted`, quel que soit le
 * statut de l'événement. La confirmation arrive avec l'epic 5. */
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
  /** Équipes souhaitées, vide = peu importe. */
  teams: number[];
  preferredTeam?: number;
  event: PublicEvent;
  discordUrl?: string;
  placement?: ParticipantPlacement;
  /** Titulaire : une entrée, son équipe et ses autres titulaires ; banc : une entrée par équipe de
   * son groupe, dans l'ordre. Membres triés par slot, éventuellement aucun. */
  teammates?: TeammateParty[];
  backInPreparation?: true;
  /** Toutes les candidatures `accepted` de l'événement, celle-ci comprise, dans l'ordre de
   * création. */
  acceptedRoster?: RosterEntry[];
  /** Rang de cette candidature dans `acceptedRoster`, à partir de 0. */
  acceptedRosterSelf?: number;
}

/** Place du participant. `groupNumber` : rang parmi les groupes retenus (« G1 »). */
export interface ParticipantPlacement {
  groupNumber: number;
  slotId: string;
  startsAt: string;
  roomName: string;
  seat: ParticipantSeat;
}

export type ParticipantSeat = { kind: "starter"; party: number; index: number } | { kind: "bench"; benchIndex: number };

/** Une équipe vue par le participant : son numéro et ses coéquipiers visibles. */
export interface TeammateParty {
  party: number;
  members: Teammate[];
}

/** Un coéquipier visible : `Pseudo[SRV]` composé par le serveur et sa classe, rien d'autre. */
export interface Teammate {
  pseudoSrv: string;
  class: string;
}

/** Une ligne de la liste des acceptés : même forme qu'un coéquipier, `Pseudo[SRV]` et classe, rien
 * d'autre. */
export interface RosterEntry extends Teammate {
  /** Équipes souhaitées, vide = peu importe. */
  teams: number[];
  preferredTeam?: number;
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
  /** Équipes souhaitées, vide = peu importe. */
  teams: number[];
  preferredTeam?: number;
}

/** `GET /api/organizer` et réponse des commandes de l'organisateur, sous son lien `/o/`. `discordUrl`
 * présent seulement si l'événement a un lien. */
export interface OrganizerEvent {
  event: PublicEvent;
  discordUrl?: string;
  /** `classId → bornes`, une entrée par classe bornée de l'événement. */
  classBounds: Record<string, { min: number; max: number }>;
  /** Rôles demandés par Équipe, `roleId → nombre`, la première entrée pour l'Équipe 1. */
  teamRoles: Record<string, number>[];
  /** Une entrée par créneau, dans l'ordre de `event.slots`. */
  slots: OrganizerSlot[];
  /** Groupes, en sommeil compris (`retained` faux), triés par début de créneau puis par
   * `occurrence`. Vide avant le premier verrouillage. */
  groups: OrganizerGroup[];
  /** Ordre de création, jamais réordonnées. */
  candidatures: OrganizerCandidature[];
}

/** Couverture d'un créneau, calculée par le serveur sur les seules candidatures `accepted` (jamais
 * `pending`, `rejected` ni `withdrawn`). `coverage` : `classId → n`, une entrée par classe bornée
 * (zéros compris), plus toute classe `accepted` sans borne. `hasGroups` : un groupe existe sur ce
 * créneau, même en sommeil ; le supprimer le perdrait. */
export interface OrganizerSlot {
  slotId: string;
  availableAccepted: number;
  coverage: Record<string, number>;
  hasGroups: boolean;
}

/** Un groupe : son créneau et son rang d'occurrence, `retained` s'il fait partie du verrouillage
 * courant, la disposition et le libellé figés à sa création, et son nom de salle composé par le
 * serveur. Le libellé « G1 » se calcule ici, sur les seuls groupes `retained`, dans l'ordre reçu.
 * Ses placements et leurs avertissements, calculés par le serveur à la lecture. */
export interface OrganizerGroup {
  id: string;
  slotId: string;
  occurrence: number;
  retained: boolean;
  parties: number;
  partySize: number;
  roomLabel: string;
  roomName: string;
  /** Titulaires, triés par équipe puis par slot. */
  starters: GroupStarter[];
  /** Banc, trié par rang (contigu à partir de 1). */
  bench: GroupBench[];
  /** `classId → compte`, seulement les classes bornées dont le compte de titulaires sort des bornes. */
  classWarnings: Record<string, ClassWarning>;
}

/** Un titulaire ; `unavailable` s'il n'a pas coché le créneau du groupe (avertissement). */
export interface GroupStarter {
  candidatureId: string;
  party: number;
  index: number;
  unavailable: boolean;
}

/** Un joueur au banc ; `unavailable` s'il n'a pas coché le créneau du groupe. */
export interface GroupBench {
  candidatureId: string;
  benchIndex: number;
  unavailable: boolean;
}

/** Compte des titulaires d'une classe hors de ses bornes (« 3 / max 2 »). */
export interface ClassWarning {
  count: number;
  min: number;
  max: number;
}

/** Statut que l'organisateur peut demander. */
export type Decision = "accepted" | "rejected";

/** Corps de `POST /api/organizer/candidature-status`. */
export interface SetCandidatureStatus {
  candidatureId: string;
  status: Decision;
}

/** Créneau saisi : `startsAt` en UTC, forme `YYYY-MM-DDTHH:MM:SSZ` ; `durationMin` absente = durée
 * de l'instance. Corps de `POST /api/organizer/slots`. */
export interface NewSlot {
  startsAt: string;
  durationMin?: number;
}

/** Corps de `POST /api/organizer/settings` : il remplace tous les réglages. `discordUrl` et `minCp`
 * absents effacent la valeur ; une classe omise de `classBounds` garde ses bornes. */
export interface UpdateEventSettings {
  instance: string;
  region: string;
  factionScope: FactionScope;
  note: string;
  discordUrl?: string;
  visibility: Visibility;
  minCp?: number;
  classBounds: Record<string, { min: number; max: number }>;
  /** Absent : les rôles demandés par Équipe ne changent pas. */
  teamRoles?: Record<string, number>[];
}

/** Corps de `POST /api/organizer/slot-removal`. */
export interface SlotRemoval {
  slotId: string;
}

/** Corps de `POST /api/organizer/lock` : un id de créneau par groupe, doublons répétés. */
export interface LockEvent {
  slotIds: string[];
}

/** Une entrée du placement : titulaire (`party` et `index`) ou banc (`benchIndex`). */
export type PlacementEntry =
  | { candidatureId: string; groupId: string; party: number; index: number }
  | { candidatureId: string; groupId: string; benchIndex: number };

/** Corps de `POST /api/organizer/placement` : le placement complet des groupes retenus. */
export interface ApplyPlacement {
  placements: PlacementEntry[];
}

/** Corps de `POST /api/organizer/room-name` : le groupe retenu à renommer. Le nom est composé par le
 * serveur, jamais ici (AD-18). */
export interface RegenerateRoomName {
  groupId: string;
}
