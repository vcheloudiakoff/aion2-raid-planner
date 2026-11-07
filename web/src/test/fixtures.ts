// Données de test partagées : valeurs fictives, jamais la référence réelle.
import type { CreatedEvent, ReferenceSet } from "../api/types";

export const reference: ReferenceSet = {
  regions: [
    { id: "r1", label: { en: "Region One", fr: "Région Une" }, enabled: true },
    { id: "r_off", label: { en: "Closed" }, enabled: false },
  ],
  factions: { elyos: { en: "Elyos", fr: "Élyséens" }, asmodian: { en: "Asmodians", fr: "Asmodiens" } },
  servers: [],
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
  roles: [],
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
