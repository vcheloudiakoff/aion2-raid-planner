// @vitest-environment happy-dom
// Onglet Réglages (story 2-5), rendu par la page `/o/:secret` : les commandes passent par son
// séquencement et sa relecture, comme en vrai. Fuseau fixé avant tout calcul de date : UTC+9.
process.env.TZ = "Asia/Tokyo";

import { act } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter, Route, Routes, useLocation } from "react-router";
import { resetClientForTests } from "../api/client";
import type { OrganizerEvent } from "../api/types";
import { resetLangForTests } from "../i18n";
import { byText, click, fill, mount, type Mounted } from "../test/dom";
import { duplicateDraft } from "../lib/duplicate";
import { ORGANIZER, organizerEvent, reference } from "../test/fixtures";
import { REFRESH_MS } from "../pages/EventPage";
import { OrganizerPage } from "../pages/OrganizerPage";

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
function serve(view: () => Response, ...posts: (() => Response)[]) {
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

const reads = (calls: Call[]) => calls.filter((c) => c.url === "/api/organizer").length;
const posts = (calls: Call[]) => calls.filter((c) => c.init?.method === "POST");
const sent = (c: Call) => JSON.parse(String(c.init?.body)) as Record<string, unknown>;

/** Vue organisateur avec un patch sur l'événement et sur le reste. */
function viewWith(event: Partial<OrganizerEvent["event"]> = {}, extra: Partial<OrganizerEvent> = {}): OrganizerEvent {
  return { ...organizerEvent, ...extra, event: { ...organizerEvent.event, ...event } };
}

/** Sans candidature : instance, région et portée sont encore modifiables. */
const fresh = (event: Partial<OrganizerEvent["event"]> = {}) => viewWith(event, { candidatures: [] });

async function flush() {
  await act(async () => {
    for (let i = 0; i < 10; i++) await Promise.resolve();
  });
}

/** État de navigation reçu par `/new`. */
let newState: unknown;
function NewProbe() {
  newState = useLocation().state;
  return <p data-testid="new-page">new page</p>;
}

/** Page rendue, onglet Réglages ouvert. */
async function render() {
  m = mount(
    <MemoryRouter initialEntries={[`/o/${ORGANIZER}`]}>
      <Routes>
        <Route path="/o/:secret" element={<OrganizerPage reference={reference} />} />
        <Route path="/new" element={<NewProbe />} />
      </Routes>
    </MemoryRouter>,
  );
  await flush();
  click(m.container.querySelectorAll<HTMLButtonElement>('[role="tab"]')[2]);
}

const q = <T extends Element>(sel: string) => m.container.querySelector<T>(sel);
const settings = () => q<HTMLElement>('[data-testid="settings"]')!;
const save = () => q<HTMLButtonElement>('[data-testid="settings"] button[type="submit"]')!;
const note = () => q<HTMLTextAreaElement>('[data-testid="settings"] textarea')!;
const discord = () => q<HTMLInputElement>('[data-testid="settings"] input[type="url"]')!;
const listed = () => q<HTMLInputElement>('[data-testid="settings"] input[type="checkbox"]')!;
const minCp = () => q<HTMLInputElement>('[data-testid="settings"] input[max="999999"]')!;
const slotsBlock = () => q<HTMLElement>('[data-testid="settings-slots"]')!;
const addDate = () => slotsBlock().querySelector<HTMLInputElement>('input[type="datetime-local"]');
const addDuration = () => slotsBlock().querySelector<HTMLInputElement>('input[type="number"]');
const addButton = () => [...slotsBlock().querySelectorAll("button")].find((b) => b.textContent === "Ajouter") ?? null;
const removeButtons = () => [...slotsBlock().querySelectorAll<HTMLButtonElement>('button[aria-label^="Supprimer le créneau"]')];
const removeOf = (slotId: string) =>
  slotsBlock().querySelector<HTMLButtonElement>(`[data-testid="slot-${slotId}"] button`);
const dialog = () => document.querySelector<HTMLElement>('[role="dialog"]');
const inDialog = (text: string) =>
  [...(dialog()?.querySelectorAll<HTMLButtonElement>("button") ?? [])].find((b) => b.textContent === text) ?? null;
const reasonText = () => {
  const id = save().getAttribute("aria-describedby");
  return id ? document.getElementById(id)?.textContent : null;
};

/** Saisie que le navigateur ne sait pas lire (« 1e », « -- ») : valeur rapportée vide, `badInput`. */
function garble(el: HTMLInputElement) {
  Object.defineProperty(el, "validity", { configurable: true, value: { badInput: true } });
  fill(el, "");
}

/** Saisie lisible après [`garble`]. */
function retype(el: HTMLInputElement, value: string) {
  Reflect.deleteProperty(el, "validity");
  fill(el, value);
}

async function press(b: HTMLButtonElement | null) {
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

describe("settings form", () => {
  it("starts from the view, with Enregistrer disabled and its reason until something changes", async () => {
    serve(() => json(organizerEvent));
    await render();
    expect(note().value).toBe("Vocal prêt");
    expect(byText(settings(), "10 / 280")).toBeTruthy();
    expect(discord().value).toBe("https://discord.gg/abcdef");
    expect(listed().checked).toBe(true);
    expect(minCp().value).toBe("1200");
    const bound = (id: string) =>
      [...q<HTMLElement>(`[data-testid="bound-${id}"]`)!.querySelectorAll<HTMLInputElement>("input")].map((i) => i.value);
    expect(bound("c1")).toEqual(["1", "3"]);
    expect(bound("c2")).toEqual(["1", "10"]);
    expect(save().disabled).toBe(true);
    expect(reasonText()).toBe("Rien à enregistrer.");

    fill(note(), "Vocal prêt ce soir");
    expect(save().disabled).toBe(false);
    expect(reasonText()).toBeNull();
    // Revenir à la valeur de la vue : plus rien à enregistrer.
    fill(note(), "Vocal prêt");
    expect(save().disabled).toBe(true);
    click(q(`[data-testid="bound-c1"] button[aria-label="Plus : Minimum Classe Soin"]`)!);
    expect(save().disabled).toBe(false);
  });

  it("saves every value at once, clears the empty ones, then restarts from the returned view", async () => {
    const saved = viewWith({ note: "Go", visibility: "unlisted", minCp: undefined }, { discordUrl: undefined });
    const calls = serve(() => json(organizerEvent), () => json(saved));
    await render();
    fill(note(), "Go");
    fill(discord(), "  ");
    click(listed());
    fill(minCp(), "");
    click(q(`[data-testid="bound-c1"] button[aria-label="Plus : Minimum Classe Soin"]`)!);
    await press(save());
    const [post] = posts(calls);
    expect(post.url).toBe("/api/organizer/settings");
    // Champs figés renvoyés tels quels ; lien Discord et CP vides : absents, donc effacés.
    expect(sent(post)).toEqual({
      instance: "i1",
      region: "r1",
      factionScope: "all",
      note: "Go",
      visibility: "unlisted",
      classBounds: { c1: { min: 2, max: 3 }, c2: { min: 1, max: 10 } },
    });
    // Le brouillon repart de la vue rendue.
    expect(note().value).toBe("Go");
    expect(discord().value).toBe("");
    expect(listed().checked).toBe(false);
    expect(q<HTMLInputElement>('[data-testid="bound-c1"] input')!.value).toBe("1");
    expect(save().disabled).toBe(true);
    expect(reasonText()).toBe("Rien à enregistrer.");
  });

  it("shows Envoi… while saving and blocks every other command", async () => {
    let answer: (r: Response) => void = () => {};
    vi.stubGlobal(
      "fetch",
      vi.fn(async (_url: string, init?: RequestInit) =>
        init?.method === "POST" ? new Promise<Response>((r) => (answer = r)) : json(organizerEvent),
      ),
    );
    await render();
    fill(note(), "Go");
    await press(save());
    expect(save().textContent).toBe("Envoi…");
    expect(save().disabled).toBe(true);
    // La saisie est figée : la vue rendue va remplacer le brouillon.
    const fieldset = save().closest("fieldset")!;
    expect(fieldset.disabled).toBe(true);
    for (const el of [note(), discord(), listed(), minCp()]) expect(fieldset.contains(el)).toBe(true);
    expect(fieldset.contains(q('[data-testid="bound-c1"]'))).toBe(true);
    expect(removeButtons().every((b) => b.disabled)).toBe(true);
    expect(byText(settings(), "Annuler le raid").closest("button")!.disabled).toBe(true);
    // « Dupliquer » attend la vue enregistrée.
    expect(byText(settings(), "Dupliquer").closest("button")!.disabled).toBe(true);
    await act(async () => answer(json(viewWith({ note: "Go" }))));
    await flush();
    expect(save().textContent).toBe("Enregistrer");
    expect(save().closest("fieldset")!.disabled).toBe(false);
    expect(byText(settings(), "Dupliquer").closest("button")!.disabled).toBe(false);
  });

  it("never overwrites an unsaved draft on refresh, but follows the view otherwise", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval"] });
    let current = organizerEvent;
    serve(() => json(current));
    await render();
    // Sans changement : la relecture met le formulaire à jour.
    current = viewWith({ note: "Changé ailleurs", minCp: 1500 });
    await tick();
    expect(note().value).toBe("Changé ailleurs");
    expect(minCp().value).toBe("1500");
    // Avec un changement non enregistré : rien n'est écrasé.
    fill(note(), "Mon brouillon");
    current = viewWith({ note: "Encore ailleurs", minCp: 1600 });
    await tick();
    expect(note().value).toBe("Mon brouillon");
    expect(minCp().value).toBe("1500");
    expect(save().disabled).toBe(false);
  });

  it("puts the server field errors under their fields", async () => {
    serve(
      () => json(organizerEvent),
      () =>
        problem(400, "event.note_invalid", {
          note: "event.note_invalid",
          "classBounds.c2.max": "event.class_bounds_invalid",
        }),
    );
    await render();
    fill(note(), "voir www.example.org");
    await press(save());
    expect(note().getAttribute("aria-invalid")).toBe("true");
    expect(document.getElementById(note().getAttribute("aria-describedby")!)!.textContent).toBe(
      "280 caractères au plus, sans lien.",
    );
    expect(q('[data-testid="bound-c2"]')!.textContent).toContain("Minimum au-dessus du maximum");
    expect(q('[data-testid="settings"] [role="alert"]')!.textContent).toBe("280 caractères au plus, sans lien.");
    // La saisie est gardée ; l'erreur part quand le champ change.
    expect(note().value).toBe("voir www.example.org");
    fill(note(), "ok");
    expect(note().getAttribute("aria-invalid")).toBeNull();
  });

  it("refuses decimal or exponent numbers before sending", async () => {
    const calls = serve(() => json(organizerEvent));
    await render();
    for (const value of ["12.5", "1e3"]) {
      fill(minCp(), value);
      expect(save().disabled, value).toBe(true);
      expect(reasonText(), value).toBe("Le CP minimum est un nombre entier.");
    }
    fill(minCp(), "900");
    expect(save().disabled).toBe(false);
    fill(addDate()!, "2030-02-01T21:00");
    fill(addDuration()!, "12.5");
    expect(addButton()!.disabled).toBe(true);
    expect(slotsBlock().textContent).toContain("La durée est un nombre entier de minutes.");
    fill(addDuration()!, "90");
    expect(addButton()!.disabled).toBe(false);
    expect(posts(calls)).toEqual([]);
  });

  it("never reads an unreadable number as an empty one", async () => {
    const calls = serve(() => json(organizerEvent));
    await render();
    // Le CP enregistré ne s'efface pas sur une saisie illisible.
    garble(minCp());
    expect(save().disabled).toBe(true);
    expect(reasonText()).toBe("Le CP minimum est un nombre entier.");
    retype(minCp(), "900");
    expect(save().disabled).toBe(false);
    // Une durée illisible ne prend pas la durée de l'instance.
    fill(addDate()!, "2030-02-01T21:00");
    garble(addDuration()!);
    expect(addButton()!.disabled).toBe(true);
    expect(slotsBlock().textContent).toContain("La durée est un nombre entier de minutes.");
    // Effacée pour de bon : la durée de l'instance revient.
    retype(addDuration()!, "");
    expect(addButton()!.disabled).toBe(false);
    expect(posts(calls)).toEqual([]);
    m.unmount();

    // Sans CP enregistré : la valeur rapportée reste vide, la saisie illisible est vue quand même.
    serve(() => json(viewWith({ minCp: undefined })));
    await render();
    expect(minCp().value).toBe("");
    garble(minCp());
    expect(save().disabled).toBe(true);
    expect(reasonText()).toBe("Le CP minimum est un nombre entier.");
  });

  it("keeps the note under 280 characters", async () => {
    serve(() => json(organizerEvent));
    await render();
    fill(note(), "a".repeat(281));
    expect(save().disabled).toBe(true);
    expect(reasonText()).toBe("La note dépasse 280 caractères.");
    expect(byText(settings(), "281 / 280").className).toContain("text-danger");
  });
});

describe("raid fields", () => {
  it("are read-only once a candidature exists, of any status", async () => {
    const onlyWithdrawn = viewWith({}, { candidatures: [organizerEvent.candidatures[4]] });
    expect(onlyWithdrawn.candidatures[0].status).toBe("withdrawn");
    serve(() => json(onlyWithdrawn));
    await render();
    const raid = q<HTMLElement>('[data-testid="settings-raid"]')!;
    expect(raid.querySelectorAll("select, input")).toHaveLength(0);
    expect(raid.textContent).toContain("Instance Une");
    expect(raid.textContent).toContain("Région Une");
    expect(raid.textContent).toContain("Toutes");
    expect(raid.textContent).toContain("Fixés depuis la première candidature.");
  });

  it("change before the first candidature, with the scope and the bounds fitted to the instance", async () => {
    const calls = serve(
      () => json(fresh()),
      () => json(fresh({ instance: "mono", factionScope: "elyos" })),
    );
    await render();
    const raid = q<HTMLElement>('[data-testid="settings-raid"]')!;
    expect(raid.textContent).not.toContain("Fixés depuis la première candidature.");
    const [instance, region] = [...raid.querySelectorAll<HTMLSelectElement>("select")];
    // Valeurs changées : seulement les entrées actives de la référence.
    expect([...instance.options].map((o) => o.textContent)).toEqual(["Instance Une", "Mono"]);
    expect([...region.options].map((o) => o.textContent)).toEqual(["Région Une"]);
    fill(instance, "mono");
    const radio = (v: string) => [...raid.querySelectorAll<HTMLInputElement>('input[type="radio"]')].find((r) => r.value === v)!;
    expect(radio("all").disabled).toBe(true);
    expect(radio("elyos").checked).toBe(true);
    await press(save());
    expect(sent(posts(calls)[0])).toMatchObject({
      instance: "mono",
      region: "r1",
      factionScope: "elyos",
      classBounds: { c1: { min: 1, max: 3 }, c2: { min: 1, max: 4 } },
    });
  });

  it("keeps a current value retired from the reference among the choices", async () => {
    serve(() => json(fresh({ instance: "i_off", region: "r_off" })));
    await render();
    const [instance, region] = [
      ...q<HTMLElement>('[data-testid="settings-raid"]')!.querySelectorAll<HTMLSelectElement>("select"),
    ];
    expect([...instance.options].map((o) => o.textContent)).toEqual(["Instance Une", "Mono", "Instance retirée"]);
    expect(instance.value).toBe("i_off");
    expect([...region.options].map((o) => o.value)).toEqual(["r1", "r_off"]);
    expect(region.value).toBe("r_off");
  });

  it("keeps a current value missing from the reference, under its id", async () => {
    serve(() => json(fresh({ instance: "i_gone", region: "r_gone" })));
    await render();
    const [instance, region] = [
      ...q<HTMLElement>('[data-testid="settings-raid"]')!.querySelectorAll<HTMLSelectElement>("select"),
    ];
    expect([...instance.options].map((o) => [o.value, o.textContent])).toEqual([
      ["i_gone", "i_gone"],
      ["i1", "Instance Une"],
      ["mono", "Mono"],
    ]);
    expect(instance.value).toBe("i_gone");
    expect([...region.options].map((o) => [o.value, o.textContent])).toEqual([
      ["r_gone", "r_gone"],
      ["r1", "Région Une"],
    ]);
    expect(region.value).toBe("r_gone");
    expect(save().disabled).toBe(true);
    expect(reasonText()).toBe("Rien à enregistrer.");
  });

  it("caps the bounds from the stored ones when the instance is missing from the reference", async () => {
    serve(() => json(viewWith({ instance: "i_gone" })));
    await render();
    const maxOf = (id: string) => q<HTMLInputElement>(`[data-testid="bound-${id}"] input[aria-label^="Maximum"]`)!;
    expect(maxOf("c2").max).toBe("10");
    click(q(`[data-testid="bound-c2"] button[aria-label="Moins : Maximum Classe Dégâts"]`)!);
    expect(maxOf("c2").value).toBe("9");
    // Le plafond ne suit pas le brouillon qu'il borne.
    expect(maxOf("c2").max).toBe("10");
    expect(q<HTMLButtonElement>(`[data-testid="bound-c2"] button[aria-label="Plus : Maximum Classe Dégâts"]`)!.disabled).toBe(
      false,
    );
  });

  it("drop the bounds fitted to an abandoned instance once the fields freeze", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval"] });
    let current = fresh();
    const calls = serve(
      () => json(current),
      () => json(viewWith({ note: "Go" })),
    );
    await render();
    const raid = () => q<HTMLElement>('[data-testid="settings-raid"]')!;
    fill(raid().querySelector<HTMLSelectElement>("select")!, "mono");
    const maxOf = (id: string) => q<HTMLInputElement>(`[data-testid="bound-${id}"] input[aria-label^="Maximum"]`)!;
    expect(maxOf("c2").value).toBe("4");
    // Une première candidature arrive : les champs du raid sont figés sur la vue.
    current = organizerEvent;
    await tick();
    expect(raid().textContent).toContain("Fixés depuis la première candidature.");
    expect(maxOf("c2").value).toBe("10");
    expect(save().disabled).toBe(true);
    expect(reasonText()).toBe("Rien à enregistrer.");
    fill(note(), "Go");
    await press(save());
    expect(sent(posts(calls)[0])).toMatchObject({
      instance: "i1",
      factionScope: "all",
      classBounds: { c1: { min: 1, max: 3 }, c2: { min: 1, max: 10 } },
    });
  });
});

describe("slots", () => {
  it("lists the slots in local time and adds one with the instance duration by default", async () => {
    const added = viewWith({
      slots: [...organizerEvent.event.slots, { id: "slot_c", startsAt: "2030-02-01T12:00:00Z", durationMin: 60 }],
    });
    const calls = serve(() => json(organizerEvent), () => json(added), () => json(added));
    await render();
    expect(slotsBlock().textContent).toContain("Heures en Asia/Tokyo");
    expect(q('[data-testid="slot-slot_a"]')!.textContent).toContain("mer. 05:00");
    expect(q('[data-testid="slot-slot_a"]')!.textContent).toContain("mer. 9 janv. 2030");
    expect(q('[data-testid="slot-slot_past"]')!.className).toContain("text-ink-disabled");
    expect(addButton()!.disabled).toBe(true);
    expect(addDuration()!.placeholder).toBe("60");

    fill(addDate()!, "2030-02-01T21:00");
    await press(addButton());
    expect(posts(calls).map((c) => [c.url, sent(c)])).toEqual([
      ["/api/organizer/slots", { startsAt: "2030-02-01T12:00:00Z" }],
    ]);
    expect(q('[data-testid="slot-slot_c"]')).not.toBeNull();
    // La saisie repart à vide.
    expect(addDate()!.value).toBe("");

    fill(addDate()!, "2030-02-02T21:00");
    fill(addDuration()!, "90");
    await press(addButton());
    expect(sent(posts(calls)[1])).toEqual({ startsAt: "2030-02-02T12:00:00Z", durationMin: 90 });
  });

  it("takes the duration hint from the stored instance, not the unsaved one", async () => {
    serve(() => json(fresh()));
    await render();
    fill(q<HTMLElement>('[data-testid="settings-raid"]')!.querySelector<HTMLSelectElement>("select")!, "mono");
    expect(addDuration()!.placeholder).toBe("60");
  });

  it("shows a quota refusal on add in the slots block", async () => {
    serve(
      () => json(organizerEvent),
      () => problem(429, "limit.rate_limited"),
    );
    await render();
    fill(addDate()!, "2030-02-01T21:00");
    await press(addButton());
    expect(slotsBlock().querySelector('[role="alert"]')!.textContent).toBe("Doucement : réessaie dans un moment.");
    expect(addDate()!.value).toBe("2030-02-01T21:00");
  });

  it("shows a failed removal in the slots block, or in its sheet that stays open", async () => {
    serve(
      () => json(viewWith({}, { candidatures: [] })),
      () => problem(429, "limit.rate_limited"),
    );
    await render();
    await press(removeOf("slot_past"));
    expect(dialog()).toBeNull();
    expect(slotsBlock().querySelector('[role="alert"]')!.textContent).toBe("Doucement : réessaie dans un moment.");
    expect(q('[data-testid="slot-slot_past"]')).not.toBeNull();
    m.unmount();

    serve(
      () => json(organizerEvent),
      () => problem(429, "limit.rate_limited"),
    );
    await render();
    click(removeOf("slot_a")!);
    await press(inDialog("Supprimer le créneau"));
    expect(dialog()).not.toBeNull();
    expect(dialog()!.querySelector('[role="alert"]')!.textContent).toBe("Doucement : réessaie dans un moment.");
    expect(inDialog("Supprimer le créneau")!.disabled).toBe(false);
    expect(slotsBlock().querySelector('[role="alert"]')).toBeNull();
  });

  it("shows a refused slot under its inputs, and a full event before sending", async () => {
    serve(
      () => json(organizerEvent),
      () => problem(400, "slot.in_past", { startsAt: "slot.in_past" }),
    );
    await render();
    fill(addDate()!, "2020-02-01T21:00");
    await press(addButton());
    expect(addDate()!.getAttribute("aria-invalid")).toBe("true");
    expect(slotsBlock().textContent).toContain("Ce créneau est déjà passé.");
    expect(addDate()!.value).toBe("2020-02-01T21:00");
    m.unmount();

    const fourteen = Array.from({ length: 14 }, (_, i) => ({
      id: `s${i}`,
      startsAt: `2030-03-${String(i + 1).padStart(2, "0")}T12:00:00Z`,
      durationMin: 60,
    }));
    serve(() => json(viewWith({ slots: fourteen })));
    await render();
    fill(addDate()!, "2030-04-01T21:00");
    expect(addButton()!.disabled).toBe(true);
    expect(slotsBlock().textContent).toContain("14 créneaux au plus : supprimes-en un pour en ajouter un autre.");
  });

  it("removes an unticked slot right away", async () => {
    const view = viewWith({}, { candidatures: [] });
    const calls = serve(
      () => json(view),
      () => json(viewWith({ slots: organizerEvent.event.slots.slice(1) }, { candidatures: [] })),
    );
    await render();
    await press(removeOf("slot_past"));
    expect(dialog()).toBeNull();
    expect(posts(calls).map((c) => [c.url, sent(c)])).toEqual([["/api/organizer/slot-removal", { slotId: "slot_past" }]]);
    expect(q('[data-testid="slot-slot_past"]')).toBeNull();
    // Le bouton touché est parti : le focus revient au titre du bloc.
    expect(document.activeElement?.textContent).toBe("Créneaux");
  });

  it("asks first when players ticked the slot, withdrawn ones not counted", async () => {
    const calls = serve(
      () => json(organizerEvent),
      () => json(viewWith({ slots: organizerEvent.event.slots.filter((s) => s.id !== "slot_b") })),
    );
    await render();
    // slot_a : Nyra (en attente), Kael (acceptée), Toma (refusée).
    click(removeOf("slot_a")!);
    const message = () => document.getElementById(dialog()!.getAttribute("aria-labelledby")!)!.textContent;
    expect(message()).toBe("3 joueurs ont coché ce créneau : leur dispo sera effacée.");
    expect(document.activeElement).toBe(inDialog("Garder le créneau"));
    click(inDialog("Garder le créneau")!);
    expect(dialog()).toBeNull();
    // slot_past : Nyra seule.
    click(removeOf("slot_past")!);
    expect(message()).toBe("1 joueur a coché ce créneau : sa dispo sera effacée.");
    click(inDialog("Garder le créneau")!);
    expect(posts(calls)).toEqual([]);
    // slot_b : Kael et nyra ; Lio est retirée.
    click(removeOf("slot_b")!);
    expect(message()).toBe("2 joueurs ont coché ce créneau : leur dispo sera effacée.");
    const confirm = inDialog("Supprimer le créneau")!;
    expect(confirm.className).toContain("text-danger");
    await press(confirm);
    expect(sent(posts(calls)[0])).toEqual({ slotId: "slot_b" });
    expect(dialog()).toBeNull();
    expect(q('[data-testid="slot-slot_b"]')).toBeNull();
  });

  it("never offers to remove the last slot", async () => {
    serve(() => json(viewWith({ slots: [organizerEvent.event.slots[1]] })));
    await render();
    expect(removeButtons()).toHaveLength(0);
  });

  it("are hidden on a locked raid, while the settings stay editable", async () => {
    serve(() => json(viewWith({ status: "locked" })));
    await render();
    expect(slotsBlock().textContent).toBe("CréneauxLe raid est verrouillé : les créneaux ne bougent plus.");
    expect(slotsBlock().querySelectorAll("button, input")).toHaveLength(0);
    fill(note(), "Go");
    expect(save().disabled).toBe(false);
  });
});

describe("cancellation", () => {
  it("asks for a destructive confirmation; Garder le raid closes without sending", async () => {
    const calls = serve(() => json(organizerEvent), () => json(viewWith({ status: "cancelled" })));
    await render();
    const trigger = byText(settings(), "Annuler le raid") as HTMLButtonElement;
    expect(trigger.className).toContain("text-danger");
    click(trigger);
    const d = dialog()!;
    expect(document.getElementById(d.getAttribute("aria-labelledby")!)!.textContent).toBe(
      "Le raid sera annulé pour tout le monde. Ça ne se défait pas.",
    );
    expect(document.activeElement).toBe(inDialog("Garder le raid"));
    click(inDialog("Garder le raid")!);
    expect(dialog()).toBeNull();
    expect(document.activeElement).toBe(trigger);
    expect(posts(calls)).toEqual([]);

    click(trigger);
    const confirm = inDialog("Annuler le raid")!;
    expect(confirm.className).toContain("text-danger");
    await press(confirm);
    expect(posts(calls).map((c) => [c.url, sent(c)])).toEqual([["/api/organizer/cancellation", {}]]);
    expect(dialog()).toBeNull();
    // Raid annulé : bandeau, onglet en lecture seule ; seul « Dupliquer » reste.
    expect(byText(m.container, "Raid annulé")).toBeTruthy();
    expect(settings().querySelectorAll("input, select, textarea")).toHaveLength(0);
    expect([...settings().querySelectorAll("button")].map((b) => b.textContent)).toEqual(["Dupliquer"]);
    expect(settings().querySelector('[data-testid="summary"]')).not.toBeNull();
  });

  it("keeps the sheet open with the error on a network or quota failure", async () => {
    serve(
      () => json(organizerEvent),
      () => problem(429, "limit.rate_limited"),
    );
    await render();
    click(byText(settings(), "Annuler le raid"));
    await press(inDialog("Annuler le raid"));
    expect(dialog()!.querySelector('[role="alert"]')!.textContent).toBe("Doucement : réessaie dans un moment.");
    expect(inDialog("Garder le raid")!.disabled).toBe(false);
  });
});

describe("duplicate", () => {
  const duplicate = () =>
    [...q<HTMLElement>('[data-testid="settings-duplicate"]')!.querySelectorAll("button")].find(
      (b) => b.textContent === "Dupliquer",
    )!;

  it("opens /new with a draft of the saved view, never the unsaved input, and sends nothing", async () => {
    newState = undefined;
    const calls = serve(() => json(organizerEvent));
    await render();
    fill(note(), "Pas encore enregistré");
    expect(q('[data-testid="settings-duplicate"]')!.textContent).toContain(
      "Un nouveau raid avec les réglages enregistrés, sans les dates. Une saisie pas encore enregistrée n'est pas reprise.",
    );
    // Avant « Annuler le raid ».
    const cancel = byText(settings(), "Annuler le raid");
    expect(duplicate().compareDocumentPosition(cancel) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    click(duplicate());
    expect(q('[data-testid="new-page"]')).not.toBeNull();
    expect(newState).toEqual({ duplicate: duplicateDraft(organizerEvent, reference) });
    expect(JSON.stringify(newState)).not.toContain("Pas encore enregistré");
    expect(JSON.stringify(newState)).not.toContain(ORGANIZER);
    expect(posts(calls)).toEqual([]);
  });

  it("is offered on a locked raid and on a cancelled one", async () => {
    for (const status of ["locked", "cancelled"] as const) {
      newState = undefined;
      serve(() => json(viewWith({ status })));
      await render();
      click(duplicate());
      expect(newState, status).toEqual({ duplicate: duplicateDraft(viewWith({ status }), reference) });
      m.unmount();
    }
    m = mount(<div />);
  });
});

describe("state refusals", () => {
  it("show the error in the page and reload the view", async () => {
    const cases: [string, number, (calls: Call[]) => Promise<void>, Partial<OrganizerEvent["event"]>][] = [
      [
        "event.field_immutable",
        409,
        async () => {
          fill(note(), "Go");
          await press(save());
        },
        {},
      ],
      [
        "slot.not_found",
        404,
        async () => {
          await press(removeOf("slot_b"));
          await press(inDialog("Supprimer le créneau"));
        },
        { slots: organizerEvent.event.slots.filter((s) => s.id !== "slot_b") },
      ],
      [
        "event.locked",
        409,
        async () => {
          fill(addDate()!, "2030-02-01T21:00");
          await press(addButton());
        },
        { status: "locked" },
      ],
      [
        "event.closed",
        409,
        async () => {
          click(byText(settings(), "Annuler le raid"));
          await press(inDialog("Annuler le raid"));
        },
        { status: "cancelled" },
      ],
    ];
    for (const [code, status, gesture, after] of cases) {
      let current = organizerEvent;
      const calls = serve(
        () => json(current),
        () => {
          current = viewWith(after);
          return problem(status, code);
        },
      );
      await render();
      await gesture(calls);
      expect(dialog(), code).toBeNull();
      expect(q('[data-testid="action-error"]'), code).not.toBeNull();
      expect(reads(calls), code).toBe(2);
      m.unmount();
    }
    m = mount(<div />);
  });

  it("name the frozen fields in the page, then show them read-only after the reload", async () => {
    let current = fresh();
    serve(
      () => json(current),
      () => {
        current = organizerEvent;
        return problem(409, "event.field_immutable", { region: "event.field_immutable" });
      },
    );
    await render();
    fill(note(), "Go");
    await press(save());
    expect(q('[data-testid="action-error"]')!.textContent).toBe(
      "Instance, région et factions sont fixées depuis la première candidature.",
    );
    expect(q('[data-testid="settings-raid"]')!.textContent).toContain("Fixés depuis la première candidature.");
    // Le reste du brouillon survit à la relecture.
    expect(note().value).toBe("Go");
  });

  it("show the dead link when the link dies under a command", async () => {
    serve(
      () => json(organizerEvent),
      () => problem(404, "auth.not_found"),
    );
    await render();
    fill(note(), "Go");
    await press(save());
    expect(byText(m.container, "Retour à l'accueil")).toBeTruthy();
  });
});
