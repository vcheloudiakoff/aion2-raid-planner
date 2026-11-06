// @vitest-environment happy-dom
import { act } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";
import { resetClientForTests } from "./api/client";
import { REFERENCE_CACHE_KEY } from "./api/reference";
import { resetLangForTests } from "./i18n";
import { byText, mount, type Mounted } from "./test/dom";

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
  it("renders the empty state at once, before the API answers, without a create button", () => {
    lang("fr-FR");
    vi.stubGlobal("fetch", vi.fn(() => new Promise(() => {})));
    m = mount(<App />);
    expect(byText(m.container, "Aucun raid ouvert pour l'instant.")).toBeTruthy();
    expect(m.container.textContent).not.toMatch(/Créer/);
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
