// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  LINKS_KEY,
  eventLinks,
  forgetEvent,
  forgetOrganizer,
  forgetParticipant,
  forgetSecret,
  readLinks,
  saveOrganizerLink,
  saveOrganizerLinks,
  saveParticipantLink,
} from "./links";
import { spyStorageWrites } from "../test/dom";

const id = (c: string) => c.repeat(22);
const secret = (c: string) => c.repeat(43);

const E = id("E");
const F = id("F");
const C = id("C");
const D = id("D");

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  localStorage.clear();
});

describe("links", () => {
  it("round-trips organizer and participant links", () => {
    expect(readLinks()).toEqual({});
    expect(saveOrganizerLinks(E, { organizer: secret("o"), join: secret("j") })).toBe(true);
    expect(saveParticipantLink(E, C, secret("p"))).toBe(true);
    expect(eventLinks(E)).toEqual({ organizer: secret("o"), join: secret("j"), participants: { [C]: secret("p") } });
    expect(JSON.parse(localStorage.getItem(LINKS_KEY)!)).toEqual(readLinks());
    expect(eventLinks(F)).toBeUndefined();
  });

  it("merges without overwriting other entries", () => {
    saveParticipantLink(E, C, secret("p"));
    saveParticipantLink(F, D, secret("q"));
    saveOrganizerLinks(E, { organizer: secret("o"), join: secret("j") });
    saveParticipantLink(E, D, secret("r"));
    expect(readLinks()).toEqual({
      [E]: { organizer: secret("o"), join: secret("j"), participants: { [C]: secret("p"), [D]: secret("r") } },
      [F]: { participants: { [D]: secret("q") } },
    });

    expect(forgetParticipant(E, C)).toBe(true);
    expect(eventLinks(E)?.participants).toEqual({ [D]: secret("r") });
    expect(forgetParticipant(F, D)).toBe(true);
    expect(eventLinks(F)).toBeUndefined();
    expect(forgetEvent(E)).toBe(true);
    expect(readLinks()).toEqual({});
  });

  it("saves the organizer link alone and keeps an existing direct link", () => {
    expect(saveOrganizerLink(E, secret("o"))).toBe(true);
    expect(eventLinks(E)).toEqual({ organizer: secret("o"), participants: {} });
    saveOrganizerLinks(F, { organizer: secret("o"), join: secret("j") });
    saveParticipantLink(F, C, secret("p"));
    expect(saveOrganizerLink(F, secret("q"))).toBe(true);
    expect(eventLinks(F)).toEqual({ organizer: secret("q"), join: secret("j"), participants: { [C]: secret("p") } });
    expect(saveOrganizerLink("short", secret("o"))).toBe(false);
    expect(saveOrganizerLink(D, `${secret("o").slice(1)}+`)).toBe(false);
    expect(eventLinks(D)).toBeUndefined();
  });

  it("forgets the organizer and direct links, keeping the candidatures", () => {
    saveOrganizerLinks(E, { organizer: secret("o"), join: secret("j") });
    saveParticipantLink(E, C, secret("p"));
    saveOrganizerLinks(F, { organizer: secret("q"), join: secret("r") });
    expect(forgetOrganizer(E)).toBe(true);
    expect(eventLinks(E)).toEqual({ participants: { [C]: secret("p") } });
    // Plus rien pour cet événement : l'entrée disparaît.
    expect(forgetOrganizer(F)).toBe(true);
    expect(eventLinks(F)).toBeUndefined();
    expect(Object.keys(readLinks())).toEqual([E]);
  });

  it("forgets every slot holding a secret, wherever it is", () => {
    saveOrganizerLinks(E, { organizer: secret("o"), join: secret("j") });
    saveParticipantLink(E, C, secret("p"));
    saveParticipantLink(F, D, secret("q"));
    saveOrganizerLinks(F, { organizer: secret("r"), join: secret("s") });
    // Un lien direct mort ne touche que lui.
    expect(forgetSecret(secret("s"))).toBe(true);
    expect(eventLinks(F)).toEqual({ organizer: secret("r"), participants: { [D]: secret("q") } });
    // Un lien organisateur mort emporte le lien direct de son événement, jamais ses candidatures.
    expect(forgetSecret(secret("o"))).toBe(true);
    expect(eventLinks(E)).toEqual({ participants: { [C]: secret("p") } });
    expect(forgetSecret(secret("j"))).toBe(false);
    expect(forgetSecret(secret("p"))).toBe(true);
    expect(eventLinks(E)).toBeUndefined();
    expect(forgetSecret(secret("q"))).toBe(true);
    expect(forgetSecret(secret("r"))).toBe(true);
    expect(readLinks()).toEqual({});
  });

  it("writes nothing when no slot holds the secret, or when it is malformed", () => {
    saveParticipantLink(E, C, secret("p"));
    const before = localStorage.getItem(LINKS_KEY);
    const setItem = spyStorageWrites();
    expect(forgetSecret(secret("x"))).toBe(false);
    expect(forgetSecret("short")).toBe(false);
    expect(forgetSecret("")).toBe(false);
    expect(setItem).not.toHaveBeenCalled();
    expect(localStorage.getItem(LINKS_KEY)).toBe(before);
  });

  it("refuses malformed ids and secrets", () => {
    expect(saveOrganizerLinks("short", { organizer: secret("o"), join: secret("j") })).toBe(false);
    expect(saveOrganizerLinks(E, { organizer: secret("o").slice(1), join: secret("j") })).toBe(false);
    expect(saveParticipantLink(E, C, `${secret("p").slice(1)}+`)).toBe(false);
    expect(saveParticipantLink(E, "bad", secret("p"))).toBe(false);
    expect(readLinks()).toEqual({});
  });

  it.each(["{oops", "null", "[]", '"text"', "42"])("treats %s as no links", (raw) => {
    localStorage.setItem(LINKS_KEY, raw);
    expect(readLinks()).toEqual({});
  });

  it("drops invalid entries and keeps valid ones", () => {
    localStorage.setItem(
      LINKS_KEY,
      JSON.stringify({
        [E]: {
          organizer: secret("o"),
          join: 12,
          participants: { [C]: secret("p"), [D]: "short", bad: secret("x") },
        },
        [F]: "not an object",
        short: { organizer: secret("o"), participants: {} },
        [id("G")]: { participants: [secret("p")] },
        [id("H")]: { join: secret("j"), participants: "nope" },
        ["!".repeat(22)]: { organizer: secret("o"), participants: {} },
      }),
    );
    expect(readLinks()).toEqual({
      [E]: { organizer: secret("o"), participants: { [C]: secret("p") } },
      [id("H")]: { join: secret("j"), participants: {} },
    });
    // Une écriture repart des seules entrées valides, sans perdre les autres.
    expect(saveParticipantLink(F, D, secret("q"))).toBe(true);
    expect(Object.keys(readLinks()).sort()).toEqual([E, F, id("H")].sort());
  });

  it("keeps the page working when localStorage throws", () => {
    const boom = () => {
      throw new DOMException("blocked", "SecurityError");
    };
    vi.stubGlobal("localStorage", { getItem: boom, setItem: boom, removeItem: boom });
    expect(readLinks()).toEqual({});
    expect(eventLinks(E)).toBeUndefined();
    expect(saveOrganizerLinks(E, { organizer: secret("o"), join: secret("j") })).toBe(false);
    expect(saveOrganizerLink(E, secret("o"))).toBe(false);
    expect(saveParticipantLink(E, C, secret("p"))).toBe(false);
    expect(forgetParticipant(E, C)).toBe(false);
    expect(forgetEvent(E)).toBe(false);
    expect(forgetOrganizer(E)).toBe(false);
    expect(forgetSecret(secret("o"))).toBe(false);
  });

  it("keeps the page working when localStorage is absent", () => {
    vi.stubGlobal("localStorage", undefined);
    expect(readLinks()).toEqual({});
    expect(saveParticipantLink(E, C, secret("p"))).toBe(false);
  });
});
