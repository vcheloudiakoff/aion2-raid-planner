import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { resetClientForTests } from "./client";
import {
  addSlot,
  cancelEvent,
  getOrganizerView,
  lockEvent,
  removeSlot,
  setCandidatureStatus,
  unlockEvent,
  updateEventSettings,
} from "./organizer";

const SECRET = "o".repeat(43);
const CANDIDATURE = "c".repeat(22);
const ok = (body: unknown = {}) =>
  new Response(JSON.stringify(body), { status: 200, headers: { "Content-Type": "application/json" } });

function serve() {
  const calls: { url: string; init?: RequestInit }[] = [];
  vi.stubGlobal(
    "fetch",
    vi.fn(async (url: string, init?: RequestInit) => {
      calls.push({ url, init });
      return ok({ candidatures: [] });
    }),
  );
  return calls;
}

const headers = (c: { init?: RequestInit }) => (c.init?.headers ?? {}) as Record<string, string>;

beforeEach(() => resetClientForTests());
afterEach(() => vi.unstubAllGlobals());

describe("organizer api", () => {
  it("reads and decides with the secret in the header only, the id in the body", async () => {
    const calls = serve();
    await getOrganizerView(SECRET);
    await setCandidatureStatus(SECRET, CANDIDATURE, "accepted");
    await setCandidatureStatus(SECRET, CANDIDATURE, "rejected");
    expect(calls.map((c) => [c.init?.method ?? "GET", c.url])).toEqual([
      ["GET", "/api/organizer"],
      ["POST", "/api/organizer/candidature-status"],
      ["POST", "/api/organizer/candidature-status"],
    ]);
    for (const c of calls) {
      expect(headers(c)["X-Link-Secret"]).toBe(SECRET);
      expect(c.url).not.toContain(SECRET);
      expect(c.url).not.toContain(CANDIDATURE);
      expect(String(c.init?.body ?? "")).not.toContain(SECRET);
    }
    // Sans jeton de formulaire ni honeypot.
    expect(JSON.parse(String(calls[1].init?.body))).toEqual({ candidatureId: CANDIDATURE, status: "accepted" });
    expect(JSON.parse(String(calls[2].init?.body))).toEqual({ candidatureId: CANDIDATURE, status: "rejected" });
  });

  it("sends the settings, slot and cancellation commands with the secret in the header only", async () => {
    const calls = serve();
    const SLOT = "s".repeat(22);
    const settings = {
      instance: "i1",
      region: "r1",
      factionScope: "all" as const,
      note: "Vocal prêt",
      visibility: "unlisted" as const,
      classBounds: { c1: { min: 1, max: 3 } },
    };
    await updateEventSettings(SECRET, settings);
    await addSlot(SECRET, { startsAt: "2030-01-08T20:00:00Z", durationMin: 90 });
    await removeSlot(SECRET, SLOT);
    await cancelEvent(SECRET);
    expect(calls.map((c) => [c.init?.method, c.url])).toEqual([
      ["POST", "/api/organizer/settings"],
      ["POST", "/api/organizer/slots"],
      ["POST", "/api/organizer/slot-removal"],
      ["POST", "/api/organizer/cancellation"],
    ]);
    for (const c of calls) {
      expect(headers(c)["X-Link-Secret"]).toBe(SECRET);
      expect(c.url).not.toContain(SECRET);
      expect(c.url).not.toContain(SLOT);
      expect(String(c.init?.body ?? "")).not.toContain(SECRET);
    }
    expect(calls.map((c) => JSON.parse(String(c.init?.body)))).toEqual([
      settings,
      { startsAt: "2030-01-08T20:00:00Z", durationMin: 90 },
      { slotId: SLOT },
      {},
    ]);
  });

  it("locks with the slot ids repeated in the body and unlocks with an empty body, the secret in the header only", async () => {
    const calls = serve();
    const [A, B] = ["a".repeat(22), "b".repeat(22)];
    await lockEvent(SECRET, [A, A, B]);
    await unlockEvent(SECRET);
    expect(calls.map((c) => [c.init?.method, c.url])).toEqual([
      ["POST", "/api/organizer/lock"],
      ["POST", "/api/organizer/unlock"],
    ]);
    for (const c of calls) {
      expect(headers(c)["X-Link-Secret"]).toBe(SECRET);
      expect(c.url).not.toContain(SECRET);
      expect(c.url).not.toContain(A);
      expect(String(c.init?.body ?? "")).not.toContain(SECRET);
    }
    expect(calls.map((c) => JSON.parse(String(c.init?.body)))).toEqual([{ slotIds: [A, A, B] }, {}]);
  });
});
