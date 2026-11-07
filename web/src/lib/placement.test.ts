import { describe, expect, it } from "vitest";
import type { OrganizerEvent } from "../api/types";
import { KAEL, NYRA, NYRA_TWIN, organizerEvent, organizerGroup } from "../test/fixtures";
import {
  bodyOf,
  changedCount,
  placeAt,
  placementOf,
  seatOf,
  swapSeats,
  toBench,
  unplace,
  type Placement,
} from "./placement";

const view = (patch: Partial<OrganizerEvent> = {}): OrganizerEvent => ({
  ...organizerEvent,
  event: { ...organizerEvent.event, status: "locked" },
  groups: [
    organizerGroup({
      id: "g1",
      starters: [{ candidatureId: KAEL, party: 1, index: 2, unavailable: false }],
      bench: [{ candidatureId: NYRA_TWIN, benchIndex: 1, unavailable: true }],
    }),
    organizerGroup({ id: "zz", retained: false, starters: [{ candidatureId: NYRA, party: 1, index: 1, unavailable: false }] }),
    organizerGroup({ id: "g2", slotId: "slot_b" }),
  ],
  ...patch,
});

describe("placement", () => {
  it("reads the retained groups only, in order", () => {
    expect(placementOf(view())).toEqual({
      g1: { starters: { "1.2": KAEL }, bench: [NYRA_TWIN] },
      g2: { starters: {}, bench: [] },
    });
  });

  it("moves instead of duplicating, and keeps the bench contiguous", () => {
    const p = placementOf(view());
    const moved = placeAt(p, KAEL, { groupId: "g2", party: 2, index: 5 });
    expect(moved).toEqual({
      g1: { starters: {}, bench: [NYRA_TWIN] },
      g2: { starters: { "2.5": KAEL }, bench: [] },
    });
    // L'original n'est jamais modifié.
    expect(p.g1.starters).toEqual({ "1.2": KAEL });
    const benched = placeAt(moved, NYRA_TWIN, { groupId: "g2", bench: 0 });
    expect(benched.g1.bench).toEqual([]);
    expect(benched.g2.bench).toEqual([NYRA_TWIN]);
    expect(unplace(benched, KAEL).g2).toEqual({ starters: {}, bench: [NYRA_TWIN] });
  });

  it("never overwrites a slot taken by another candidature", () => {
    const p = placementOf(view());
    expect(placeAt(p, NYRA, { groupId: "g1", party: 1, index: 2 })).toBe(p);
    // La même candidature sur sa propre place : rien ne bouge.
    expect(placeAt(p, KAEL, { groupId: "g1", party: 1, index: 2 })).toEqual(p);
  });

  it("sends a starter to the end of its own bench", () => {
    const p = toBench(placementOf(view()), KAEL);
    expect(p.g1).toEqual({ starters: {}, bench: [NYRA_TWIN, KAEL] });
    // Déjà au banc : rien ne change.
    expect(toBench(p, KAEL)).toBe(p);
  });

  it("swaps two occupants, across groups, or moves into an empty spot", () => {
    const p: Placement = {
      g1: { starters: { "1.1": "a", "1.2": "b" }, bench: ["c", "d"] },
      g2: { starters: { "2.3": "e" }, bench: [] },
    };
    expect(swapSeats(p, { groupId: "g1", party: 1, index: 1 }, { groupId: "g2", party: 2, index: 3 })).toEqual({
      g1: { starters: { "1.1": "e", "1.2": "b" }, bench: ["c", "d"] },
      g2: { starters: { "2.3": "a" }, bench: [] },
    });
    // Banc ↔ slot libre : le rang vidé disparaît.
    expect(swapSeats(p, { groupId: "g1", bench: 0 }, { groupId: "g1", party: 2, index: 4 })).toEqual({
      g1: { starters: { "1.1": "a", "1.2": "b", "2.4": "c" }, bench: ["d"] },
      g2: { starters: { "2.3": "e" }, bench: [] },
    });
    // Slot ↔ banc d'un autre groupe (ajout à la fin).
    expect(swapSeats(p, { groupId: "g1", party: 1, index: 2 }, { groupId: "g2", bench: 0 })).toEqual({
      g1: { starters: { "1.1": "a" }, bench: ["c", "d"] },
      g2: { starters: { "2.3": "e" }, bench: ["b"] },
    });
    expect(swapSeats(p, { groupId: "g1", bench: 1 }, { groupId: "g1", bench: 1 })).toBe(p);
    expect(seatOf(p, "d")).toEqual({ groupId: "g1", bench: 1 });
    expect(seatOf(p, "zz")).toBeNull();
  });

  it("builds the full body: starters by party then slot, then the bench ranked from 1", () => {
    const p: Placement = {
      g1: { starters: { "2.1": "x", "1.3": "y" }, bench: ["z", "w"] },
      g2: { starters: {}, bench: [] },
    };
    expect(bodyOf(p)).toEqual([
      { candidatureId: "y", groupId: "g1", party: 1, index: 3 },
      { candidatureId: "x", groupId: "g1", party: 2, index: 1 },
      { candidatureId: "z", groupId: "g1", benchIndex: 1 },
      { candidatureId: "w", groupId: "g1", benchIndex: 2 },
    ]);
  });

  it("counts each candidature changed since the draft once", () => {
    const base = view();
    expect(changedCount(base, base)).toBe(0);
    // Classe seule changée : une.
    const reclassed = view({
      candidatures: base.candidatures.map((c) => (c.id === KAEL ? { ...c, class: "c1", role: "healer" } : c)),
    });
    expect(changedCount(base, reclassed)).toBe(1);
    const changed = view({
      candidatures: base.candidatures
        // Statut et CP changés pour la même : une seule.
        .map((c) => (c.id === NYRA ? { ...c, status: "accepted" as const, cp: 9999 } : c))
        // Disponibilités changées.
        .map((c) => (c.id === NYRA_TWIN ? { ...c, availabilities: [] } : c)),
      groups: base.groups.map((g) =>
        // Placement serveur changé.
        g.id === "g1" ? { ...g, starters: [{ candidatureId: KAEL, party: 2, index: 2, unavailable: false }] } : g,
      ),
    });
    expect(changedCount(base, changed)).toBe(3);
    // Ajoutée, puis disparue.
    const extra = { ...base.candidatures[0], id: "X".repeat(22) };
    expect(changedCount(base, view({ candidatures: [...base.candidatures, extra] }))).toBe(1);
    expect(changedCount(view({ candidatures: [...base.candidatures, extra] }), base)).toBe(1);
    // Préférence seule : ne compte pas.
    const pref = view({
      candidatures: base.candidatures.map((c) =>
        c.id === KAEL ? { ...c, availabilities: c.availabilities.map((a) => ({ ...a, preferred: !a.preferred })) } : c,
      ),
    });
    expect(changedCount(base, pref)).toBe(0);
  });
});
