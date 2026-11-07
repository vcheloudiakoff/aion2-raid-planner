// @vitest-environment happy-dom
import { act } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";
import { resetClientForTests } from "./api/client";
import { REFERENCE_CACHE_KEY } from "./api/reference";
import { resetLangForTests } from "./i18n";
import { byText, mount, type Mounted } from "./test/dom";
import { EVENT_ID, JOIN, ORGANIZER, PARTICIPANT, publicEvent, reference } from "./test/fixtures";

let m: Mounted | null = null;

async function flush() {
  await act(async () => {
    for (let i = 0; i < 5; i++) await Promise.resolve();
  });
}

beforeEach(() => {
  localStorage.clear();
  resetClientForTests();
});

afterEach(() => {
  m?.unmount();
  m = null;
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  localStorage.clear();
});

function lang(tag: string) {
  vi.spyOn(navigator, "language", "get").mockReturnValue(tag);
  resetLangForTests();
}

describe("home", () => {
  it("renders the empty state at once, before the API answers, with a create button", () => {
    lang("fr-FR");
    vi.stubGlobal("fetch", vi.fn(() => new Promise(() => {})));
    m = mount(<App />);
    expect(byText(m.container, "Aucun raid ouvert pour l'instant.")).toBeTruthy();
    expect(byText(m.container, "Créer un raid").getAttribute("href")).toBe("/new");
    expect(m.container.textContent).not.toMatch(/Sur cet appareil/);
  });

  it("is in English for an en-US browser", () => {
    lang("en-US");
    vi.stubGlobal("fetch", vi.fn(() => new Promise(() => {})));
    m = mount(<App />);
    expect(document.documentElement.lang).toBe("en");
    expect(byText(m.container, "No raids open right now.")).toBeTruthy();
  });

  it("shows the offline banner with the cache time when the server is unreachable", async () => {
    lang("fr-FR");
    const fetchedAt = "2026-11-06T20:04:00Z";
    localStorage.setItem(
      REFERENCE_CACHE_KEY,
      JSON.stringify({
        etag: '"e"',
        data: { regions: [], factions: {}, servers: [], instances: [], classes: [], roles: [] },
        fetchedAt,
      }),
    );
    vi.stubGlobal("fetch", vi.fn(async () => Promise.reject(new TypeError("Failed to fetch"))));
    m = mount(<App />);
    await flush();
    const hhmm = new Intl.DateTimeFormat("fr", { hour: "2-digit", minute: "2-digit", hourCycle: "h23" }).format(
      new Date(fetchedAt),
    );
    expect(byText(m.container, `Hors ligne · dernières données de ${hhmm}`)).toBeTruthy();
    expect(byText(m.container, "Aucun raid ouvert pour l'instant.")).toBeTruthy();
  });

  it("shows the offline banner without time when there is no cache, and retries on online", async () => {
    lang("fr-FR");
    const fetch = vi.fn(async () => Promise.reject(new TypeError("Failed to fetch")));
    vi.stubGlobal("fetch", fetch);
    m = mount(<App />);
    await flush();
    expect(byText(m.container, "Hors ligne")).toBeTruthy();
    const before = fetch.mock.calls.length;
    await act(async () => {
      window.dispatchEvent(new Event("online"));
    });
    await flush();
    expect(fetch.mock.calls.length).toBe(before + 1);
  });

  it("retries when the page becomes visible, not when it is hidden", async () => {
    lang("fr-FR");
    const fetch = vi.fn(async () => Promise.reject(new TypeError("Failed to fetch")));
    vi.stubGlobal("fetch", fetch);
    m = mount(<App />);
    await flush();
    const visibility = vi.spyOn(document, "visibilityState", "get");
    const change = async (state: DocumentVisibilityState) => {
      visibility.mockReturnValue(state);
      await act(async () => {
        document.dispatchEvent(new Event("visibilitychange"));
      });
      await flush();
    };
    const before = fetch.mock.calls.length;
    await change("hidden");
    expect(fetch.mock.calls.length).toBe(before);
    await change("visible");
    expect(fetch.mock.calls.length).toBe(before + 1);
  });

  it("shows the new-version banner on a build change, and reloads only when asked", async () => {
    lang("en-US");
    let build = "b1";
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => new Response("{}", { status: 200, headers: { ETag: '"e"', "X-App-Build": build } })),
    );
    m = mount(<App />);
    await flush();
    expect(m.container.textContent).not.toMatch(/New version/);
    build = "b2";
    await act(async () => {
      window.dispatchEvent(new Event("online"));
    });
    await flush();
    expect(byText(m.container, "New version")).toBeTruthy();
    expect(byText(m.container, "Reload").tagName).toBe("BUTTON");
  });
});

describe("routes", () => {
  afterEach(() => window.history.pushState({}, "", "/"));

  it("serves the create form on /new with game labels from the cached reference", () => {
    lang("fr-FR");
    localStorage.setItem(
      REFERENCE_CACHE_KEY,
      JSON.stringify({ etag: '"e"', data: reference, fetchedAt: "2026-11-06T20:04:00Z" }),
    );
    vi.stubGlobal("fetch", vi.fn(() => new Promise(() => {})));
    window.history.pushState({}, "", "/new");
    m = mount(<App />);
    expect(byText(m.container, "Créer un raid").tagName).toBe("H1");
    const options = [...m.container.querySelectorAll("option")].map((o) => o.textContent);
    expect(options).toEqual(["Instance Une", "Mono", "Région Une"]);
    expect(m.container.textContent).toContain("Classe Soin");
  });

  it("serves the pending organizer page on /o/<secret>, secret shown nowhere", () => {
    lang("fr-FR");
    vi.stubGlobal("fetch", vi.fn(() => new Promise(() => {})));
    window.history.pushState({}, "", `/o/${ORGANIZER}`);
    m = mount(<App />);
    expect(byText(m.container, "Ton raid est publié")).toBeTruthy();
    expect(m.container.textContent).not.toContain(ORGANIZER);
    expect(document.title).not.toContain(ORGANIZER);
  });

  it("serves the event page on /e/<id> and the direct form on /j/<secret>, secret out of the title", async () => {
    lang("fr-FR");
    localStorage.setItem(
      REFERENCE_CACHE_KEY,
      JSON.stringify({ etag: '"e"', data: reference, fetchedAt: "2026-11-06T20:04:00Z" }),
    );
    const fetch = vi.fn(async (url: string) => {
      if (url === `/api/events/${EVENT_ID}` || url === "/api/join") {
        return new Response(JSON.stringify(publicEvent), { status: 200, headers: { "Content-Type": "application/json" } });
      }
      return new Promise<Response>(() => {});
    });
    vi.stubGlobal("fetch", fetch);
    window.history.pushState({}, "", `/e/${EVENT_ID}`);
    m = mount(<App />);
    await flush();
    expect(m.container.querySelector("h1")?.textContent).toMatch(/^Instance Une · /);
    expect(m.container.textContent).not.toContain("Lien direct");
    m.unmount();

    window.history.pushState({}, "", `/j/${JOIN}`);
    m = mount(<App />);
    await flush();
    expect(m.container.textContent).toContain("Lien direct : ta candidature est acceptée d'office.");
    expect(m.container.textContent).not.toContain(JOIN);
    expect(document.title).not.toContain(JOIN);
    expect(fetch.mock.calls.some(([url]) => String(url).includes(JOIN))).toBe(false);
  });

  it("serves the pending participant page on /p/<secret>, secret out of the title", () => {
    lang("fr-FR");
    vi.stubGlobal("fetch", vi.fn(() => new Promise(() => {})));
    window.history.pushState({}, "", `/p/${PARTICIPANT}`);
    m = mount(<App />);
    expect(m.container.querySelector("h1")?.textContent).toBe("Ton lien personnel");
    expect(m.container.textContent).toContain(`/p/${PARTICIPANT}`);
    expect(document.title).not.toContain(PARTICIPANT);
  });
});
