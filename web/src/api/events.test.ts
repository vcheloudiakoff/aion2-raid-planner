import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError, resetClientForTests } from "./client";
import { EVENTS_URL, MIN_SUBMIT_DELAY_MS, createEvent, getFormToken, type CreateDeps } from "./events";
import type { CreateEvent, CreatedEvent } from "./types";

const input: CreateEvent = {
  instance: "i1",
  region: "r1",
  factionScope: "all",
  slots: [{ startsAt: "2026-11-11T20:00:00Z" }],
};

const created: CreatedEvent = {
  eventId: "e".repeat(22),
  organizerSecret: "o".repeat(43),
  joinSecret: "j".repeat(43),
  links: { organizer: `/o/${"o".repeat(43)}`, join: `/j/${"j".repeat(43)}`, public: `/e/${"e".repeat(22)}` },
};

const problem = (status: number, code: string, fields?: Record<string, string>) =>
  new Response(JSON.stringify({ type: "about:blank", title: "x", status, code, fields }), {
    status,
    headers: { "Content-Type": "application/problem+json" },
  });
const ok = () => new Response(JSON.stringify(created), { status: 201, headers: { "Content-Type": "application/json" } });

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

function deps(): CreateDeps & { renewToken: ReturnType<typeof vi.fn>; sleep: ReturnType<typeof vi.fn> } {
  return { renewToken: vi.fn(async () => "fresh"), sleep: vi.fn(async () => {}) };
}

const sent = (c: { init?: RequestInit }) => JSON.parse(String(c.init?.body)) as Record<string, unknown>;

beforeEach(() => resetClientForTests());
afterEach(() => vi.unstubAllGlobals());

describe("createEvent", () => {
  it("posts the form with its token and an empty honeypot", async () => {
    const calls = serve(ok());
    const d = deps();
    expect(await createEvent(input, "tok", "", d)).toEqual(created);
    expect(calls[0].url).toBe(EVENTS_URL);
    expect(calls[0].init?.method).toBe("POST");
    expect(sent(calls[0])).toEqual({ ...input, formToken: "tok", website: "" });
    expect(d.renewToken).not.toHaveBeenCalled();
  });

  it("renews the token once on form.token_invalid, waits, and retries silently", async () => {
    const calls = serve(problem(400, "form.token_invalid"), ok());
    const d = deps();
    expect(await createEvent(input, "old", "", d)).toEqual(created);
    expect(d.renewToken).toHaveBeenCalledTimes(1);
    expect(d.sleep).toHaveBeenCalledWith(MIN_SUBMIT_DELAY_MS);
    expect(calls.map((c) => sent(c).formToken)).toEqual(["old", "fresh"]);
  });

  it("gives up after a second form.token_invalid", async () => {
    serve(problem(400, "form.token_invalid"), problem(400, "form.token_invalid"));
    const d = deps();
    await expect(createEvent(input, "old", "", d)).rejects.toMatchObject({ code: "form.token_invalid" });
    expect(d.renewToken).toHaveBeenCalledTimes(1);
  });

  it("waits MIN_SUBMIT_DELAY once on form.too_fast, with the same token", async () => {
    const calls = serve(problem(400, "form.too_fast"), ok());
    const d = deps();
    expect(await createEvent(input, "young", "", d)).toEqual(created);
    expect(d.sleep).toHaveBeenCalledTimes(1);
    expect(d.sleep).toHaveBeenCalledWith(3000);
    expect(calls.map((c) => sent(c).formToken)).toEqual(["young", "young"]);

    serve(problem(400, "form.too_fast"), problem(400, "form.too_fast"));
    await expect(createEvent(input, "young", "", deps())).rejects.toMatchObject({ code: "form.too_fast" });
  });

  it("surfaces any other error with its fields, without retrying", async () => {
    const calls = serve(problem(400, "slot.in_past", { "slots.0.startsAt": "slot.in_past" }));
    const d = deps();
    const err = await createEvent(input, "tok", "", d).catch((e: unknown) => e);
    expect(err).toBeInstanceOf(ApiError);
    expect((err as ApiError).code).toBe("slot.in_past");
    expect((err as ApiError).fields).toEqual({ "slots.0.startsAt": "slot.in_past" });
    expect(calls).toHaveLength(1);
    expect(d.sleep).not.toHaveBeenCalled();
  });
});

describe("getFormToken", () => {
  it("asks for a token of the given class", async () => {
    const calls = serve(new Response(JSON.stringify({ token: "t1" }), { status: 200, headers: { "Content-Type": "application/json" } }));
    expect(await getFormToken("create")).toBe("t1");
    expect(calls[0].url).toBe("/api/form-token?class=create");
  });
});
