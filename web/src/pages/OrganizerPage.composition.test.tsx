// @vitest-environment happy-dom
// Onglet Composition dans la page `/o/:secret` (story 3-3) : chunk chargé à la demande à travers
// `Suspense`, brouillon tenu par la page (créé au premier geste, gardé d'un onglet à l'autre, jamais
// écrasé par une relecture), « Enregistrer » qui envoie le placement complet, « Abandonner » confirmé,
// bandeau des changements, refus structurel qui garde le brouillon. Le test du chargement à la
// demande vient en premier : le module paresseux n'est chargé qu'une fois par fichier. Fuseau fixé
// avant tout calcul de date : UTC+9, sans heure d'été.
process.env.TZ = "Asia/Tokyo";

import { act } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter, Route, Routes } from "react-router";
import { resetClientForTests } from "../api/client";
import type { OrganizerCandidature, OrganizerEvent } from "../api/types";
import { resetLangForTests } from "../i18n";
import { click, mount, type Mounted } from "../test/dom";
import { KAEL, NYRA_TWIN, ORGANIZER, organizerEvent, organizerGroup, reference } from "../test/fixtures";
import { REFRESH_MS } from "./EventPage";
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

/** Raid verrouillé à un groupe G1 sur `slot_a` ; Kael et nyra `accepted`. */
function locked(
  patch: Record<string, Partial<OrganizerCandidature>> = {},
  starters: OrganizerEvent["groups"][number]["starters"] = [],
): OrganizerEvent {
  return {
    ...organizerEvent,
    event: { ...organizerEvent.event, status: "locked" },
    groups: [organizerGroup({ id: "g1", starters })],
    candidatures: organizerEvent.candidatures.map((c) => ({ ...c, ...patch[c.id] })),
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
const slot = (party: number, index: number) => q<HTMLButtonElement>(`[data-testid="slot-${party}-${index}"]`)!;
const bar = () => q<HTMLElement>('[data-testid="composition-bar"]');
const inDialog = (label: string) =>
  [...(q('[role="dialog"]')?.querySelectorAll<HTMLButtonElement>("button") ?? [])].find((b) => b.textContent === label)!;

/** Onglet Composition ouvert, module paresseux chargé. */
async function openComposition() {
  click(tab("Composition"));
  await act(async () => {
    await vi.dynamicImportSettled();
  });
  await flush();
}

/** Kael sur le slot 1 de l'Équipe 1 : le premier geste. */
function placeKael() {
  click(slot(1, 1));
  click(q(`[data-testid="reserve-${KAEL}"]`)!);
}

async function press(b: HTMLElement | null) {
  await act(async () => b!.click());
  await flush();
}

async function tick() {
  await act(async () => {
    vi.advanceTimersByTime(REFRESH_MS);
  });
  await flush();
}

beforeEach(() => {
  localStorage.clear();
  resetClientForTests();
  vi.spyOn(navigator, "language", "get").mockReturnValue("fr-FR");
  resetLangForTests();
});

afterEach(() => {
  m.unmount();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  vi.useRealTimers();
  localStorage.clear();
});

describe("composition tab", () => {
  it("loads its chunk on demand, through Suspense with a skeleton", async () => {
    serve(() => json(locked()));
    await render();
    expect(q('[data-testid="composition"]')).toBeNull();
    click(tab("Composition"));
    // Le module n'est pas encore là : le squelette tient sa place.
    expect(q('[data-testid="composition-loading"]')).not.toBeNull();
    expect(q('[data-testid="composition"]')).toBeNull();
    await act(async () => {
      await vi.dynamicImportSettled();
    });
    await flush();
    expect(q('[data-testid="composition-loading"]')).toBeNull();
    expect(q('[data-testid="composition"]')).not.toBeNull();
    expect(q('[data-testid="group-chip-g1"]')!.textContent).toBe("G1 · mer. 05:00");
  });

  it("opens a draft at the first change and sends nothing before Enregistrer, then the full placement", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval", "setTimeout", "clearTimeout"] });
    const saved = locked({}, [{ candidatureId: KAEL, party: 1, index: 1, unavailable: false }]);
    const calls = serve(() => json(locked()), () => json(saved));
    await render();
    await openComposition();
    expect(bar()).toBeNull();
    placeKael();
    expect(bar()).not.toBeNull();
    expect(slot(1, 1).textContent).toContain("Kael[SE]");
    // Bench, puis une relecture : toujours rien d'envoyé, et le brouillon tient.
    click(q('[data-testid="bench-add"]')!);
    // nyra n'a coché que l'autre créneau : repliée, et placée quand même.
    click(q('[data-testid="reserve-unavailable-toggle"]')!);
    click(q(`[data-testid="reserve-${NYRA_TWIN}"]`)!);
    await tick();
    expect(posts(calls)).toEqual([]);
    expect(slot(1, 1).textContent).toContain("Kael[SE]");
    await press(q('[data-testid="composition-save"]'));
    expect(posts(calls).map((c) => c.url)).toEqual(["/api/organizer/placement"]);
    expect(JSON.parse(String(posts(calls)[0].init?.body))).toEqual({
      placements: [
        { candidatureId: KAEL, groupId: "g1", party: 1, index: 1 },
        { candidatureId: NYRA_TWIN, groupId: "g1", benchIndex: 1 },
      ],
    });
    // Enregistré : plus de brouillon, la vue rendue s'affiche.
    expect(bar()).toBeNull();
    expect(slot(1, 1).textContent).toContain("Kael[SE]");
    expect(q('[data-testid="bench-1"]')).toBeNull();
  });

  it("asks before Abandonner, and keeps the draft on Garder", async () => {
    const calls = serve(() => json(locked()));
    await render();
    await openComposition();
    placeKael();
    click(q('[data-testid="composition-discard"]')!);
    expect(q('[role="dialog"] p')!.textContent).toBe(
      "Abandonner le brouillon ? Tes changements non enregistrés seront perdus.",
    );
    click(inDialog("Garder le brouillon"));
    expect(bar()).not.toBeNull();
    expect(slot(1, 1).textContent).toContain("Kael[SE]");
    click(q('[data-testid="composition-discard"]')!);
    click(inDialog("Abandonner"));
    expect(bar()).toBeNull();
    expect(slot(1, 1).textContent).toBe("1libre");
    expect(posts(calls)).toEqual([]);
  });

  it("warns that signups changed without ever overwriting the draft; Voir reloads after confirming", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval", "setTimeout", "clearTimeout"] });
    let current = locked();
    const calls = serve(() => json(current));
    await render();
    await openComposition();
    placeKael();
    expect(q('[data-testid="composition-changed"]')).toBeNull();
    // Kael change de CP, nyra est refusée, et le serveur place nyra ailleurs.
    current = locked({ [KAEL]: { cp: 2400 }, [NYRA_TWIN]: { status: "rejected" } });
    await tick();
    expect(q('[data-testid="composition-changed"] p')!.textContent).toBe(
      "2 candidatures ont changé depuis ton brouillon",
    );
    // Le brouillon tient, « Enregistrer » reste possible.
    expect(slot(1, 1).textContent).toContain("Kael[SE]");
    expect(q<HTMLButtonElement>('[data-testid="composition-save"]')!.disabled).toBe(false);
    current = locked({ [KAEL]: { cp: 2400 } }, [{ candidatureId: NYRA_TWIN, party: 2, index: 1, unavailable: true }]);
    await tick();
    expect(q('[data-testid="composition-changed"] p')!.textContent).toBe(
      "2 candidatures ont changé depuis ton brouillon",
    );
    expect(slot(2, 1).textContent).toBe("1libre");
    // « Voir » : confirmé, il abandonne le brouillon et relit.
    const before = reads(calls);
    click(q('[data-testid="composition-see"]')!);
    expect(q('[role="dialog"] p')!.textContent).toBe("Recharger les données ? Ton brouillon sera abandonné.");
    await press(inDialog("Recharger"));
    expect(reads(calls)).toBe(before + 1);
    expect(bar()).toBeNull();
    expect(q('[data-testid="composition-changed"]')).toBeNull();
    expect(slot(1, 1).textContent).toBe("1libre");
    expect(slot(2, 1).textContent).toContain("nyra[SA]");
    expect(posts(calls)).toEqual([]);
  });

  it("keeps the draft on a structural 400 and shows why", async () => {
    const calls = serve(
      () => json(locked()),
      () =>
        problem(400, "placement.candidature_not_placeable", {
          "placements.0.candidatureId": "placement.candidature_not_placeable",
        }),
    );
    await render();
    await openComposition();
    placeKael();
    await press(q('[data-testid="composition-save"]'));
    expect(posts(calls)).toHaveLength(1);
    expect(q('[data-testid="composition-error"]')!.textContent).toBe(
      "Un joueur placé n'est plus accepté dans ce raid.",
    );
    expect(bar()).not.toBeNull();
    expect(slot(1, 1).textContent).toContain("Kael[SE]");
  });

  it("keeps the draft when the organizer switches tabs", async () => {
    serve(() => json(locked()));
    await render();
    await openComposition();
    placeKael();
    click(tab("Candidatures"));
    expect(q('[data-testid="composition"]')).toBeNull();
    click(tab("Composition"));
    await flush();
    expect(bar()).not.toBeNull();
    expect(slot(1, 1).textContent).toContain("Kael[SE]");
  });

  it("drops the draft with a notice when the raid is unlocked or its retained groups change", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval", "setTimeout", "clearTimeout"] });
    for (const changed of [
      // Déverrouillé ailleurs.
      { ...locked(), event: { ...locked().event, status: "open" as const } },
      // Re-verrouillé sur d'autres groupes : G1 en sommeil, un groupe neuf.
      { ...locked(), groups: [organizerGroup({ id: "g1", retained: false }), organizerGroup({ id: "g9", slotId: "slot_b" })] },
    ]) {
      let current = locked();
      const calls = serve(() => json(current));
      await render();
      await openComposition();
      placeKael();
      expect(q('[data-testid="draft-dropped"]')).toBeNull();
      current = changed;
      await tick();
      expect(q('[data-testid="draft-dropped"]')!.textContent).toBe(
        "Le raid a changé (déverrouillé ou groupes modifiés) : ton brouillon de composition a été abandonné.",
      );
      expect(bar()).toBeNull();
      expect(posts(calls)).toEqual([]);
      if (changed.event.status === "locked") {
        // Le nouveau groupe s'affiche sans le brouillon ; un nouveau geste efface l'avis.
        expect(q('[data-testid="group-chip-g9"]')!.textContent).toBe("G1 · ven. 05:00");
        expect(slot(1, 1).textContent).toBe("1libre");
        placeKael();
        expect(q('[data-testid="draft-dropped"]')).toBeNull();
      }
      m.unmount();
    }
    m = mount(<div />);
  });

  it("points at the refused seats inline with the error", async () => {
    serve(
      () => json(locked()),
      () =>
        problem(400, "placement.candidature_not_placeable", {
          "placements.1.candidatureId": "placement.candidature_not_placeable",
        }),
    );
    await render();
    await openComposition();
    placeKael();
    click(q('[data-testid="bench-add"]')!);
    click(q('[data-testid="reserve-unavailable-toggle"]')!);
    click(q(`[data-testid="reserve-${NYRA_TWIN}"]`)!);
    await press(q('[data-testid="composition-save"]'));
    // Le corps envoyé : Kael (0) puis nyra au banc (1) ; seule nyra est désignée.
    const message = "Un joueur placé n'est plus accepté dans ce raid.";
    expect(q('[data-testid="bench-1"] [data-testid="slot-error"]')!.textContent).toBe(message);
    expect(q('[data-testid="bench-1"]')!.getAttribute("aria-label")).toContain(message);
    expect(slot(1, 1).querySelector('[data-testid="slot-error"]')).toBeNull();
    // Un nouveau geste efface la désignation, devenue fausse.
    click(slot(1, 1));
    click(q('[data-testid="action-bench"]')!);
    expect(q('[data-testid="slot-error"]')).toBeNull();
  });

  it("keeps the draft and shows the message when Save fails on a state code or the network", async () => {
    for (const [answer, where, message] of [
      [() => problem(409, "event.not_locked"), '[data-testid="action-error"]', "Le raid n'est plus verrouillé."],
      [
        () => {
          throw new TypeError("network down");
        },
        '[data-testid="composition-error"]',
        "Ça n'a pas marché. Réessaie dans un moment.",
      ],
    ] as const) {
      // La relecture qui suit un refus d'état dit encore `locked` : le brouillon reste.
      const calls = serve(() => json(locked()), answer);
      await render();
      await openComposition();
      placeKael();
      await press(q('[data-testid="composition-save"]'));
      expect(posts(calls), message).toHaveLength(1);
      expect(q(where)?.textContent, message).toBe(message);
      expect(bar(), message).not.toBeNull();
      expect(slot(1, 1).textContent, message).toContain("Kael[SE]");
      m.unmount();
    }
    m = mount(<div />);
  });
});
