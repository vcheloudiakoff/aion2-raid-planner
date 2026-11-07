// Données de test partagées : valeurs fictives, jamais la référence réelle.
import type {
  CreatedCandidature,
  CreatedEvent,
  OrganizerCandidature,
  OrganizerEvent,
  ParticipantView,
  PublicEvent,
  ReferenceSet,
} from "../api/types";

export const reference: ReferenceSet = {
  regions: [
    { id: "r1", label: { en: "Region One", fr: "Région Une" }, enabled: true },
    { id: "r_off", label: { en: "Closed" }, enabled: false },
  ],
  factions: { elyos: { en: "Elyos", fr: "Élyséens" }, asmodian: { en: "Asmodians", fr: "Asmodiens" } },
  servers: [
    { id: "s_e", region: "r1", faction: "elyos", tag: "SE", label: { en: "Server E" }, enabled: true },
    { id: "s_a", region: "r1", faction: "asmodian", tag: "SA", label: { en: "Server A" }, enabled: true },
    { id: "s_far", region: "r2", faction: "elyos", tag: "FAR", label: { en: "Far Server" }, enabled: true },
    { id: "s_off", region: "r1", faction: "elyos", tag: "OFF", label: { en: "Closed Server" }, enabled: false },
  ],
  instances: [
    {
      id: "i1",
      kind: "sanctuary",
      parties: 2,
      partySize: 5,
      durationMin: 60,
      crossFaction: true,
      roomLabel: "I1",
      label: { en: "Instance One", fr: "Instance Une" },
      enabled: true,
    },
    {
      id: "mono",
      kind: "expedition",
      parties: 1,
      partySize: 4,
      durationMin: 30,
      crossFaction: false,
      roomLabel: "MONO",
      label: { en: "Mono" },
      enabled: true,
    },
    {
      id: "i_off",
      kind: "sanctuary",
      parties: 2,
      partySize: 5,
      durationMin: 75,
      crossFaction: true,
      roomLabel: "OFF",
      label: { en: "Retired Instance", fr: "Instance retirée" },
      enabled: false,
    },
  ],
  classes: [
    { id: "c1", role: "healer", defaultMin: 0, defaultMax: 10, label: { en: "Healer Class", fr: "Classe Soin" }, enabled: true },
    { id: "c2", role: "dps", defaultMin: 1, defaultMax: 10, label: { en: "Damage Class", fr: "Classe Dégâts" }, enabled: true },
    { id: "c_off", role: "dps", defaultMin: 0, defaultMax: 1, label: { en: "Old" }, enabled: false },
  ],
  roles: [
    { id: "healer", label: { en: "Healer", fr: "Soin" }, enabled: true },
    { id: "dps", label: { en: "DPS" }, enabled: true },
  ],
};

export const ORGANIZER = "O".repeat(42) + "1";
export const JOIN = "J".repeat(42) + "2";
export const EVENT_ID = "E".repeat(21) + "3";

export const created: CreatedEvent = {
  eventId: EVENT_ID,
  organizerSecret: ORGANIZER,
  joinSecret: JOIN,
  links: { organizer: `/o/${ORGANIZER}`, join: `/j/${JOIN}`, public: `/e/${EVENT_ID}` },
};

export const PARTICIPANT = "P".repeat(42) + "4";
export const CANDIDATURE_ID = "C".repeat(21) + "5";

/** Deux créneaux à venir (2030) et un passé (2020), triés par début. */
export const publicEvent: PublicEvent = {
  id: EVENT_ID,
  instance: "i1",
  region: "r1",
  factionScope: "all",
  visibility: "listed",
  status: "open",
  minCp: 1200,
  note: "Vocal prêt",
  slots: [
    { id: "slot_past", startsAt: "2020-01-07T20:00:00Z", durationMin: 60 },
    { id: "slot_a", startsAt: "2030-01-08T20:00:00Z", durationMin: 60 },
    { id: "slot_b", startsAt: "2030-01-10T20:00:00Z", durationMin: 90 },
  ],
  countsByClass: { c1: 1, c2: 2 },
  countsByRole: { healer: 1, dps: 2 },
  missingRoles: [{ role: "healer", missing: 1 }],
};

export const createdCandidature: CreatedCandidature = {
  eventId: EVENT_ID,
  candidatureId: CANDIDATURE_ID,
  participantSecret: PARTICIPANT,
  status: "pending",
  links: { participant: `/p/${PARTICIPANT}` },
};

/** Vue participant : en attente, créneau à venir `slot_a` préféré, créneau passé coché. */
export const participantView: ParticipantView = {
  eventId: EVENT_ID,
  candidatureId: CANDIDATURE_ID,
  pseudo: "Nyra",
  server: "s_a",
  class: "c1",
  cp: 1500,
  status: "pending",
  availabilities: [
    { slotId: "slot_past", preferred: false },
    { slotId: "slot_a", preferred: true },
  ],
  event: publicEvent,
};

/** Ids de candidature de la vue organisateur, dans l'ordre de création. */
export const NYRA = "N".repeat(21) + "1";
export const KAEL = "K".repeat(21) + "2";
export const NYRA_TWIN = "M".repeat(21) + "3";
export const TOMA = "T".repeat(21) + "4";
export const LIO = "L".repeat(21) + "5";

const candidature = (patch: Partial<OrganizerCandidature> & Pick<OrganizerCandidature, "id">): OrganizerCandidature => ({
  pseudo: "Nyra",
  server: "s_a",
  pseudoSrv: "Nyra[SA]",
  class: "c1",
  role: "healer",
  cp: 1500,
  status: "pending",
  origin: "public",
  availabilities: [],
  duplicate: false,
  ...patch,
});

/** Vue organisateur : une candidature par statut, et deux `Nyra[SA]` signalées en doublon. */
export const organizerEvent: OrganizerEvent = {
  event: publicEvent,
  discordUrl: "https://discord.gg/abcdef",
  classBounds: { c1: { min: 1, max: 3 }, c2: { min: 1, max: 10 } },
  candidatures: [
    candidature({
      id: NYRA,
      availabilities: [
        { slotId: "slot_past", preferred: false },
        { slotId: "slot_a", preferred: true },
      ],
      duplicate: true,
    }),
    candidature({
      id: KAEL,
      pseudo: "Kael",
      server: "s_e",
      pseudoSrv: "Kael[SE]",
      class: "c2",
      role: "dps",
      cp: 2300,
      status: "accepted",
      origin: "direct",
      availabilities: [
        { slotId: "slot_a", preferred: false },
        { slotId: "slot_b", preferred: true },
      ],
    }),
    candidature({
      id: NYRA_TWIN,
      pseudo: "nyra",
      pseudoSrv: "nyra[SA]",
      cp: 1400,
      status: "accepted",
      origin: "direct",
      availabilities: [{ slotId: "slot_b", preferred: false }],
      duplicate: true,
    }),
    candidature({
      id: TOMA,
      pseudo: "Toma",
      server: "s_e",
      pseudoSrv: "Toma[SE]",
      class: "c2",
      role: "dps",
      cp: 900,
      status: "rejected",
      availabilities: [{ slotId: "slot_a", preferred: false }],
    }),
    candidature({
      id: LIO,
      pseudo: "Lio",
      pseudoSrv: "Lio[SA]",
      cp: 1100,
      status: "withdrawn",
      availabilities: [{ slotId: "slot_b", preferred: false }],
    }),
  ],
};
