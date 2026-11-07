// @vitest-environment happy-dom
// Fuseau fixé avant tout calcul de date : UTC+9, sans heure d'été.
process.env.TZ = "Asia/Tokyo";

import { act } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter, Route, Routes, useLocation, type Location } from "react-router";
import { resetClientForTests } from "../api/client";
import { LINKS_KEY } from "../api/links";
import { resetLangForTests } from "../i18n";
import { byText, click, fill, mount, type Mounted, spyStorageWrites } from "../test/dom";
import { EVENT_ID, JOIN, ORGANIZER, created, reference } from "../test/fixtures";
import type { ReferenceSet } from "../api/types";
import type { DuplicateDraft } from "../lib/duplicate";
import { NewEvent } from "./NewEvent";

let m: Mounted;
type Call = { url: string; init?: RequestInit };

/** `fetch` simulé : jeton puis réponses de `POST /api/events` dans l'ordre. */
function serve(...posts: (() => Response)[]) {
  const calls: Call[] = [];
  vi.stubGlobal(
    "fetch",
    vi.fn(async (url: string, init?: RequestInit) => {
      calls.push({ url, init });
      if (url.startsWith("/api/form-token")) {
        return new Response(JSON.stringify({ token: "tok" }), { status: 200, headers: { "Content-Type": "application/json" } });
      }
      const next = posts.shift();
      if (url === "/api/events" && next) return next();
      throw new Error(`unexpected fetch ${url}`);
    }),
  );
  return calls;
}

const createdReply = () =>
  new Response(JSON.stringify(created), { status: 201, headers: { "Content-Type": "application/json" } });
const problem = (code: string, fields: Record<string, string>) => () =>
  new Response(JSON.stringify({ type: "about:blank", title: "Bad Request", status: 400, code, fields }), {
    status: 400,
    headers: { "Content-Type": "application/problem+json" },
  });

const posts = (calls: Call[]) =>
  calls.filter((c) => c.url === "/api/events").map((c) => JSON.parse(String(c.init?.body)) as Record<string, unknown>);

async function flush() {
  await act(async () => {
    for (let i = 0; i < 10; i++) await Promise.resolve();
  });
}

/** Accès de test au routeur : emplacement courant et son état d'historique. */
let here: Location;
function Probe() {
  here = useLocation();
  return null;
}

function render(ref: ReferenceSet = reference, entry: string | Partial<Location> = "/new") {
  m = mount(
    <MemoryRouter initialEntries={[entry]}>
      <Routes>
        <Route
          path="/new"
          element={
            <>
              <NewEvent reference={ref} />
              <Probe />
            </>
          }
        />
        <Route path="/o/:secret" element={<p>organizer page</p>} />
      </Routes>
    </MemoryRouter>,
  );
}

const q = <T extends Element>(sel: string) => m.container.querySelector<T>(sel)!;
const slotDate = (n: number) => q<HTMLInputElement>(`[data-testid="slot-${n}"] input[type="datetime-local"]`);
const slotDuration = (n: number) => q<HTMLInputElement>(`[data-testid="slot-${n}"] input[type="number"]`);
const publish = () => q<HTMLButtonElement>('button[type="submit"]');

async function submit() {
  await act(async () => {
    publish().click();
  });
  await flush();
}

beforeEach(() => {
  localStorage.clear();
  resetClientForTests();
  vi.spyOn(navigator, "language", "get").mockReturnValue("fr-FR");
  resetLangForTests();
  document.title = "aion2-raid-planner";
});

afterEach(() => {
  m.unmount();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  localStorage.clear();
});

describe("new event form", () => {
  it("asks for a form token on first render and shows game labels from the reference", async () => {
    const calls = serve();
    render();
    await flush();
    expect(calls.map((c) => c.url)).toEqual(["/api/form-token?class=create"]);
    const options = [...m.container.querySelectorAll("option")].map((o) => o.textContent);
    expect(options).toEqual(["Instance Une", "Mono", "Région Une"]);
    expect(m.container.textContent).toContain("Élyséens");
    expect(m.container.textContent).toContain("Classe Soin");
    expect(m.container.textContent).not.toContain("Old");
    expect(m.container.textContent).toMatch(/Heures en Asia\/Tokyo/);
  });

  it("keeps Publish disabled with its reason until every slot has a date", () => {
    serve();
    render();
    expect(publish().disabled).toBe(true);
    expect(byText(m.container, "Choisis la date et l'heure de chaque créneau.")).toBeTruthy();
    fill(slotDate(0), "2026-11-11T21:00");
    expect(publish().disabled).toBe(false);
  });

  it("refuses All on a single-faction instance", () => {
    serve();
    render();
    fill(q<HTMLSelectElement>("select"), "mono");
    const all = [...m.container.querySelectorAll<HTMLInputElement>('input[type="radio"]')].find((r) => r.value === "all")!;
    expect(all.disabled).toBe(true);
    const elyos = [...m.container.querySelectorAll<HTMLInputElement>('input[type="radio"]')].find((r) => r.value === "elyos")!;
    expect(elyos.checked).toBe(true);
  });

  it("sends local times as UTC, then saves the links and shows them", async () => {
    const calls = serve(createdReply);
    render();
    await flush();
    fill(slotDate(0), "2026-11-11T21:00");
    click(byText(m.container, "+ Ajouter un créneau"));
    fill(slotDate(1), "2026-11-12T05:30");
    fill(slotDuration(1), "90");
    fill(q<HTMLTextAreaElement>("textarea"), "Vocal prêt");
    expect(byText(m.container, "10 / 280")).toBeTruthy();
    click(q<HTMLInputElement>('label input[type="checkbox"]'));
    await submit();

    expect(posts(calls)).toEqual([
      {
        instance: "i1",
        region: "r1",
        factionScope: "all",
        slots: [{ startsAt: "2026-11-11T12:00:00Z" }, { startsAt: "2026-11-11T20:30:00Z", durationMin: 90 }],
        visibility: "unlisted",
        note: "Vocal prêt",
        formToken: "tok",
        website: "",
      },
    ]);
    const stored = JSON.parse(localStorage.getItem(LINKS_KEY)!);
    expect(stored[EVENT_ID]).toEqual({ organizer: ORGANIZER, join: JOIN, participants: {} });
    expect(byText(m.container, "Liens créés")).toBeTruthy();
    expect(m.container.textContent).not.toMatch(/Ce navigateur ne gardera pas/);
    for (const secret of [ORGANIZER, JOIN]) expect(document.title).not.toContain(secret);
  });

  it("shows the Envoi… label while sending", async () => {
    let answer: (r: Response) => void = () => {};
    vi.stubGlobal(
      "fetch",
      vi.fn((url: string) =>
        url === "/api/events"
          ? new Promise<Response>((r) => (answer = r))
          : Promise.resolve(new Response(JSON.stringify({ token: "tok" }), { status: 200 })),
      ),
    );
    render();
    fill(slotDate(0), "2026-11-11T21:00");
    await submit();
    expect(publish().textContent).toBe("Envoi…");
    expect(publish().disabled).toBe(true);
    await act(async () => answer(createdReply()));
    await flush();
    expect(byText(m.container, "Liens créés")).toBeTruthy();
  });

  it("puts field errors under their fields and keeps the input", async () => {
    serve(problem("slot.in_past", { "slots.0.startsAt": "slot.in_past", note: "event.note_invalid" }));
    render();
    fill(slotDate(0), "2026-11-11T21:00");
    fill(q<HTMLTextAreaElement>("textarea"), "www.example.org");
    await submit();
    const slot0 = q<HTMLElement>('[data-testid="slot-0"]');
    expect(slot0.textContent).toContain("Ce créneau est déjà passé.");
    expect(slotDate(0).value).toBe("2026-11-11T21:00");
    expect(slotDate(0).getAttribute("aria-invalid")).toBe("true");
    const note = q<HTMLTextAreaElement>("textarea");
    expect(note.value).toBe("www.example.org");
    expect(document.getElementById(note.getAttribute("aria-describedby")!)?.textContent).toBe(
      "280 caractères au plus, sans lien.",
    );
    expect(q('[role="alert"]').textContent).toBe("Ce créneau est déjà passé.");
    expect(publish().textContent).toBe("Publier");
  });

  it("opens Options to show a class bound error under its class", async () => {
    serve(problem("event.class_bounds_invalid", { "classBounds.c1.max": "event.class_bounds_invalid" }));
    render();
    fill(slotDate(0), "2026-11-11T21:00");
    await submit();
    expect(q<HTMLDetailsElement>("details").open).toBe(true);
    expect(q('[data-testid="bound-c1"]').textContent).toContain("Minimum au-dessus du maximum");
    expect(q('[data-testid="bound-c2"]').textContent).not.toContain("Minimum au-dessus du maximum");
  });

  it("sends only the class bounds changed with the steppers, prefilled from the reference", async () => {
    const calls = serve(createdReply);
    render();
    const minOf = (id: string) => q<HTMLInputElement>(`[data-testid="bound-${id}"] input`);
    expect(minOf("c2").value).toBe("1");
    click(q(`[data-testid="bound-c1"] button[aria-label="Plus : Minimum Classe Soin"]`));
    fill(slotDate(0), "2026-11-11T21:00");
    await submit();
    expect(posts(calls)[0].classBounds).toEqual({ c1: { min: 1, max: 10 } });
  });

  it("still shows the links, with a warning, when storage is unavailable", async () => {
    serve(createdReply);
    spyStorageWrites(new DOMException("blocked", "SecurityError"));
    render();
    fill(slotDate(0), "2026-11-11T21:00");
    await submit();
    expect(byText(m.container, "Liens créés")).toBeTruthy();
    expect(byText(m.container, "Ce navigateur ne gardera pas le lien : copie-le maintenant.")).toBeTruthy();
    expect(document.title).not.toContain(ORGANIZER);
  });

  it("caps default bounds on a small instance and sends the edited one inside the cap", async () => {
    const calls = serve(createdReply);
    render();
    fill(q<HTMLSelectElement>("select"), "mono");
    click(q(`[data-testid="bound-c1"] button[aria-label="Plus : Minimum Classe Soin"]`));
    fill(slotDate(0), "2026-11-11T21:00");
    await submit();
    expect(posts(calls)[0]).toMatchObject({ instance: "mono", factionScope: "elyos", classBounds: { c1: { min: 1, max: 4 } } });
  });

  it("clamps edited bounds to the new instance size on instance change", async () => {
    const calls = serve(createdReply);
    render();
    click(q(`[data-testid="bound-c1"] button[aria-label="Plus : Minimum Classe Soin"]`));
    fill(q<HTMLSelectElement>("select"), "mono");
    const inputs = m.container.querySelectorAll<HTMLInputElement>('[data-testid="bound-c1"] input');
    expect([...inputs].map((i) => i.value)).toEqual(["1", "4"]);
    fill(slotDate(0), "2026-11-11T21:00");
    await submit();
    expect(posts(calls)[0].classBounds).toEqual({ c1: { min: 1, max: 4 } });
  });

  it("starts on a valid scope when the default instance is single-faction", () => {
    serve();
    render({ ...reference, instances: [reference.instances[1], reference.instances[0]] });
    const radio = (v: string) =>
      [...m.container.querySelectorAll<HTMLInputElement>('input[type="radio"]')].find((r) => r.value === v)!;
    expect(radio("all").disabled).toBe(true);
    expect(radio("all").checked).toBe(false);
    expect(radio("elyos").checked).toBe(true);
    fill(slotDate(0), "2026-11-11T21:00");
    expect(publish().disabled).toBe(false);
  });

  it("refuses decimal or exponent numbers before sending", () => {
    const calls = serve();
    render();
    fill(slotDate(0), "2026-11-11T21:00");
    const reason = "Les nombres doivent être entiers : CP et durées.";
    fill(slotDuration(0), "12.5");
    expect(publish().disabled).toBe(true);
    expect(byText(m.container, reason)).toBeTruthy();
    fill(slotDuration(0), "90");
    expect(publish().disabled).toBe(false);
    fill(q<HTMLInputElement>('input[max="999999"]'), "1e3");
    expect(publish().disabled).toBe(true);
    expect(byText(m.container, reason)).toBeTruthy();
    expect(posts(calls)).toEqual([]);
  });

  it("never reads an unreadable number as an empty one", () => {
    const calls = serve();
    render();
    fill(slotDate(0), "2026-11-11T21:00");
    const reason = "Les nombres doivent être entiers : CP et durées.";
    // Saisie que le navigateur ne sait pas lire : valeur rapportée vide, `validity.badInput`.
    const garble = (el: HTMLInputElement) => {
      Object.defineProperty(el, "validity", { configurable: true, value: { badInput: true } });
      fill(el, "");
    };
    const retype = (el: HTMLInputElement, value: string) => {
      Reflect.deleteProperty(el, "validity");
      fill(el, value);
    };
    for (const input of [q<HTMLInputElement>('input[max="999999"]'), slotDuration(0)]) {
      garble(input);
      expect(publish().disabled).toBe(true);
      expect(byText(m.container, reason)).toBeTruthy();
      retype(input, "");
      expect(publish().disabled).toBe(false);
    }
    expect(posts(calls)).toEqual([]);
  });

  it("clears slot errors when a slot is removed, and a field error when it changes", async () => {
    serve(problem("slot.in_past", { "slots.1.startsAt": "slot.in_past", note: "event.note_invalid" }));
    render();
    fill(slotDate(0), "2026-11-11T21:00");
    click(byText(m.container, "+ Ajouter un créneau"));
    fill(slotDate(1), "2026-11-12T21:00");
    fill(q<HTMLTextAreaElement>("textarea"), "www.x.org");
    await submit();
    expect(q('[data-testid="slot-1"]').textContent).toContain("Ce créneau est déjà passé.");
    fill(q<HTMLTextAreaElement>("textarea"), "ok");
    expect(m.container.textContent).not.toContain("280 caractères au plus, sans lien.");
    click(q('[data-testid="slot-0"] button[aria-label="Retirer le créneau 1"]'));
    expect(q('[data-testid="slot-0"]').textContent).not.toContain("Ce créneau est déjà passé.");
    expect(slotDate(0).getAttribute("aria-invalid")).toBeNull();
  });

  it("shows the dedicated pause message on limit.paused and keeps the form", async () => {
    serve(() =>
      new Response(JSON.stringify({ type: "about:blank", title: "x", status: 503, code: "limit.paused" }), {
        status: 503,
        headers: { "Content-Type": "application/problem+json", "Retry-After": "60" },
      }),
    );
    render();
    fill(slotDate(0), "2026-11-11T21:00");
    await submit();
    expect(q('[role="alert"]').textContent).toBe(
      "La création de raids est en pause. Les raids existants marchent toujours.",
    );
    expect(slotDate(0).value).toBe("2026-11-11T21:00");
  });
});

describe("duplicate", () => {
  const draft: DuplicateDraft = {
    instance: "i1",
    region: "r1",
    factionScope: "asmodian",
    note: "Vocal prêt",
    discordUrl: "https://discord.gg/abcdef",
    visibility: "unlisted",
    minCp: 1200,
    classBounds: { c1: { min: 1, max: 3 }, c2: { min: 2, max: 10 } },
    slots: [{}, {}, { durationMin: 90 }],
  };
  const radio = (v: string) =>
    [...m.container.querySelectorAll<HTMLInputElement>('input[type="radio"]')].find((r) => r.value === v)!;
  const selects = () => [...m.container.querySelectorAll<HTMLSelectElement>("select")].map((s) => s.value);
  const bound = (id: string) =>
    [...m.container.querySelectorAll<HTMLInputElement>(`[data-testid="bound-${id}"] input`)].map((i) => i.value);
  const slotCount = () => m.container.querySelectorAll('[data-testid^="slot-"]').length;

  it("prefills everything but the dates, one line per slot, then clears the navigation state", async () => {
    const calls = serve(createdReply);
    render(reference, { pathname: "/new", state: { duplicate: draft } });
    await flush();
    expect(selects()).toEqual(["i1", "r1"]);
    expect(radio("asmodian").checked).toBe(true);
    expect(slotCount()).toBe(3);
    expect([0, 1, 2].map((n) => slotDate(n).value)).toEqual(["", "", ""]);
    expect([0, 1, 2].map((n) => slotDuration(n).value)).toEqual(["", "", "90"]);
    expect(q<HTMLTextAreaElement>("textarea").value).toBe("Vocal prêt");
    expect(q<HTMLInputElement>('input[type="url"]').value).toBe("https://discord.gg/abcdef");
    expect(q<HTMLInputElement>('label input[type="checkbox"]').checked).toBe(false);
    expect(q<HTMLInputElement>('details input[type="number"]').value).toBe("1200");
    expect(bound("c1")).toEqual(["1", "3"]);
    expect(bound("c2")).toEqual(["2", "10"]);
    expect(publish().disabled).toBe(true);
    expect(byText(m.container, "Choisis la date et l'heure de chaque créneau.")).toBeTruthy();
    // Consommé : l'entrée d'historique n'a plus d'état.
    expect(here.pathname).toBe("/new");
    expect(here.state).toBeNull();

    fill(slotDate(0), "2026-11-11T21:00");
    fill(slotDate(1), "2026-11-12T21:00");
    fill(slotDate(2), "2026-11-13T21:00");
    await submit();
    expect(posts(calls)).toEqual([
      {
        instance: "i1",
        region: "r1",
        factionScope: "asmodian",
        slots: [
          { startsAt: "2026-11-11T12:00:00Z" },
          { startsAt: "2026-11-12T12:00:00Z" },
          { startsAt: "2026-11-13T12:00:00Z", durationMin: 90 },
        ],
        visibility: "unlisted",
        note: "Vocal prêt",
        discordUrl: "https://discord.gg/abcdef",
        minCp: 1200,
        classBounds: { c1: { min: 1, max: 3 }, c2: { min: 2, max: 10 } },
        formToken: "tok",
        website: "",
      },
    ]);
  });

  it("gives a blank form when the same history entry is opened again", async () => {
    serve();
    render(reference, { pathname: "/new", state: { duplicate: draft } });
    await flush();
    const reloaded = here;
    m.unmount();
    render(reference, reloaded);
    await flush();
    expect(slotCount()).toBe(1);
    expect(q<HTMLTextAreaElement>("textarea").value).toBe("");
    expect(radio("all").checked).toBe(true);
    expect(q<HTMLInputElement>('label input[type="checkbox"]').checked).toBe(true);
  });

  it("falls back on a disabled instance or region, as for any input", async () => {
    serve();
    render(reference, { pathname: "/new", state: { duplicate: { ...draft, instance: "i_off", region: "r_off" } } });
    await flush();
    expect(selects()).toEqual(["i1", "r1"]);
    expect(radio("asmodian").checked).toBe(true);
    expect(bound("c1")).toEqual(["1", "3"]);
  });

  it("fits the bounds to the instance and adjusts the scope", async () => {
    const calls = serve(createdReply);
    render(reference, {
      pathname: "/new",
      state: {
        duplicate: { ...draft, instance: "mono", factionScope: "all", classBounds: { c1: { min: 6, max: 8 }, c2: { min: 1, max: 10 } } },
      },
    });
    await flush();
    expect(selects()[0]).toBe("mono");
    expect(radio("all").disabled).toBe(true);
    expect(radio("elyos").checked).toBe(true);
    expect(bound("c1")).toEqual(["4", "4"]);
    expect(bound("c2")).toEqual(["1", "4"]);
    for (const n of [0, 1, 2]) fill(slotDate(n), `2026-11-1${n + 1}T21:00`);
    await submit();
    expect(posts(calls)[0]).toMatchObject({
      instance: "mono",
      factionScope: "elyos",
      classBounds: { c1: { min: 4, max: 4 }, c2: { min: 1, max: 4 } },
    });
  });

  it("opens Options when the draft carries a minimum CP or bounds of its own, never otherwise", async () => {
    const { minCp: _, ...noCp } = draft;
    const usual = { c1: { min: 0, max: 10 }, c2: { min: 1, max: 10 } };
    for (const [name, duplicate, open] of [
      ["minimum CP", draft, true],
      ["own bounds", { ...noCp, classBounds: { ...usual, c1: { min: 1, max: 3 } } }, true],
      ["usual bounds", { ...noCp, classBounds: usual }, false],
    ] as const) {
      serve();
      render(reference, { pathname: "/new", state: { duplicate } });
      await flush();
      expect(q<HTMLDetailsElement>("details").open, name).toBe(open);
      m.unmount();
    }
    // Sans brouillon, Options reste replié.
    serve();
    render();
    expect(q<HTMLDetailsElement>("details").open).toBe(false);
  });

  it("gives an added slot its own line, after the duplicated ones", async () => {
    serve();
    render(reference, { pathname: "/new", state: { duplicate: draft } });
    await flush();
    click(byText(m.container, "+ Ajouter un créneau"));
    expect(slotCount()).toBe(4);
    fill(slotDate(1), "2026-11-12T21:00");
    expect(slotDate(1).value).toBe("2026-11-12T21:00");
    expect(slotDate(3).value).toBe("");
    expect(slotDuration(3).value).toBe("");
    fill(slotDate(3), "2026-11-14T21:00");
    expect([0, 1, 2, 3].map((n) => slotDate(n).value)).toEqual(["", "2026-11-12T21:00", "", "2026-11-14T21:00"]);
    expect(slotDuration(2).value).toBe("90");
  });

  it("ignores a malformed state and starts blank", async () => {
    serve();
    render(reference, { pathname: "/new", state: { duplicate: { ...draft, slots: "x" } } });
    await flush();
    expect(slotCount()).toBe(1);
    expect(q<HTMLTextAreaElement>("textarea").value).toBe("");
    expect(here.state).toBeNull();
  });
});
