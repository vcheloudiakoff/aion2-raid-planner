import { describe, expect, it } from "vitest";
import type { OrganizerEvent } from "../api/types";
import { organizerEvent, reference } from "../test/fixtures";
import { duplicateDraft, duplicateFromState, type DuplicateDraft } from "./duplicate";

const view = (event: Partial<OrganizerEvent["event"]> = {}, extra: Partial<OrganizerEvent> = {}): OrganizerEvent => ({
  ...organizerEvent,
  ...extra,
  event: { ...organizerEvent.event, ...event },
});

const draft: DuplicateDraft = {
  instance: "i1",
  region: "r1",
  factionScope: "all",
  note: "Vocal prêt",
  discordUrl: "https://discord.gg/abcdef",
  visibility: "listed",
  minCp: 1200,
  classBounds: { c1: { min: 1, max: 3 }, c2: { min: 1, max: 10 } },
  slots: [{}, {}, { durationMin: 90 }],
};

describe("duplicateDraft", () => {
  it("copies everything but the dates, one line per slot, the instance duration left empty", () => {
    expect(duplicateDraft(organizerEvent, reference)).toEqual(draft);
  });

  it("keeps every duration without the reference, or for an instance it no longer knows", () => {
    const all = [{ durationMin: 60 }, { durationMin: 60 }, { durationMin: 90 }];
    expect(duplicateDraft(organizerEvent, null).slots).toEqual(all);
    expect(duplicateDraft(view({ instance: "gone" }), reference).slots).toEqual(all);
  });

  it("leaves out an absent minimum CP and Discord link, and copies the visibility and scope", () => {
    const d = duplicateDraft(
      view({ minCp: undefined, visibility: "unlisted", factionScope: "asmodian" }, { discordUrl: undefined }),
      reference,
    );
    expect(d).not.toHaveProperty("minCp");
    expect(d.discordUrl).toBe("");
    expect(d.visibility).toBe("unlisted");
    expect(d.factionScope).toBe("asmodian");
  });

  it("never shares the bounds objects of the view", () => {
    const d = duplicateDraft(organizerEvent, reference);
    expect(d.classBounds.c1).not.toBe(organizerEvent.classBounds.c1);
  });
});

describe("duplicateFromState", () => {
  it("reads back a draft from the navigation state", () => {
    expect(duplicateFromState({ duplicate: draft })).toEqual(draft);
    const { minCp: _, ...noCp } = draft;
    expect(duplicateFromState({ duplicate: noCp })).toEqual(noCp);
  });

  it.each([
    ["no state", null],
    ["no draft", { created: {} }],
    ["a draft that is not an object", { duplicate: "x" }],
    ["an unknown scope", { duplicate: { ...draft, factionScope: "both" } }],
    ["an unknown visibility", { duplicate: { ...draft, visibility: "hidden" } }],
    ["a decimal minimum CP", { duplicate: { ...draft, minCp: 1.5 } }],
    ["a missing note", { duplicate: { ...draft, note: undefined } }],
    ["slots that are not a list", { duplicate: { ...draft, slots: {} } }],
  ])("refuses %s", (_, state) => {
    expect(duplicateFromState(state)).toBeNull();
  });

  it("drops invalid bounds and durations, and keeps 1 to 14 slot lines", () => {
    const d = duplicateFromState({
      duplicate: {
        ...draft,
        classBounds: { c1: { min: 3, max: 1 }, c2: { min: -1, max: 2 }, c3: "x", c4: { min: 0, max: 2 } },
        slots: [{ durationMin: 1.5 }, "x", { durationMin: 45 }, ...Array.from({ length: 20 }, () => ({}))],
      },
    })!;
    expect(d.classBounds).toEqual({ c4: { min: 0, max: 2 } });
    expect(d.slots).toHaveLength(14);
    expect(d.slots.slice(0, 3)).toEqual([{}, {}, { durationMin: 45 }]);
    expect(duplicateFromState({ duplicate: { ...draft, slots: [] } })!.slots).toEqual([{}]);
  });
});
