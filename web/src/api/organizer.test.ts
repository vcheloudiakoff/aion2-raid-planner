import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { resetClientForTests } from "./client";
import { getOrganizerView, setCandidatureStatus } from "./organizer";

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
});
