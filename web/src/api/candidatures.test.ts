import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { getJoinEvent, getPublicEvent, submitCandidature } from "./candidatures";
import { resetClientForTests } from "./client";
import { MIN_SUBMIT_DELAY_MS, type FormDeps } from "./events";
import type { SubmitCandidature } from "./types";

const ID = "e".repeat(22);
const SECRET = "j".repeat(43);
const input: SubmitCandidature = {
  pseudo: "Nyra",
  server: "s1",
  class: "c1",
  cp: 1500,
  availabilities: [{ slotId: "a", preferred: true }],
};
const ok = (body: unknown = {}) =>
  new Response(JSON.stringify(body), { status: 200, headers: { "Content-Type": "application/json" } });
const problem = (code: string) =>
  new Response(JSON.stringify({ type: "about:blank", title: "x", status: 400, code }), {
    status: 400,
    headers: { "Content-Type": "application/problem+json" },
  });

function serve(...replies: Response[]) {
  const calls: { url: string; init?: RequestInit }[] = [];
  vi.stubGlobal(
    "fetch",
    vi.fn(async (url: string, init?: RequestInit) => {
      calls.push({ url, init });
      const r = replies.shift();
      if (!r) throw new Error("unexpected fetch");
      return r;
    }),
  );
  return calls;
}

const headers = (c: { init?: RequestInit }) => (c.init?.headers ?? {}) as Record<string, string>;
const sent = (c: { init?: RequestInit }) => JSON.parse(String(c.init?.body)) as Record<string, unknown>;

beforeEach(() => resetClientForTests());
afterEach(() => vi.unstubAllGlobals());

describe("reads", () => {
  it("reads the public event by id, without any secret", async () => {
    const calls = serve(ok({ id: ID }));
    expect(await getPublicEvent(ID)).toEqual({ id: ID });
    expect(calls[0].url).toBe(`/api/events/${ID}`);
    expect(headers(calls[0])["X-Link-Secret"]).toBeUndefined();
  });

  it("reads the direct link with the secret in the header, never in the URL", async () => {
    const calls = serve(ok({ id: ID }));
    await getJoinEvent(SECRET);
    expect(calls[0].url).toBe("/api/join");
    expect(headers(calls[0])["X-Link-Secret"]).toBe(SECRET);
  });
});

describe("submitCandidature", () => {
  it("posts to the public route or the join route, with the honeypot and the token", async () => {
    const calls = serve(ok(), ok());
    const d: FormDeps = { renewToken: vi.fn(), sleep: vi.fn() };
    await submitCandidature({ mode: "public", eventId: ID }, input, "tok", "", d);
    await submitCandidature({ mode: "join", secret: SECRET }, input, "tok", "", d);
    expect(calls.map((c) => c.url)).toEqual([`/api/events/${ID}/candidatures`, "/api/join/candidatures"]);
    expect(headers(calls[0])["X-Link-Secret"]).toBeUndefined();
    expect(headers(calls[1])["X-Link-Secret"]).toBe(SECRET);
    expect(sent(calls[1])).toEqual({ ...input, formToken: "tok", website: "" });
  });

  it("renews a candidature token once on form.token_invalid", async () => {
    const calls = serve(problem("form.token_invalid"), ok());
    const d = { renewToken: vi.fn(async () => "fresh"), sleep: vi.fn(async () => {}) };
    await submitCandidature({ mode: "join", secret: SECRET }, input, "old", "", d);
    expect(d.renewToken).toHaveBeenCalledWith("candidature");
    expect(d.sleep).toHaveBeenCalledWith(MIN_SUBMIT_DELAY_MS);
    expect(calls.map((c) => sent(c).formToken)).toEqual(["old", "fresh"]);
    expect(headers(calls[1])["X-Link-Secret"]).toBe(SECRET);
  });
});
