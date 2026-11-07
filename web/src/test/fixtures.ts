// Données de test partagées : valeurs fictives, jamais la référence réelle.
import type { CreatedCandidature, CreatedEvent, ParticipantView, PublicEvent, ReferenceSet } from "../api/types";

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
