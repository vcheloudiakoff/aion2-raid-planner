import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { resetClientForTests } from "./client";
import { getParticipant, updateCandidature, withdrawCandidature } from "./participant";
import type { SubmitCandidature } from "./types";

const SECRET = "p".repeat(43);
const input: SubmitCandidature = {
  pseudo: "Nyra",
  server: "s1",
  class: "c1",
  cp: 1500,
  availabilities: [{ slotId: "a", preferred: true }],
};
const ok = (body: unknown = {}) =>
  new Response(JSON.stringify(body), { status: 200, headers: { "Content-Type": "application/json" } });

function serve() {
  const calls: { url: string; init?: RequestInit }[] = [];
  vi.stubGlobal(
    "fetch",
    vi.fn(async (url: string, init?: RequestInit) => {
      calls.push({ url, init });
      return ok({ status: "pending" });
    }),
  );
  return calls;
}

const headers = (c: { init?: RequestInit }) => (c.init?.headers ?? {}) as Record<string, string>;

beforeEach(() => resetClientForTests());
afterEach(() => vi.unstubAllGlobals());

describe("participant api", () => {
  it("reads, updates and withdraws with the secret in the header only", async () => {
    const calls = serve();
    await getParticipant(SECRET);
    await updateCandidature(SECRET, input);
    await withdrawCandidature(SECRET);
    expect(calls.map((c) => [c.init?.method ?? "GET", c.url])).toEqual([
      ["GET", "/api/participant"],
      ["POST", "/api/participant/candidature"],
      ["POST", "/api/participant/withdrawal"],
    ]);
    for (const c of calls) {
      expect(headers(c)["X-Link-Secret"]).toBe(SECRET);
      expect(c.url).not.toContain(SECRET);
      expect(String(c.init?.body ?? "")).not.toContain(SECRET);
    }
    // Sans jeton de formulaire ni honeypot.
    expect(JSON.parse(String(calls[1].init?.body))).toEqual(input);
    expect(JSON.parse(String(calls[2].init?.body))).toEqual({});
  });
});
