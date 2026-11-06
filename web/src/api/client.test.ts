import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError, TIMEOUT_MS, getConditional, getJson, getStatus, remember, resetClientForTests } from "./client";

type Reply = { status: number; headers?: Record<string, string>; body?: unknown };

function serve(...replies: (Reply | Error)[]) {
  const calls: { url: string; headers: Record<string, string> }[] = [];
  vi.stubGlobal(
    "fetch",
    vi.fn(async (url: string, init?: RequestInit) => {
      calls.push({ url, headers: { ...(init?.headers as Record<string, string>) } });
      const r = replies.shift();
      if (!r) throw new Error("unexpected fetch");
      if (r instanceof Error) throw r;
      const body = r.body === undefined ? null : JSON.stringify(r.body);
      return new Response(r.status === 304 ? null : body, { status: r.status, headers: r.headers });
    }),
  );
  return calls;
}

const json = { "Content-Type": "application/json" };

beforeEach(() => resetClientForTests());
afterEach(() => vi.unstubAllGlobals());

describe("getJson", () => {
  it("sends If-None-Match and returns the remembered body on 304", async () => {
    const calls = serve(
      { status: 200, headers: { ...json, ETag: '"v1"', "X-App-Build": "b1" }, body: { n: 1 } },
      { status: 304, headers: { ETag: '"v1"', "X-App-Build": "b1" } },
    );
    expect(await getJson("/api/x")).toEqual({ n: 1 });
    const second = await getConditional("/api/x");
    expect(second).toEqual({ body: { n: 1 }, etag: '"v1"', notModified: true });
    expect(calls[0].headers["If-None-Match"]).toBeUndefined();
    expect(calls[1].headers["If-None-Match"]).toBe('"v1"');
  });

  it("uses a primed cache on the first request", async () => {
    remember("/api/x", '"v0"', { n: 0 });
    const calls = serve({ status: 304, headers: { "X-App-Build": "b1" } });
    expect(await getJson("/api/x")).toEqual({ n: 0 });
    expect(calls[0].headers["If-None-Match"]).toBe('"v0"');
  });

  it("raises the new-version flag when X-App-Build changes, never on the first one", async () => {
    serve(
      { status: 200, headers: { ...json, "X-App-Build": "b1" }, body: 1 },
      { status: 200, headers: { ...json, "X-App-Build": "b1" }, body: 1 },
      { status: 200, headers: { ...json, "X-App-Build": "b2" }, body: 1 },
    );
    await getJson("/api/a");
    await getJson("/api/a");
    expect(getStatus().newBuild).toBe(false);
    await getJson("/api/a");
    expect(getStatus().newBuild).toBe(true);
  });

  it("flags offline on a rejected fetch, then clears it on the next answer", async () => {
    serve(new TypeError("Failed to fetch"), { status: 200, headers: json, body: 2 });
    const err = await getJson("/api/a").catch((e: unknown) => e);
    expect(err).toBeInstanceOf(ApiError);
    expect((err as ApiError).status).toBe(0);
    expect(getStatus().offline).toBe(true);
    expect(await getJson("/api/a")).toBe(2);
    expect(getStatus().offline).toBe(false);
  });

  it.each([502, 504])("treats a %i from the edge as unreachable", async (status) => {
    serve({ status });
    const err = (await getJson("/api/a").catch((e: unknown) => e)) as ApiError;
    expect(err).toBeInstanceOf(ApiError);
    expect(err.status).toBe(0);
    expect(getStatus().offline).toBe(true);
  });

  it("ends a hung request after the timeout as unreachable", async () => {
    // `AbortSignal.timeout` échappe aux faux timers de Node : le délai est déclenché à la main.
    const controller = new AbortController();
    const timeout = vi.spyOn(AbortSignal, "timeout").mockReturnValue(controller.signal);
    vi.stubGlobal(
      "fetch",
      vi.fn(
        (_url: string, init?: RequestInit) =>
          new Promise((_resolve, reject) => {
            init?.signal?.addEventListener("abort", () => reject(init.signal!.reason));
          }),
      ),
    );
    const pending = getJson("/api/a").catch((e: unknown) => e);
    expect(timeout).toHaveBeenCalledWith(TIMEOUT_MS);
    controller.abort(new DOMException("timed out", "TimeoutError"));
    const err = (await pending) as ApiError;
    expect(err).toBeInstanceOf(ApiError);
    expect(err.status).toBe(0);
    expect(getStatus().offline).toBe(true);
    timeout.mockRestore();
  });

  it.each(["", "not json"])("throws ApiError on a 200 whose body is %j", async (text) => {
    vi.stubGlobal("fetch", vi.fn(async () => new Response(text, { status: 200, headers: json })));
    const err = (await getJson("/api/a").catch((e: unknown) => e)) as ApiError;
    expect(err).toBeInstanceOf(ApiError);
    expect(err.status).toBe(200);
  });

  it("throws ApiError with the problem code", async () => {
    serve({
      status: 503,
      headers: { "Content-Type": "application/problem+json", "Retry-After": "60" },
      body: { type: "about:blank", title: "Service Unavailable", status: 503, code: "reference.unavailable" },
    });
    const err = (await getJson("/api/reference").catch((e: unknown) => e)) as ApiError;
    expect(err).toBeInstanceOf(ApiError);
    expect(err.status).toBe(503);
    expect(err.code).toBe("reference.unavailable");
    expect(getStatus().offline).toBe(false);
  });

  it("throws ApiError without code when the body is not problem+json", async () => {
    serve({ status: 500, headers: { "Content-Type": "text/plain" } });
    const err = (await getJson("/api/a").catch((e: unknown) => e)) as ApiError;
    expect(err.status).toBe(500);
    expect(err.code).toBeUndefined();
  });

  it("rejects a 304 with nothing remembered", async () => {
    serve({ status: 304 });
    const err = (await getJson("/api/a").catch((e: unknown) => e)) as ApiError;
    expect(err.status).toBe(304);
  });
});
