// @vitest-environment happy-dom
// « Sur cet appareil » (story 2-6). Fuseau fixé avant tout calcul de date : UTC+9, sans heure d'été.
process.env.TZ = "Asia/Tokyo";

import { act } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter, Route, Routes } from "react-router";
import { resetClientForTests } from "../api/client";
import { LINKS_KEY } from "../api/links";
import type { PublicEvent } from "../api/types";
import { resetLangForTests } from "../i18n";
import { byText, click, mount, type Mounted, spyStorageWrites } from "../test/dom";
import { JOIN, ORGANIZER, PARTICIPANT, publicEvent, reference } from "../test/fixtures";
import { Home } from "./Home";

let m: Mounted;
type Call = { url: string; init?: RequestInit };

const NOW = Date.parse("2026-11-07T12:00:00Z");

// Quatre événements fictifs et leurs liens.
const A = "A".repeat(21) + "1";
const B = "B".repeat(21) + "2";
const C = "D".repeat(21) + "3";
const D = "F".repeat(21) + "4";
const CAND_B = "G".repeat(21) + "5";
const CAND_C = "H".repeat(21) + "6";
const CAND_A = "K".repeat(21) + "9";
const P_C = "R".repeat(42) + "7";

/** Clés des lignes de candidature. */
const PB = `participant:${B}:${CAND_B}`;
const PC = `participant:${C}:${CAND_C}`;

const event = (id: string, patch: Partial<PublicEvent> = {}): PublicEvent => ({ ...publicEvent, id, ...patch });
const slot = (startsAt: string) => [{ id: "s", startsAt, durationMin: 60 }];

/** A : organisateur, plus tard ; B : candidature, le plus proche ; C : candidature, raid annulé ;
 * D : lien direct seul, sans ligne. */
const EVENTS: Record<string, PublicEvent> = {
  [A]: event(A, { slots: slot("2030-03-01T11:00:00Z") }),
  [B]: event(B, { slots: slot("2030-01-08T11:00:00Z") }),
  [C]: event(C, { status: "cancelled", slots: slot("2030-02-01T11:00:00Z") }),
};

const STORED = {
  [A]: { organizer: ORGANIZER, join: JOIN, participants: {} },
  [B]: { participants: { [CAND_B]: PARTICIPANT } },
  [C]: { participants: { [CAND_C]: P_C } },
  [D]: { join: "Z".repeat(43), participants: {} },
};

const json = (body: unknown, status = 200) =>
  new Response(JSON.stringify(body), { status, headers: { "Content-Type": "application/json" } });
const problem = (status: number, code: string) =>
  new Response(JSON.stringify({ type: "about:blank", title: "x", status, code }), {
    status,
    headers: { "Content-Type": "application/problem+json" },
  });

/** `fetch` simulé : une réponse par événement lu, `EVENTS` par défaut. */
function serve(replies: Record<string, () => Response | Promise<Response>> = {}) {
  const calls: Call[] = [];
  vi.stubGlobal(
    "fetch",
    vi.fn(async (url: string, init?: RequestInit) => {
      calls.push({ url, init });
      const id = /^\/api\/events\/([^/]+)$/.exec(url)?.[1];
      if (id && replies[id]) return replies[id]();
      if (id && EVENTS[id]) return json(EVENTS[id]);
      throw new Error(`unexpected fetch ${url}`);
    }),
  );
  return calls;
}

function store(links: unknown) {
  localStorage.setItem(LINKS_KEY, JSON.stringify(links));
}
const stored = () => JSON.parse(localStorage.getItem(LINKS_KEY) ?? "{}") as Record<string, unknown>;

async function flush() {
  await act(async () => {
    for (let i = 0; i < 10; i++) await Promise.resolve();
  });
}

function render() {
  m = mount(
    <MemoryRouter initialEntries={["/"]}>
      <Routes>
        <Route path="/" element={<Home reference={reference} now={() => NOW} />} />
        <Route path="/o/:secret" element={<p>organizer page</p>} />
        <Route path="/p/:secret" element={<p>participant page</p>} />
      </Routes>
    </MemoryRouter>,
  );
}

const section = () => m.container.querySelector<HTMLElement>('[data-testid="on-this-device"]');
const rows = () => [...m.container.querySelectorAll<HTMLElement>('[data-testid^="device-"]')];
const rowKeys = () => rows().map((r) => r.dataset.testid!.slice("device-".length));
const row = (key: string) => m.container.querySelector<HTMLElement>(`[data-testid="device-${key}"]`)!;
const dialog = () => document.querySelector<HTMLElement>('[role="dialog"]');
const inDialog = (text: string) =>
  [...(dialog()?.querySelectorAll<HTMLButtonElement>("button") ?? [])].find((b) => b.textContent === text) ?? null;
const message = () => document.getElementById(dialog()!.getAttribute("aria-labelledby")!)!.textContent;
const forgetButton = (key: string) =>
  [...row(key).querySelectorAll<HTMLButtonElement>("button")].find((b) => b.textContent === "Oublier")!;

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

describe("on this device", () => {
  it("is hidden when nothing usable is stored, and the rest of the home stays", async () => {
    for (const links of [null, {}, { [D]: STORED[D] }, { short: STORED[A] }]) {
      if (links !== null) store(links);
      const calls = serve();
      render();
      await flush();
      expect(section()).toBeNull();
      expect(m.container.textContent).not.toContain("Sur cet appareil");
      expect(byText(m.container, "Aucun raid ouvert pour l'instant.")).toBeTruthy();
      expect(byText(m.container, "Créer un raid").getAttribute("href")).toBe("/new");
      expect(calls).toEqual([]);
      m.unmount();
      localStorage.clear();
    }
    m = mount(<div />);
  });

  it("renders the rows from storage at once, role and skeleton, before any answer", () => {
    store(STORED);
    vi.stubGlobal("fetch", vi.fn(() => new Promise(() => {})));
    render();
    // Après « Créer un raid ».
    const create = byText(m.container, "Créer un raid");
    expect(create.compareDocumentPosition(section()!) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(section()!.querySelector("h2")!.textContent).toBe("Sur cet appareil");
    expect(rowKeys()).toEqual([`organizer:${A}`, PB, PC]);
    expect(row(`organizer:${A}`).textContent).toContain("Organisateur");
    expect(row(PB).textContent).toContain("Candidat");
    expect(rows().every((r) => r.querySelector('[data-testid="skeleton"]'))).toBe(true);
    expect(row(`organizer:${A}`).querySelector("a")!.getAttribute("href")).toBe(`/o/${ORGANIZER}`);
    expect(row(PB).querySelector("a")!.getAttribute("href")).toBe(`/p/${PARTICIPANT}`);
  });

  it("reads each event once, publicly, never with a stored secret", async () => {
    store({ ...STORED, [A]: { ...STORED[A], participants: { [CAND_A]: "Y".repeat(43) } } });
    const calls = serve();
    render();
    await flush();
    expect(calls.map((c) => c.url).sort()).toEqual([A, B, C].map((id) => `/api/events/${id}`).sort());
    const secrets = [ORGANIZER, JOIN, PARTICIPANT, P_C, "Y".repeat(43), "Z".repeat(43)];
    for (const c of calls) {
      expect((c.init?.headers as Record<string, string>)["X-Link-Secret"]).toBeUndefined();
      expect(secrets.some((s) => c.url.includes(s) || JSON.stringify(c.init ?? {}).includes(s))).toBe(false);
    }
    // Pas de relecture : l'accueil ne se rafraîchit jamais.
    vi.useFakeTimers();
    await act(async () => {
      vi.advanceTimersByTime(120_000);
    });
    await flush();
    expect(calls).toHaveLength(3);
  });

  it("fills instance and date, says a raid is cancelled, then sorts once: nearest first", async () => {
    store(STORED);
    serve();
    render();
    await flush();
    expect(rowKeys()).toEqual([PB, `organizer:${A}`, PC]);
    const text = (key: string) => row(key).querySelector("a")!.textContent;
    expect(text(PB)).toBe("CandidatInstance Une · mar. 8 janv. 2030");
    expect(text(`organizer:${A}`)).toBe("OrganisateurInstance Une · ven. 1 mars 2030");
    expect(text(PC)).toBe("Candidat · Raid annuléInstance Une · ven. 1 févr. 2030");
    expect(m.container.querySelector('[data-testid="skeleton"]')).toBeNull();
    expect(forgetButton(`organizer:${A}`).getAttribute("aria-label")).toBe(
      "Oublier : Organisateur · Instance Une · ven. 1 mars 2030",
    );
  });

  it("does not move a row before every read is done, nor after", async () => {
    store(STORED);
    let release!: () => void;
    const late = new Promise<Response>((resolve) => {
      release = () => resolve(json(EVENTS[B]));
    });
    serve({ [B]: () => late });
    render();
    await flush();
    // A et C ont répondu : titres remplis, ordre du stockage gardé.
    expect(row(`organizer:${A}`).textContent).toContain("Instance Une");
    expect(row(PB).querySelector('[data-testid="skeleton"]')).not.toBeNull();
    expect(rowKeys()).toEqual([`organizer:${A}`, PB, PC]);
    await act(async () => release());
    await flush();
    expect(rowKeys()).toEqual([PB, `organizer:${A}`, PC]);
  });

  it("keeps a row whose read failed, without title and last, and forgets nothing", async () => {
    store(STORED);
    serve({
      [B]: () => problem(500, "server.internal"),
      [A]: () => Promise.reject(new TypeError("Failed to fetch")),
    });
    render();
    await flush();
    expect(rowKeys()).toEqual([PC, `organizer:${A}`, PB]);
    for (const key of [`organizer:${A}`, PB]) {
      expect(row(key).textContent).toContain("Infos du raid indisponibles pour l'instant.");
      expect(row(key).textContent).not.toContain("Instance Une");
      expect(row(key).querySelector('[data-testid="skeleton"]')).toBeNull();
    }
    expect(stored()).toEqual(STORED);
  });

  it("forgets a whole event that no longer exists, and its rows disappear", async () => {
    store({ ...STORED, [A]: { ...STORED[A], participants: { [CAND_A]: "Y".repeat(43) } } });
    serve({ [A]: () => problem(404, "event.not_found"), [C]: () => problem(429, "limit.rate_limited") });
    render();
    await flush();
    expect(rowKeys()).toEqual([PB, PC]);
    expect(stored()).toEqual({ [B]: STORED[B], [C]: STORED[C], [D]: STORED[D] });
  });

  it("hides the section when the last event is gone", async () => {
    store({ [A]: STORED[A] });
    serve({ [A]: () => problem(404, "event.not_found") });
    render();
    await flush();
    expect(section()).toBeNull();
    expect(stored()).toEqual({});
  });
});

describe("forget", () => {
  it("asks first; Garder changes nothing and gives focus back", async () => {
    store(STORED);
    serve();
    render();
    await flush();
    const button = forgetButton(`organizer:${A}`);
    click(button);
    expect(message()).toBe(
      "Ce raid disparaîtra de cet appareil. Sans ton lien d'organisateur, tu ne pourras plus le gérer.",
    );
    expect(document.activeElement).toBe(inDialog("Garder"));
    click(inDialog("Garder")!);
    expect(dialog()).toBeNull();
    expect(document.activeElement).toBe(button);
    expect(rows()).toHaveLength(3);
    expect(stored()).toEqual(STORED);
  });

  it("an organizer row forgets the organizer and direct links, never the candidatures", async () => {
    const mine = "Y".repeat(43);
    const links = { ...STORED, [A]: { ...STORED[A], participants: { [CAND_A]: mine } } };
    store(links);
    const calls = serve();
    render();
    await flush();
    const before = calls.length;
    click(forgetButton(`organizer:${A}`));
    // Une autre ligne du même raid reste : il ne disparaît pas de l'appareil.
    expect(message()).toBe(
      "Ton lien d'organisateur sera oublié sur cet appareil. Sans lui, tu ne pourras plus gérer ce raid.",
    );
    click(inDialog("Oublier")!);
    expect(dialog()).toBeNull();
    expect(rowKeys()).not.toContain(`organizer:${A}`);
    expect(stored()[A]).toEqual({ participants: { [CAND_A]: mine } });
    // Rien n'est envoyé au serveur.
    expect(calls).toHaveLength(before);
    // Le focus revient au titre de la section, le bouton étant parti avec sa ligne.
    expect(document.activeElement).toBe(section()!.querySelector("h2"));
  });

  it("a candidature row forgets only that candidature", async () => {
    store(STORED);
    serve();
    render();
    await flush();
    click(forgetButton(PB));
    expect(message()).toBe(
      "Ce raid disparaîtra de cet appareil. Sans ton lien personnel, tu ne pourras plus revoir ta candidature.",
    );
    click(inDialog("Oublier")!);
    expect(rowKeys()).toEqual([`organizer:${A}`, PC]);
    expect(stored()).toEqual({ [A]: STORED[A], [C]: STORED[C], [D]: STORED[D] });
  });

  it("the last row gone, the section disappears and focus goes to the page title", async () => {
    store({ [B]: STORED[B] });
    serve();
    render();
    await flush();
    click(forgetButton(PB));
    click(inDialog("Oublier")!);
    expect(section()).toBeNull();
    expect(stored()).toEqual({});
    expect(document.activeElement).toBe(m.container.querySelector("h1"));
  });

  it("names the candidature when another row of the same raid stays", async () => {
    store({ [A]: { ...STORED[A], participants: { [CAND_A]: "Y".repeat(43) } } });
    serve();
    render();
    await flush();
    click(forgetButton(`participant:${A}:${CAND_A}`));
    expect(message()).toBe(
      "Ton lien personnel sera oublié sur cet appareil. Sans lui, tu ne pourras plus revoir ta candidature.",
    );
  });

  it("keeps the row and says so in the sheet when the browser cannot write", async () => {
    store(STORED);
    serve();
    render();
    await flush();
    spyStorageWrites(new DOMException("blocked", "SecurityError"));
    click(forgetButton(PC));
    click(inDialog("Oublier")!);
    expect(dialog()!.querySelector('[role="alert"]')!.textContent).toBe(
      "Ce navigateur n'a rien pu effacer : la ligne reste.",
    );
    expect(rowKeys()).toEqual([PB, `organizer:${A}`, PC]);
    expect(stored()).toEqual(STORED);
    // Refermée puis rouverte, la feuille repart sans l'erreur.
    click(inDialog("Garder")!);
    click(forgetButton(PC));
    expect(dialog()!.querySelector('[role="alert"]')).toBeNull();
  });

  it("gives focus to the page title when a read removes the raid of the open sheet and no row is left", async () => {
    store({ [A]: STORED[A] });
    let answer!: () => void;
    const late = new Promise<Response>((resolve) => {
      answer = () => resolve(problem(404, "event.not_found"));
    });
    serve({ [A]: () => late });
    render();
    click(forgetButton(`organizer:${A}`));
    expect(dialog()).not.toBeNull();
    await act(async () => answer());
    await flush();
    expect(dialog()).toBeNull();
    expect(section()).toBeNull();
    expect(stored()).toEqual({});
    expect(document.activeElement).toBe(m.container.querySelector("h1"));
  });
});

describe("sort", () => {
  // Raids d'un seul organisateur chacun, tous lus avec succès sauf mention.
  const ids = ["P".repeat(21) + "1", "Q".repeat(21) + "2", "U".repeat(21) + "3", "V".repeat(21) + "4", "W".repeat(21) + "5", "Y".repeat(21) + "6"];
  const [ONGOING, SOON, PAST_THEN_LATER, PAST_RECENT, PAST_OLD, FAILED] = ids;
  const at = (iso: string, durationMin = 60) => ({ id: `s-${iso}`, startsAt: iso, durationMin });
  const minutes = (n: number) => new Date(NOW + n * 60_000).toISOString().replace(/\.\d{3}Z$/, "Z");
  const events: Record<string, PublicEvent> = {
    // En cours : commencé il y a 10 min, fini dans 50.
    [ONGOING]: event(ONGOING, { slots: [at(minutes(-10))] }),
    [SOON]: event(SOON, { slots: [at("2027-01-01T11:00:00Z")] }),
    // Un créneau passé puis un à venir : trié sur celui à venir.
    [PAST_THEN_LATER]: event(PAST_THEN_LATER, { slots: [at("2020-01-01T11:00:00Z"), at("2027-06-01T11:00:00Z")] }),
    [PAST_RECENT]: event(PAST_RECENT, { slots: [at("2025-01-01T11:00:00Z"), at("2026-10-01T11:00:00Z")] }),
    [PAST_OLD]: event(PAST_OLD, { slots: [at("2024-01-01T11:00:00Z")] }),
  };

  it("puts an ongoing or upcoming slot first, then past raids, most recent first, then failed reads", async () => {
    // Ordre du stockage volontairement inverse.
    const order = [FAILED, PAST_OLD, PAST_RECENT, PAST_THEN_LATER, SOON, ONGOING];
    store(
      Object.fromEntries(order.map((id, n) => [id, { organizer: String.fromCharCode(97 + n).repeat(43), participants: {} }])),
    );
    serve({
      ...Object.fromEntries(Object.entries(events).map(([id, e]) => [id, () => json(e)])),
      [FAILED]: () => problem(503, "server.unavailable"),
    });
    render();
    await flush();
    expect(rowKeys()).toEqual(
      [ONGOING, SOON, PAST_THEN_LATER, PAST_RECENT, PAST_OLD, FAILED].map((id) => `organizer:${id}`),
    );
  });
});
