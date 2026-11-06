// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { resetClientForTests } from "./client";
import { REFERENCE_CACHE_KEY, loadCachedReference, refreshReference } from "./reference";

const DATA = { regions: [], factions: {}, servers: [], instances: [], classes: [], roles: [] };

beforeEach(() => {
  resetClientForTests();
  localStorage.clear();
});
afterEach(() => {
  vi.unstubAllGlobals();
  localStorage.clear();
});

describe("reference cache", () => {
  it("stores the reference with its ETag and time on first visit", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => new Response(JSON.stringify(DATA), { status: 200, headers: { ETag: '"e1"' } })),
    );
    expect(loadCachedReference()).toBeNull();
    const r = await refreshReference(() => new Date("2026-11-06T20:04:00Z"));
    expect(r.etag).toBe('"e1"');
    expect(JSON.parse(localStorage.getItem(REFERENCE_CACHE_KEY)!)).toEqual({
      etag: '"e1"',
      data: DATA,
      fetchedAt: "2026-11-06T20:04:00.000Z",
    });
  });

  it("revalidates a returning visitor's cache with If-None-Match", async () => {
    localStorage.setItem(
      REFERENCE_CACHE_KEY,
      JSON.stringify({ etag: '"e1"', data: DATA, fetchedAt: "2026-11-05T10:00:00Z" }),
    );
    const fetch = vi.fn(async (_url: string, _init?: RequestInit) => new Response(null, { status: 304 }));
    vi.stubGlobal("fetch", fetch);
    expect(loadCachedReference()?.etag).toBe('"e1"');
    const r = await refreshReference(() => new Date("2026-11-06T20:04:00Z"));
    expect((fetch.mock.calls[0][1]!.headers as Record<string, string>)["If-None-Match"]).toBe('"e1"');
    expect(r.data).toEqual(DATA);
    expect(r.fetchedAt).toBe("2026-11-06T20:04:00.000Z");
    expect(JSON.parse(localStorage.getItem(REFERENCE_CACHE_KEY)!)).toEqual({
      etag: '"e1"',
      data: DATA,
      fetchedAt: "2026-11-06T20:04:00.000Z",
    });
  });

  it("ignores a malformed cache", () => {
    const fetchedAt = "2026-11-05T10:00:00Z";
    for (const bad of [
      { etag: 1 },
      { etag: '"e"', fetchedAt, data: {} },
      { etag: '"e"', fetchedAt, data: { ...DATA, servers: null } },
      { etag: '"e"', fetchedAt, data: { ...DATA, factions: [] } },
      { etag: '"e"', fetchedAt: "not a date", data: DATA },
    ]) {
      localStorage.setItem(REFERENCE_CACHE_KEY, JSON.stringify(bad));
      expect(loadCachedReference(), JSON.stringify(bad)).toBeNull();
    }
  });
});
