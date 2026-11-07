// @vitest-environment happy-dom
// Onglet Salle dans la page `/o/:secret` (story 3-4) : chunk chargé à la demande à travers
// `Suspense`, message Discord copié sans le secret du lien, « Régénérer le nom » envoyé par
// `command` (le groupe dans le corps, jamais dans l'URL), un groupe qui n'est plus retenu traité comme
// un refus d'état (feuille fermée, vue relue), une autre erreur gardée dans la feuille. Fuseau fixé
// avant tout calcul de date : UTC+9, sans heure d'été.
process.env.TZ = "Asia/Tokyo";

import { act } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter, Route, Routes } from "react-router";
import { resetClientForTests } from "../api/client";
import { saveOrganizerLinks } from "../api/links";
import type { OrganizerEvent } from "../api/types";
import { resetLangForTests } from "../i18n";
import { click, mount, type Mounted } from "../test/dom";
import { EVENT_ID, JOIN, KAEL, ORGANIZER, organizerEvent, organizerGroup, reference } from "../test/fixtures";
import { OrganizerPage } from "./OrganizerPage";

let m: Mounted;
type Call = { url: string; init?: RequestInit };

const json = (body: unknown, status = 200) =>
  new Response(JSON.stringify(body), { status, headers: { "Content-Type": "application/json" } });
const problem = (status: number, code: string, fields?: Record<string, string>) =>
  new Response(JSON.stringify({ type: "about:blank", title: "x", status, code, ...(fields && { fields }) }), {
    status,
    headers: { "Content-Type": "application/problem+json" },
  });

/** `fetch` simulé : la lecture rend toujours `view()`, les POST répondent dans l'ordre. */
function serve(view: () => Response, ...answers: (() => Response)[]) {
  const calls: Call[] = [];
  vi.stubGlobal(
    "fetch",
    vi.fn(async (url: string, init?: RequestInit) => {
      calls.push({ url, init });
      if (init?.method === "POST") {
        const next = answers.shift();
        if (next) return next();
        throw new Error(`unexpected POST ${url}`);
      }
      if (url === "/api/organizer") return view();
      throw new Error(`unexpected fetch ${url}`);
    }),
  );
  return calls;
}

const posts = (calls: Call[]) => calls.filter((c) => c.init?.method === "POST");
const reads = (calls: Call[]) => calls.filter((c) => c.url === "/api/organizer").length;

/** Raid verrouillé à un groupe G1 sur `slot_a`, Kael titulaire, nommé `roomName`. */
function locked(roomName = "I1-K7Q2"): OrganizerEvent {
  return {
    ...organizerEvent,
    event: { ...organizerEvent.event, status: "locked" },
    groups: [
      organizerGroup({
        id: "g1",
        roomName,
        starters: [{ candidatureId: KAEL, party: 1, index: 1, unavailable: false }],
      }),
    ],
  };
}

async function flush() {
  await act(async () => {
    for (let i = 0; i < 10; i++) await Promise.resolve();
  });
}

async function render() {
  m = mount(
    <MemoryRouter initialEntries={[`/o/${ORGANIZER}`]}>
      <Routes>
        <Route path="/o/:secret" element={<OrganizerPage reference={reference} />} />
      </Routes>
    </MemoryRouter>,
  );
  await flush();
}

const q = <T extends Element>(sel: string) => document.querySelector<T>(sel);
const tab = (label: string) =>
  [...m.container.querySelectorAll<HTMLButtonElement>('[role="tab"]')].find((b) => b.textContent === label)!;
const dialog = () => q<HTMLElement>('[role="dialog"]');
const inDialog = (label: string) =>
  [...(dialog()?.querySelectorAll<HTMLButtonElement>("button") ?? [])].find((b) => b.textContent === label)!;
const roomName = () => q('[data-testid="room-card-g1"] [data-variant="display"]')!.textContent;

/** Onglet Salle ouvert, module paresseux chargé. */
async function openRoom() {
  click(tab("Salle"));
  await act(async () => {
    await vi.dynamicImportSettled();
  });
  await flush();
}

async function press(b: Element | null) {
  await act(async () => (b as HTMLElement).click());
  await flush();
}

/** « … » puis « Régénérer le nom » sur G1 : la confirmation s'ouvre. */
function askRegenerate() {
  click(q('[data-testid="room-more-g1"]')!);
  click(q('[data-testid="room-regenerate-g1"]')!);
}

let writeText: ReturnType<typeof vi.fn>;

beforeEach(() => {
  localStorage.clear();
  resetClientForTests();
  vi.spyOn(navigator, "language", "get").mockReturnValue("fr-FR");
  resetLangForTests();
  writeText = vi.fn(async () => {});
  Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true, writable: true });
});

afterEach(() => {
  m.unmount();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  vi.useRealTimers();
  localStorage.clear();
});

describe("room tab", () => {
  it("loads its chunk on demand, through Suspense with a skeleton", async () => {
    serve(() => json(locked()));
    await render();
    expect(q('[data-testid="room"]')).toBeNull();
    click(tab("Salle"));
    // Le module n'est pas encore là : le squelette tient sa place.
    expect(q('[data-testid="room-loading"]')).not.toBeNull();
    expect(q('[data-testid="room"]')).toBeNull();
    await act(async () => {
      await vi.dynamicImportSettled();
    });
    await flush();
    expect(q('[data-testid="room-loading"]')).toBeNull();
    expect(roomName()).toBe("I1-K7Q2");
  });

  it("copies the Discord message of /o/<secret> without the secret, with the public link", async () => {
    // Le lien direct est connu de cet appareil : il ne doit pas fuir non plus.
    expect(saveOrganizerLinks(EVENT_ID, { organizer: ORGANIZER, join: JOIN })).toBe(true);
    serve(() => json(locked()));
    await render();
    await openRoom();
    await press(q('[data-testid="room-copy-discord"]'));
    expect(writeText).toHaveBeenCalledTimes(1);
    const text = String(writeText.mock.calls[0][0]);
    expect(text).toContain(`${window.location.origin}/e/${EVENT_ID}`);
    expect(text).toContain("<t:1894132800:F>");
    expect(text).toContain("Salle : I1-K7Q2");
    expect(text).toContain("1. Kael[SE] · DPS");
    for (const leak of [ORGANIZER, JOIN, "/o/", "/j/", "/p/"]) expect(text).not.toContain(leak);
  });

  it("regenerates the name through the organizer command, the group in the body, then shows it", async () => {
    let current = locked();
    const calls = serve(
      () => json(current),
      () => {
        current = locked("I1-NEW2");
        return json(current);
      },
    );
    await render();
    await openRoom();
    askRegenerate();
    expect(posts(calls)).toEqual([]);
    await press(inDialog("Régénérer"));
    expect(posts(calls).map((c) => c.url)).toEqual(["/api/organizer/room-name"]);
    const [call] = posts(calls);
    expect(JSON.parse(String(call.init?.body))).toEqual({ groupId: "g1" });
    expect((call.init?.headers as Record<string, string>)["X-Link-Secret"]).toBe(ORGANIZER);
    expect(call.url).not.toContain("g1");
    expect(dialog()).toBeNull();
    expect(roomName()).toBe("I1-NEW2");
  });

  it("treats an unknown group as a state refusal: sheet closed, message shown, view re-read", async () => {
    const calls = serve(
      () => json(locked()),
      () => problem(400, "placement.unknown_group", { groupId: "placement.unknown_group" }),
    );
    await render();
    await openRoom();
    const before = reads(calls);
    askRegenerate();
    await press(inDialog("Régénérer"));
    expect(dialog()).toBeNull();
    expect(q('[data-testid="action-error"]')!.textContent).toBe("Ce groupe n'est plus retenu.");
    expect(reads(calls)).toBe(before + 1);
  });

  it("treats a raid back in preparation as a state refusal too", async () => {
    const calls = serve(
      () => json(locked()),
      () => problem(409, "event.not_locked"),
    );
    await render();
    await openRoom();
    const before = reads(calls);
    askRegenerate();
    await press(inDialog("Régénérer"));
    expect(dialog()).toBeNull();
    expect(q('[data-testid="action-error"]')).not.toBeNull();
    expect(reads(calls)).toBe(before + 1);
  });

  it("keeps the sheet open with the error on a quota refusal", async () => {
    serve(
      () => json(locked()),
      () => problem(429, "limit.rate_limited"),
    );
    await render();
    await openRoom();
    askRegenerate();
    await press(inDialog("Régénérer"));
    expect(dialog()).not.toBeNull();
    expect(dialog()!.querySelector('[role="alert"]')!.textContent).not.toBe("");
    expect(q('[data-testid="action-error"]')).toBeNull();
    expect(roomName()).toBe("I1-K7Q2");
  });

  it("says in Salle that an unsaved composition draft is not in the message", async () => {
    serve(() => json(locked()));
    await render();
    await openRoom();
    expect(q('[data-testid="room-draft"]')).toBeNull();
    // Un geste dans Composition ouvre le brouillon : Kael retiré du slot 1.
    click(tab("Composition"));
    await act(async () => {
      await vi.dynamicImportSettled();
    });
    await flush();
    click(q('[data-testid="slot-1-1"]')!);
    click(inDialog("Retirer"));
    expect(q('[data-testid="composition-bar"]')).not.toBeNull();
    click(tab("Salle"));
    await flush();
    expect(q('[data-testid="room-draft"]')!.textContent).toBe(
      "Ton brouillon de composition n'est pas enregistré : ce message reprend la dernière version enregistrée.",
    );
    // Le message reprend le placement enregistré : Kael y est toujours.
    await press(q('[data-testid="room-copy-discord"]'));
    expect(String(writeText.mock.calls[0][0])).toContain("1. Kael[SE] · DPS");
  });
});
