// @vitest-environment happy-dom
// Feuille Verrouiller (story 3-2), ouverte depuis l'onglet Disponibilités de la page `/o/:secret` :
// la commande passe par le séquencement et la relecture de la page, comme en vrai. Fuseau fixé avant
// tout calcul de date : UTC+9, sans heure d'été.
process.env.TZ = "Asia/Tokyo";

import { act } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter, Route, Routes } from "react-router";
import { resetClientForTests } from "../api/client";
import type { OrganizerEvent, OrganizerGroup, OrganizerSlot } from "../api/types";
import { resetLangForTests } from "../i18n";
import { errorText } from "../lib/form";
import { click, fill, mount, type Mounted } from "../test/dom";
import { ORGANIZER, organizerEvent, reference } from "../test/fixtures";
import { OrganizerPage } from "../pages/OrganizerPage";
import { enoughFor, groupSize, initialPicks, lockList, upcomingSlots } from "./LockSheet";

let m: Mounted;
type Call = { url: string; init?: RequestInit };

/** Horloge de la page : avant tous les créneaux de 2030, après celui de 2020. */
const NOW = Date.parse("2026-11-07T00:00:00Z");

const json = (body: unknown, status = 200) =>
  new Response(JSON.stringify(body), { status, headers: { "Content-Type": "application/json" } });
const problem = (status: number, code: string, fields?: Record<string, string>) =>
  new Response(JSON.stringify({ type: "about:blank", title: "x", status, code, ...(fields && { fields }) }), {
    status,
    headers: { "Content-Type": "application/problem+json" },
  });

function serve(view: () => Response, ...posts: (() => Response | Promise<Response>)[]) {
  const calls: Call[] = [];
  vi.stubGlobal(
    "fetch",
    vi.fn(async (url: string, init?: RequestInit) => {
      calls.push({ url, init });
      if (init?.method === "POST") {
        const next = posts.shift();
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
const sent = (c: Call) => JSON.parse(String(c.init?.body)) as Record<string, unknown>;

/** Disponibles parmi les `accepted` par créneau. */
function withAvailable(available: Record<string, number>, extra: Partial<OrganizerEvent> = {}): OrganizerEvent {
  return {
    ...organizerEvent,
    slots: organizerEvent.slots.map(
      (s): OrganizerSlot => ({ ...s, availableAccepted: available[s.slotId] ?? s.availableAccepted }),
    ),
    ...extra,
  };
}

const group = (patch: Partial<OrganizerGroup> & Pick<OrganizerGroup, "id" | "slotId">): OrganizerGroup => ({
  occurrence: 1,
  retained: true,
  parties: 2,
  partySize: 5,
  roomLabel: "I1",
  roomName: `I1-${patch.id}`,
  ...patch,
});

async function flush() {
  await act(async () => {
    for (let i = 0; i < 10; i++) await Promise.resolve();
  });
}

/** Page rendue, onglet Disponibilités ouvert. */
async function render() {
  m = mount(
    <MemoryRouter initialEntries={[`/o/${ORGANIZER}`]}>
      <Routes>
        <Route path="/o/:secret" element={<OrganizerPage reference={reference} deps={{ now: () => NOW }} />} />
      </Routes>
    </MemoryRouter>,
  );
  await flush();
  click([...m.container.querySelectorAll<HTMLButtonElement>('[role="tab"]')].find((b) => b.textContent === "Disponibilités")!);
}

const q = <T extends Element>(sel: string) => document.querySelector<T>(sel);
const openButton = () => q<HTMLButtonElement>('[data-testid="lock-open"]');
const sheet = () => q<HTMLElement>('[data-testid="lock-sheet"]');
const submit = () => q<HTMLButtonElement>('[data-testid="lock-submit"]')!;
const row = (slot: string) => q<HTMLElement>(`[data-testid="lock-row-${slot}"]`);
const box = (slot: string) => row(slot)!.querySelector<HTMLInputElement>('input[type="checkbox"]')!;
const count = (slot: string) => row(slot)!.querySelector<HTMLInputElement>('input[type="number"]')!;
const plus = (slot: string) => row(slot)!.querySelector<HTMLButtonElement>('button[aria-label^="Plus"]')!;
const enough = (slot: string) => q(`[data-testid="lock-enough-${slot}"]`)!.textContent;
const describedBy = (b: HTMLElement) => {
  const id = b.getAttribute("aria-describedby");
  return id ? document.getElementById(id)?.textContent : null;
};

async function press(b: HTMLElement | null) {
  await act(async () => b!.click());
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
  localStorage.clear();
});

describe("lock sheet", () => {
  it("locks 1 group on Thursday in two taps when 12 accepted are available there", async () => {
    // Le jeudi de la story est ici `slot_b` (ven. 05:00 à Tokyo) : 12 acceptés dispo, groupes de 10.
    const calls = serve(
      () => json(withAvailable({ slot_a: 3, slot_b: 12 })),
      () =>
        json(
          withAvailable(
            { slot_a: 3, slot_b: 12 },
            {
              event: { ...organizerEvent.event, status: "locked" },
              groups: [group({ id: "g1", slotId: "slot_b" })],
            },
          ),
        ),
    );
    await render();
    await press(openButton());
    expect(sheet()).not.toBeNull();
    expect(box("slot_b").checked).toBe(true);
    expect(box("slot_a").checked).toBe(false);
    expect(count("slot_b").max).toBe("1");
    expect(plus("slot_b").disabled).toBe(true);
    expect(enough("slot_b")).toBe("Assez pour 1 groupe");
    expect(submit().textContent).toBe("Verrouiller 1 groupe");
    await press(submit());
    expect(posts(calls).map((c) => [c.url, sent(c)])).toEqual([["/api/organizer/lock", { slotIds: ["slot_b"] }]]);
    // Vue verrouillée : feuille fermée, grille sans bouton, Composition et Salle apparues.
    expect(sheet()).toBeNull();
    expect(openButton()).toBeNull();
    expect([...document.querySelectorAll('[role="tab"]')].map((t) => t.textContent)).toEqual([
      "Candidatures",
      "Disponibilités",
      "Composition",
      "Salle",
      "Réglages",
    ]);
  });

  it("lists upcoming slots only, and bounds each Stepper by the available accepted over the group size", async () => {
    serve(() => json(withAvailable({ slot_a: 25, slot_b: 0 })));
    await render();
    await press(openButton());
    expect(row("slot_past")).toBeNull();
    expect(enough("slot_a")).toBe("Assez pour 2 groupes");
    expect(count("slot_a").max).toBe("2");
    // Aucun groupe complet possible : le Stepper reste à 1, et la ligne le dit au singulier.
    expect(enough("slot_b")).toBe("Assez pour 0 groupe");
    expect(count("slot_b").max).toBe("1");
    await press(plus("slot_a"));
    expect(count("slot_a").value).toBe("2");
    expect(plus("slot_a").disabled).toBe(true);
    expect(submit().textContent).toBe("Verrouiller 2 groupes");
  });

  it("sends one id per group, duplicates repeated, in slot order", async () => {
    const calls = serve(
      () => json(withAvailable({ slot_a: 20, slot_b: 20 })),
      () => json(withAvailable({}, { event: { ...organizerEvent.event, status: "locked" } })),
    );
    await render();
    await press(openButton());
    // Égalité : le plus tôt est pré-coché. Toucher le Stepper de l'autre le retient aussi.
    expect(box("slot_a").checked).toBe(true);
    await press(plus("slot_a"));
    await press(plus("slot_b"));
    expect(box("slot_b").checked).toBe(true);
    expect(count("slot_b").value).toBe("2");
    await press(row("slot_b")!.querySelector<HTMLButtonElement>('button[aria-label^="Moins"]'));
    expect(submit().textContent).toBe("Verrouiller 3 groupes");
    await press(submit());
    expect(sent(posts(calls)[0])).toEqual({ slotIds: ["slot_a", "slot_a", "slot_b"] });
  });

  it("disables Verrouiller with its reason when no slot is kept", async () => {
    const calls = serve(() => json(organizerEvent));
    await render();
    await press(openButton());
    // Fixture : `slot_b` a le plus de disponibles.
    expect(box("slot_b").checked).toBe(true);
    click(box("slot_b"));
    expect(submit().disabled).toBe(true);
    expect(submit().textContent).toBe("Verrouiller 0 groupe");
    expect(describedBy(submit())).toBe("Retiens au moins un créneau.");
    await press(submit());
    expect(posts(calls)).toEqual([]);
  });

  it("pre-checks the retained groups of the last lock, past the bound if need be", async () => {
    serve(() =>
      json(
        withAvailable(
          { slot_a: 0, slot_b: 30 },
          {
            groups: [
              group({ id: "gp", slotId: "slot_past" }),
              group({ id: "g1", slotId: "slot_a" }),
              group({ id: "g2", slotId: "slot_a", occurrence: 2 }),
              group({ id: "g3", slotId: "slot_b", retained: false }),
            ],
          },
        ),
      ),
    );
    await render();
    await press(openButton());
    expect(box("slot_a").checked).toBe(true);
    expect(count("slot_a").value).toBe("2");
    // Plus personne de dispo : la borne n'enlève pas un groupe déjà retenu.
    expect(count("slot_a").max).toBe("2");
    expect(box("slot_b").checked).toBe(false);
    expect(submit().textContent).toBe("Verrouiller 2 groupes");
  });

  it("keeps the sheet open with the error on a quota refusal, closes it on a state refusal", async () => {
    let current = organizerEvent;
    const calls = serve(
      () => json(current),
      () => problem(429, "limit.rate_limited"),
      () => {
        current = withAvailable({}, { event: { ...organizerEvent.event, status: "locked" } });
        return problem(409, "event.locked");
      },
    );
    await render();
    await press(openButton());
    await press(submit());
    expect(sheet()).not.toBeNull();
    expect(sheet()!.querySelector('[role="alert"]')!.textContent).toBe("Doucement : réessaie dans un moment.");
    await press(submit());
    expect(sheet()).toBeNull();
    expect(q('[data-testid="action-error"]')!.textContent).toBe(
      "Le raid est verrouillé : ce changement n'est plus possible.",
    );
    expect(calls.filter((c) => c.url === "/api/organizer")).toHaveLength(2);
    expect(openButton()).toBeNull();
  });

  it("disables Verrouiller with its reason above 14 groups", async () => {
    const calls = serve(() => json(withAvailable({ slot_a: 100, slot_b: 100 })));
    await render();
    await press(openButton());
    fill(count("slot_a"), "10");
    fill(count("slot_b"), "5");
    expect(box("slot_b").checked).toBe(true);
    expect(submit().textContent).toBe("Verrouiller 15 groupes");
    expect(submit().disabled).toBe(true);
    expect(describedBy(submit())).toBe("14 groupes au plus : retires-en.");
    fill(count("slot_b"), "4");
    expect(submit().disabled).toBe(false);
    expect(submit().getAttribute("aria-describedby")).toBeNull();
    expect(posts(calls)).toEqual([]);
  });

  it("freezes the Steppers while sending", async () => {
    let answer: (r: Response) => void = () => {};
    serve(
      () => json(withAvailable({ slot_a: 30, slot_b: 30 })),
      () => new Promise<Response>((resolve) => (answer = resolve)),
    );
    await render();
    await press(openButton());
    await press(submit());
    expect(submit().textContent).toBe("Envoi…");
    for (const slot of ["slot_a", "slot_b"]) {
      expect(count(slot).disabled, slot).toBe(true);
      expect(plus(slot).disabled, slot).toBe(true);
      expect(box(slot).disabled, slot).toBe(true);
    }
    answer(problem(429, "limit.rate_limited"));
    await flush();
    expect(count("slot_a").disabled).toBe(false);
  });

  it("drops a slot the server says has started, and shows the error", async () => {
    const calls = serve(
      () => json(withAvailable({ slot_a: 10, slot_b: 20 })),
      () => problem(400, "slot.in_past", { "slotIds.0": "slot.in_past" }),
      () => json(withAvailable({}, { event: { ...organizerEvent.event, status: "locked" } })),
    );
    await render();
    await press(openButton());
    click(box("slot_a"));
    expect(submit().textContent).toBe("Verrouiller 2 groupes");
    await press(submit());
    expect(sent(posts(calls)[0])).toEqual({ slotIds: ["slot_a", "slot_b"] });
    expect(row("slot_a")).toBeNull();
    expect(sheet()!.querySelector('[role="alert"]')!.textContent).toBe("Ce créneau est déjà passé.");
    expect(submit().textContent).toBe("Verrouiller 1 groupe");
    await press(submit());
    expect(sent(posts(calls)[1])).toEqual({ slotIds: ["slot_b"] });
    expect(sheet()).toBeNull();
  });

  it("closes on Fermer without sending", async () => {
    const calls = serve(() => json(organizerEvent));
    await render();
    await press(openButton());
    await press([...sheet()!.querySelectorAll("button")].find((b) => b.textContent === "Fermer")!);
    expect(sheet()).toBeNull();
    expect(document.activeElement).toBe(openButton());
    expect(posts(calls)).toEqual([]);
  });

  it("is disabled with its reason when no slot is upcoming", async () => {
    serve(() => json({ ...organizerEvent, event: { ...organizerEvent.event, slots: [organizerEvent.event.slots[0]] } }));
    await render();
    expect(openButton()!.disabled).toBe(true);
    expect(describedBy(openButton()!)).toBe("Plus aucun créneau à venir : rien à verrouiller.");
  });
});

describe("lock helpers", () => {
  it("compute the group size, the full groups and the request", () => {
    expect(groupSize(organizerEvent, reference)).toBe(10);
    expect(groupSize({ ...organizerEvent, event: { ...organizerEvent.event, instance: "i_off" } }, reference)).toBe(10);
    expect(groupSize({ ...organizerEvent, event: { ...organizerEvent.event, instance: "gone" } }, reference)).toBeNull();
    expect(groupSize(organizerEvent, null)).toBeNull();
    expect([enoughFor(9, 10), enoughFor(10, 10), enoughFor(29, 10), enoughFor(5, null)]).toEqual([0, 1, 2, 0]);
    const slots = upcomingSlots(organizerEvent, NOW);
    expect(slots.map((s) => s.id)).toEqual(["slot_a", "slot_b"]);
    expect(
      lockList(slots, { slot_a: { on: true, count: 2 }, slot_b: { on: false, count: 3 } }),
    ).toEqual(["slot_a", "slot_a"]);
  });

  it("name the group limit in the server refusal", () => {
    expect(errorText("event.group_count_out_of_range")).toBe("Il faut de 1 à 14 groupes.");
  });

  it("pre-check nothing without an upcoming slot", () => {
    expect(initialPicks(organizerEvent, Date.parse("2031-01-01T00:00:00Z"))).toEqual({});
  });
});
