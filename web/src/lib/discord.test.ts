import { describe, expect, it } from "vitest";
import type { OrganizerEvent } from "../api/types";
import {
  EVENT_ID,
  JOIN,
  KAEL,
  NYRA,
  NYRA_TWIN,
  ORGANIZER,
  PARTICIPANT,
  TOMA,
  organizerEvent,
  organizerGroup,
  reference,
} from "../test/fixtures";
import { discordMessage, discordTime } from "./discord";

const ORIGIN = "https://raid.example.org";

/** G1 sur `slot_a` (Kael en 1.1, nyra en 2.5, banc Nyra puis Toma), un groupe en sommeil, G2 vide
 * sur `slot_b`. */
const view = (patch: Partial<OrganizerEvent["event"]> = {}, groups?: OrganizerEvent["groups"]): OrganizerEvent => ({
  ...organizerEvent,
  event: { ...organizerEvent.event, status: "locked", ...patch },
  groups: groups ?? [
    organizerGroup({
      id: "g1",
      starters: [
        { candidatureId: KAEL, party: 1, index: 1, unavailable: false },
        { candidatureId: NYRA_TWIN, party: 2, index: 5, unavailable: true },
      ],
      bench: [
        { candidatureId: TOMA, benchIndex: 2, unavailable: false },
        { candidatureId: NYRA, benchIndex: 1, unavailable: false },
      ],
    }),
    organizerGroup({
      id: "zz",
      retained: false,
      starters: [{ candidatureId: KAEL, party: 1, index: 2, unavailable: false }],
    }),
    organizerGroup({ id: "g2", slotId: "slot_b" }),
  ],
});

const free = (from: number, to: number, word: string) =>
  Array.from({ length: to - from + 1 }, (_, i) => `${from + i}. ${word}`);

describe("discordMessage", () => {
  it("writes one block per retained group, in slot order, then the public link, in French", () => {
    expect(discordMessage(view(), reference, "fr", ORIGIN).split("\n")).toEqual([
      "Instance Une · <t:1894132800:F>",
      "Salle : I1-G1",
      "Faction : All",
      "Équipe 1",
      "1. Kael[SE] · DPS",
      ...free(2, 5, "libre"),
      "Équipe 2",
      ...free(1, 4, "libre"),
      "5. nyra[SA] · Soin",
      "Banc : Nyra[SA], Toma[SE]",
      "",
      "Instance Une · <t:1894305600:F>",
      "Salle : I1-G2",
      "Faction : All",
      "Équipe 1",
      ...free(1, 5, "libre"),
      "Équipe 2",
      ...free(1, 5, "libre"),
      "",
      `${ORIGIN}/e/${EVENT_ID}`,
    ]);
  });

  it("uses the English templates and labels in English", () => {
    const lines = discordMessage(view(), reference, "en", ORIGIN).split("\n");
    expect(lines.slice(0, 5)).toEqual([
      "Instance One · <t:1894132800:F>",
      "Room: I1-G1",
      "Faction: All",
      "Party 1",
      "1. Kael[SE] · DPS",
    ]);
    expect(lines).toContain("2. empty");
    expect(lines).toContain("5. nyra[SA] · Healer");
    expect(lines).toContain("Bench: Nyra[SA], Toma[SE]");
    expect(lines.at(-1)).toBe(`${ORIGIN}/e/${EVENT_ID}`);
  });

  it("drops the Faction line on a single-faction raid", () => {
    for (const lang of ["fr", "en"] as const) {
      const text = discordMessage(view({ factionScope: "elyos" }), reference, lang, ORIGIN);
      expect(text, lang).not.toMatch(/Faction/);
      expect(text.split("\n")[1], lang).toBe(lang === "fr" ? "Salle : I1-G1" : "Room: I1-G1");
    }
  });

  it("works without any placement: every slot empty, no bench line", () => {
    const text = discordMessage(view({}, [organizerGroup({ id: "g1" })]), reference, "fr", ORIGIN);
    expect(text.split("\n")).toEqual([
      "Instance Une · <t:1894132800:F>",
      "Salle : I1-G1",
      "Faction : All",
      "Équipe 1",
      ...free(1, 5, "libre"),
      "Équipe 2",
      ...free(1, 5, "libre"),
      "",
      `${ORIGIN}/e/${EVENT_ID}`,
    ]);
    expect(text).not.toMatch(/Banc/);
  });

  it("follows the frozen layout of the group and falls back to ids without reference", () => {
    const text = discordMessage(
      view({}, [
        organizerGroup({
          id: "g1",
          parties: 1,
          partySize: 2,
          starters: [{ candidatureId: KAEL, party: 1, index: 2, unavailable: false }],
        }),
      ]),
      null,
      "fr",
      ORIGIN,
    );
    expect(text.split("\n").slice(0, 6)).toEqual([
      "i1 · <t:1894132800:F>",
      "Salle : I1-G1",
      "Faction : All",
      "Équipe 1",
      "1. libre",
      "2. Kael[SE] · dps",
    ]);
    expect(text).not.toMatch(/Équipe 2/);
  });

  it("writes a single group's block, then the public link, when asked for one group", () => {
    expect(discordMessage(view(), reference, "fr", ORIGIN, "g2").split("\n")).toEqual([
      "Instance Une · <t:1894305600:F>",
      "Salle : I1-G2",
      "Faction : All",
      "Équipe 1",
      ...free(1, 5, "libre"),
      "Équipe 2",
      ...free(1, 5, "libre"),
      "",
      `${ORIGIN}/e/${EVENT_ID}`,
    ]);
    // Un groupe en sommeil n'a pas de message.
    expect(discordMessage(view(), reference, "fr", ORIGIN, "zz")).toBe(`${ORIGIN}/e/${EVENT_ID}`);
  });

  it("never carries a secret nor a secret route", () => {
    for (const lang of ["fr", "en"] as const) {
      const text = discordMessage(view(), reference, lang, ORIGIN);
      for (const leak of [ORGANIZER, JOIN, PARTICIPANT, "/o/", "/j/", "/p/"]) {
        expect(text, `${lang} ${leak}`).not.toContain(leak);
      }
      expect(text.match(/https?:\/\/\S+/g)).toEqual([`${ORIGIN}/e/${EVENT_ID}`]);
    }
  });

  it("rounds the slot start down to whole seconds", () => {
    expect(discordTime("2030-01-08T20:00:00Z")).toBe("<t:1894132800:F>");
    expect(discordTime("2030-01-08T20:00:00.900Z")).toBe("<t:1894132800:F>");
  });
});
