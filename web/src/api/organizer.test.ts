import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { resetClientForTests } from "./client";
import {
  addSlot,
  applyPlacement,
  cancelEvent,
  getOrganizerView,
  lockEvent,
  regenerateRoomName,
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

  it("sends the full placement in the body, ids never in the URL, the secret in the header only", async () => {
    const calls = serve();
    const [C, D, G] = ["c".repeat(22), "d".repeat(22), "g".repeat(22)];
    const placements = [
      { candidatureId: C, groupId: G, party: 2, index: 5 },
      { candidatureId: D, groupId: G, benchIndex: 1 },
    ];
    await applyPlacement(SECRET, placements);
    expect(calls.map((c) => [c.init?.method, c.url])).toEqual([["POST", "/api/organizer/placement"]]);
    expect(headers(calls[0])["X-Link-Secret"]).toBe(SECRET);
    for (const id of [SECRET, C, D, G]) expect(calls[0].url).not.toContain(id);
    expect(String(calls[0].init?.body)).not.toContain(SECRET);
    expect(JSON.parse(String(calls[0].init?.body))).toEqual({ placements });
  });

  it("asks for a new room name with the group in the body, never in the URL, the secret in the header only", async () => {
    const calls = serve();
    const G = "g".repeat(22);
    await regenerateRoomName(SECRET, G);
    expect(calls.map((c) => [c.init?.method, c.url])).toEqual([["POST", "/api/organizer/room-name"]]);
    expect(headers(calls[0])["X-Link-Secret"]).toBe(SECRET);
    for (const id of [SECRET, G]) expect(calls[0].url).not.toContain(id);
    expect(JSON.parse(String(calls[0].init?.body))).toEqual({ groupId: G });
  });
});
