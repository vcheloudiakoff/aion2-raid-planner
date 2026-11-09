// @vitest-environment happy-dom
// Fuseau fixé avant tout calcul de date : UTC+9, sans heure d'été.
process.env.TZ = "Asia/Tokyo";

import { act } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter, Route, Routes } from "react-router";
import { resetClientForTests } from "../api/client";
import { LINKS_KEY } from "../api/links";
import type { PublicEvent } from "../api/types";
import { resetLangForTests } from "../i18n";
import { CANDIDATE_PREFS_KEY } from "../lib/storage";
import { byText, click, fill, mount, type Mounted, spyStorageWrites } from "../test/dom";
import {
  CANDIDATURE_ID,
  EVENT_ID,
  JOIN,
  PARTICIPANT,
  createdCandidature,
  publicEvent,
  reference,
} from "../test/fixtures";
import { EventPage, REFRESH_MS } from "./EventPage";
import { ParticipantPage } from "./ParticipantPage";

let m: Mounted;
type Call = { url: string; init?: RequestInit };

const json = (body: unknown, status = 200) =>
  new Response(JSON.stringify(body), { status, headers: { "Content-Type": "application/json" } });
const problem = (status: number, code: string, fields?: Record<string, string>) =>
  new Response(JSON.stringify({ type: "about:blank", title: "x", status, code, fields }), {
    status,
    headers: { "Content-Type": "application/problem+json" },
  });

/** `fetch` simulé : jeton, lecture de l'événement (toujours `event()`), puis les POST dans l'ordre. */
function serve(event: () => Response, ...posts: (() => Response)[]) {
  const calls: Call[] = [];
  vi.stubGlobal(
    "fetch",
    vi.fn(async (url: string, init?: RequestInit) => {
      calls.push({ url, init });
      if (url.startsWith("/api/form-token")) return json({ token: "tok" });
      if (init?.method === "POST") {
        const next = posts.shift();
        if (next) return next();
        throw new Error(`unexpected POST ${url}`);
      }
      if (url === `/api/events/${EVENT_ID}` || url === "/api/join") return event();
      throw new Error(`unexpected fetch ${url}`);
    }),
  );
  return calls;
}

const posts = (calls: Call[]) => calls.filter((c) => c.init?.method === "POST");
const sent = (c: Call) => JSON.parse(String(c.init?.body)) as Record<string, unknown>;
const headerOf = (c: Call, name: string) => (c.init?.headers as Record<string, string> | undefined)?.[name];

async function flush() {
  await act(async () => {
    for (let i = 0; i < 10; i++) await Promise.resolve();
  });
}

async function render(path = `/e/${EVENT_ID}`) {
  m = mount(
    <MemoryRouter initialEntries={[path]}>
      <Routes>
        <Route path="/e/:id" element={<EventPage reference={reference} mode="public" />} />
        <Route path="/j/:secret" element={<EventPage reference={reference} mode="join" />} />
        <Route path="/p/:secret" element={<ParticipantPage reference={reference} origin="https://raid.example.org" />} />
      </Routes>
    </MemoryRouter>,
  );
  await flush();
}

const q = <T extends Element>(sel: string) => m.container.querySelector<T>(sel)!;
const pseudo = () => q<HTMLInputElement>('input[type="text"]:not([name="website"])');
const serverSelect = () => m.container.querySelectorAll<HTMLSelectElement>("select")[0];
const classSelect = () => m.container.querySelectorAll<HTMLSelectElement>("select")[1];
const cp = () => q<HTMLInputElement>('input[type="number"]');
const slotButton = (id: string) => q<HTMLButtonElement>(`[data-testid="slot-${id}"] button[aria-label^="Disponible"]`);
const star = (id: string) =>
  m.container.querySelector<HTMLButtonElement>(`[data-testid="slot-${id}"] button[aria-label^="Préféré"]`);
const submitButton = () => q<HTMLButtonElement>('button[type="submit"]');

function fillValid() {
  fill(pseudo(), "Nyra");
  fill(serverSelect(), "s_a");
  fill(classSelect(), "c1");
  fill(cp(), "1500");
  click(slotButton("slot_a"));
}

async function submit() {
  await act(async () => {
    submitButton().click();
  });
  await flush();
}

beforeEach(() => {
  localStorage.clear();
  resetClientForTests();
  vi.spyOn(navigator, "language", "get").mockReturnValue("fr-FR");
  resetLangForTests();
  Object.defineProperty(navigator, "share", { value: undefined, configurable: true, writable: true });
});

afterEach(() => {
  m.unmount();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  vi.useRealTimers();
  localStorage.clear();
});

describe("summary", () => {
  it("shows instance and first date, region, scope, note, local slots and the roles line", async () => {
    serve(() => json(publicEvent));
    await render();
    const summary = q('[data-testid="summary"]');
    // Premier créneau à venir : 2030-01-08T20:00Z = mer. 9 janv. 2030, 05:00 à Tokyo.
    expect(q("h1").textContent).toBe("Instance Une · mer. 9 janv. 2030");
    expect(summary.textContent).toContain("Région Une");
    expect(summary.textContent).toContain("Toutes factions");
    expect(summary.textContent).toContain("Vocal prêt");
    expect(summary.textContent).toContain("Heures en Asia/Tokyo");
    const slots = [...m.container.querySelectorAll('[data-testid="summary-slots"] li')].map((l) => l.textContent);
    expect(slots).toEqual(["mer. 05:00 mer. 8 janv. 2020", "mer. 05:00 mer. 9 janv. 2030", "ven. 05:00 ven. 11 janv. 2030"]);
    expect(q('[data-testid="roles"]').textContent).toBe("3 inscrits · manque 1 Soin");
    expect(summary.textContent).toMatch(/CP minimum : 1\s200/);
    expect(summary.textContent).not.toContain("Lien direct");
  });

  it("shows a faction dot and name for a single-faction raid", async () => {
    serve(() => json({ ...publicEvent, factionScope: "asmodian", missingRoles: [], countsByClass: { c1: 1 } }));
    await render();
    const summary = q('[data-testid="summary"]');
    expect(summary.textContent).toContain("Asmodiens");
    expect(summary.querySelector(".bg-faction-asmodian")).not.toBeNull();
    expect(q('[data-testid="roles"]').textContent).toBe("1 inscrit");
  });

  it("refreshes every 10 s without touching the form", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval"] });
    let current: PublicEvent = publicEvent;
    const calls = serve(() => json(current));
    await render();
    fill(pseudo(), "Nyra");
    current = { ...publicEvent, countsByClass: { c1: 2, c2: 2 }, missingRoles: [] };
    await act(async () => {
      vi.advanceTimersByTime(REFRESH_MS);
    });
    await flush();
    expect(calls.filter((c) => c.url === `/api/events/${EVENT_ID}`).length).toBe(2);
    expect(q('[data-testid="roles"]').textContent).toBe("4 inscrits");
    expect(pseudo().value).toBe("Nyra");
  });
});

describe("refresh", () => {
  it("titles with the last slot when every slot is past", async () => {
    serve(() => json({ ...publicEvent, slots: [publicEvent.slots[0]] }));
    await render();
    expect(q("h1").textContent).toBe("Instance Une · mer. 8 janv. 2020");
  });

  it("stops polling on a terminal code and shows the dead-link message, without skeleton", async () => {
    for (const code of ["event.not_found", "auth.not_found", "auth.wrong_right", "auth.missing"]) {
      vi.useFakeTimers({ toFake: ["setInterval", "clearInterval"] });
      const calls = serve(() => problem(code === "auth.missing" ? 401 : code === "auth.wrong_right" ? 403 : 404, code));
      await render(code.startsWith("auth") ? `/j/${JOIN}` : `/e/${EVENT_ID}`);
      expect(q('[role="alert"]').textContent).toBe("Ce lien ne mène à rien. Il a peut-être expiré.");
      expect(m.container.querySelector('[aria-busy="true"]')).toBeNull();
      const reads = () => calls.filter((c) => c.init?.method !== "POST" && !c.url.startsWith("/api/form-token")).length;
      await act(async () => {
        vi.advanceTimersByTime(REFRESH_MS * 3);
      });
      await flush();
      expect(reads(), code).toBe(1);
      m.unmount();
      vi.useRealTimers();
    }
    m = mount(<div />);
  });

  it("stops polling once the raid is cancelled", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval"] });
    const calls = serve(() => json({ ...publicEvent, status: "cancelled" }));
    await render();
    await act(async () => {
      vi.advanceTimersByTime(REFRESH_MS * 3);
    });
    await flush();
    expect(calls.filter((c) => c.url === `/api/events/${EVENT_ID}`).length).toBe(1);
  });
});

describe("form", () => {
  it("renews a candidature-class token on form.token_invalid before the second POST", async () => {
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
    const calls = serve(
      () => json(publicEvent),
      () => problem(400, "form.token_invalid"),
      () => json(createdCandidature, 201),
    );
    await render();
    fillValid();
    await act(async () => {
      submitButton().click();
    });
    await flush();
    await act(async () => {
      vi.advanceTimersByTime(5000);
    });
    await flush();
    const order = calls.map((c) => (c.init?.method === "POST" ? "POST" : c.url));
    const firstPost = order.indexOf("POST");
    const secondPost = order.indexOf("POST", firstPost + 1);
    expect(secondPost).toBeGreaterThan(firstPost);
    const renewal = order.slice(firstPost + 1, secondPost);
    expect(renewal).toContain("/api/form-token?class=candidature");
    expect(q("h1").textContent).toBe("Ton lien personnel");
  });

  it("marks the slot named by an availabilities.N.slotId error", async () => {
    serve(
      () => json(publicEvent),
      () => problem(400, "slot.in_past", { "availabilities.1.slotId": "slot.in_past" }),
    );
    await render();
    fillValid();
    click(slotButton("slot_b"));
    await submit();
    expect(slotButton("slot_b").getAttribute("aria-invalid")).toBe("true");
    expect(slotButton("slot_b").className).toContain("border-danger");
    expect(slotButton("slot_a").getAttribute("aria-invalid")).toBeNull();
    expect(m.container.textContent).toContain("Ce créneau est déjà passé.");
  });

  it("lists only enabled servers of the region within the scope, grouped by faction", async () => {
    serve(() => json(publicEvent));
    await render();
    const groups = [...serverSelect().querySelectorAll("optgroup")].map((g) => [
      g.label,
      [...g.querySelectorAll("option")].map((o) => o.value),
    ]);
    expect(groups).toEqual([
      ["Élyséens", ["s_e"]],
      ["Asmodiens", ["s_a"]],
    ]);
    m.unmount();
    serve(() => json({ ...publicEvent, factionScope: "elyos" }));
    await render();
    const values = [...serverSelect().querySelectorAll("option")].map((o) => o.value);
    expect(values).toEqual(["", "s_e"]);
  });

  it("offers only the event's classes and shows faction and role once chosen", async () => {
    serve(() => json({ ...publicEvent, countsByClass: { c1: 1 } }));
    await render();
    expect([...classSelect().querySelectorAll("option")].map((o) => o.value)).toEqual(["", "c1"]);
    expect(m.container.querySelector('[data-testid="faction"]')).toBeNull();
    expect(m.container.querySelector('[data-testid="role"]')).toBeNull();
    fill(serverSelect(), "s_a");
    fill(classSelect(), "c1");
    expect(q('[data-testid="faction"]').textContent).toBe("Faction : Asmodiens");
    expect(q('[data-testid="role"]').textContent).toBe("Rôle : Soin");
  });

  it("shows one toggle per upcoming slot, the star only on a ticked slot, and sends several stars", async () => {
    const calls = serve(() => json(publicEvent), () => json(createdCandidature, 201));
    await render();
    expect(m.container.querySelector('[data-testid="slot-slot_past"]')).toBeNull();
    expect(slotButton("slot_a").getAttribute("aria-pressed")).toBe("false");
    expect(star("slot_a")).toBeNull();
    fillValid();
    expect(slotButton("slot_a").getAttribute("aria-pressed")).toBe("true");
    click(star("slot_a")!);
    click(slotButton("slot_b"));
    click(star("slot_b")!);
    expect(star("slot_b")!.getAttribute("aria-pressed")).toBe("true");
    // Décocher retire aussi la préférence.
    click(slotButton("slot_b"));
    expect(star("slot_b")).toBeNull();
    click(slotButton("slot_b"));
    expect(star("slot_b")!.getAttribute("aria-pressed")).toBe("false");
    click(star("slot_b")!);
    await submit();
    const [post] = posts(calls);
    expect(post.url).toBe(`/api/events/${EVENT_ID}/candidatures`);
    expect(headerOf(post, "X-Link-Secret")).toBeUndefined();
    expect(sent(post)).toEqual({
      pseudo: "Nyra",
      server: "s_a",
      class: "c1",
      cp: 1500,
      availabilities: [
        { slotId: "slot_a", preferred: true },
        { slotId: "slot_b", preferred: true },
      ],
      formToken: "tok",
      website: "",
    });
  });

  it("explains why the button is disabled and hints a CP under the minimum", async () => {
    serve(() => json(publicEvent));
    await render();
    expect(submitButton().disabled).toBe(true);
    expect(m.container.textContent).toContain("Écris ton pseudo : 16 lettres ou chiffres au plus.");
    fill(pseudo(), "Nyra");
    expect(m.container.textContent).toContain("Choisis ton serveur.");
    fill(serverSelect(), "s_e");
    fill(classSelect(), "c2");
    fill(cp(), "12.5");
    expect(m.container.textContent).toContain("Le CP est un nombre entier, de 0 à 999 999.");
    fill(cp(), "900");
    expect(q('[data-testid="cp-below"]').textContent).toMatch(/Sous le CP minimum de 1\s200/);
    expect(m.container.textContent).toContain("Coche au moins un créneau.");
    click(slotButton("slot_b"));
    expect(submitButton().disabled).toBe(false);
  });

  it("restores pseudo, server and class from prefs.candidate and writes them after success, never the CP", async () => {
    localStorage.setItem(CANDIDATE_PREFS_KEY, JSON.stringify({ pseudo: "Kael", server: "s_e", class: "c2" }));
    serve(() => json(publicEvent), () => json(createdCandidature, 201));
    await render();
    expect(pseudo().value).toBe("Kael");
    expect(serverSelect().value).toBe("s_e");
    expect(classSelect().value).toBe("c2");
    expect(cp().value).toBe("");
    fill(pseudo(), "Kaelan");
    fill(serverSelect(), "s_a");
    fill(cp(), "1300");
    click(slotButton("slot_a"));
    await submit();
    expect(JSON.parse(localStorage.getItem(CANDIDATE_PREFS_KEY)!)).toEqual({
      pseudo: "Kaelan",
      server: "s_a",
      class: "c2",
    });
  });

  it("ignores remembered choices that this event does not offer", async () => {
    localStorage.setItem(CANDIDATE_PREFS_KEY, JSON.stringify({ pseudo: "Kael", server: "s_far", class: "c_off" }));
    serve(() => json(publicEvent));
    await render();
    expect(serverSelect().value).toBe("");
    expect(classSelect().value).toBe("");
  });

  it("shows server field errors under their fields and keeps the input", async () => {
    serve(
      () => json(publicEvent),
      () =>
        problem(400, "candidature.server_out_of_region", {
          server: "candidature.server_out_of_region",
          "availabilities.0.slotId": "candidature.slot_unknown",
        }),
    );
    await render();
    fillValid();
    await submit();
    expect(m.container.textContent).toContain("Ce serveur n'est pas dans la région du raid.");
    expect(m.container.textContent).toContain("Un de ces créneaux n'existe plus. Recharge la page.");
    expect(serverSelect().getAttribute("aria-invalid")).toBe("true");
    expect(pseudo().value).toBe("Nyra");
    expect(submitButton().textContent).toBe("Candidater");
  });

  it("shows errors.limit.paused when candidatures are paused", async () => {
    serve(() => json(publicEvent), () => problem(503, "limit.paused"));
    await render();
    fillValid();
    await submit();
    expect(q('[role="alert"]').textContent).toBe("Pause : trop d'activité en une heure. Réessaie plus tard.");
  });

  it("saves the participant link, then goes to /p with the reply", async () => {
    serve(() => json(publicEvent), () => json(createdCandidature, 201));
    await render();
    fillValid();
    await submit();
    expect(JSON.parse(localStorage.getItem(LINKS_KEY)!)).toEqual({
      [EVENT_ID]: { participants: { [CANDIDATURE_ID]: PARTICIPANT } },
    });
    expect(q("h1").textContent).toBe("Ton lien personnel");
    expect(m.container.textContent).toContain(`https://raid.example.org/p/${PARTICIPANT}`);
    expect(m.container.textContent).toContain("Garde ce lien : c'est le seul moyen de revoir ta candidature.");
    expect(q('[data-testid="status"]').textContent).toBe("Statut : En attente de l'organisateur");
    expect(m.container.textContent).not.toContain("Ce navigateur ne gardera pas le lien");
  });

  it("warns on /p when the browser cannot keep the link", async () => {
    spyStorageWrites(new DOMException("blocked", "SecurityError"));
    serve(() => json(publicEvent), () => json({ ...createdCandidature, status: "accepted" }, 201));
    await render();
    fillValid();
    await submit();
    expect(q("h1").textContent).toBe("Ton lien personnel");
    expect(m.container.textContent).toContain("Ce navigateur ne gardera pas le lien : copie-le maintenant.");
    expect(q('[data-testid="status"]').textContent).toBe("Statut : Acceptée");
  });
});

describe("states", () => {
  it("direct mode reads and posts with the secret in the header, never in the URL nor the title", async () => {
    document.title = "aion2-raid-planner";
    const calls = serve(() => json(publicEvent), () => json({ ...createdCandidature, status: "accepted" }, 201));
    await render(`/j/${JOIN}`);
    expect(q('[data-testid="direct"]').textContent).toBe("Lien direct : ta candidature est acceptée d'office.");
    const read = calls.find((c) => c.url === "/api/join")!;
    expect(headerOf(read, "X-Link-Secret")).toBe(JOIN);
    fillValid();
    await submit();
    const [post] = posts(calls);
    expect(post.url).toBe("/api/join/candidatures");
    expect(headerOf(post, "X-Link-Secret")).toBe(JOIN);
    for (const c of calls) expect(c.url).not.toContain(JOIN);
    expect(document.title).not.toContain(JOIN);
  });

  it("direct mode shows the planning of the accepted, the public page never does", async () => {
    const roster = [{ pseudoSrv: "Kael[SA]", class: "c2", teams: [1], availabilities: [{ slotId: "slot_a", preferred: true }] }];
    serve(() => json({ ...publicEvent, acceptedRoster: roster }));
    await render(`/j/${JOIN}`);
    expect(q('[data-testid="accepted"]')).not.toBeNull();
    expect(q('[data-testid="roster-cell-slot_a"]').textContent).toBe("★");
    m.unmount();
    serve(() => json({ ...publicEvent, acceptedRoster: roster }));
    await render(`/e/${EVENT_ID}`);
    expect(q('[data-testid="accepted"]')).toBeNull();
  });

  it("hides the form of a cancelled raid, on /e and on /j", async () => {
    for (const path of [`/e/${EVENT_ID}`, `/j/${JOIN}`]) {
      serve(() => json({ ...publicEvent, status: "cancelled" }));
      await render(path);
      expect(byText(m.container, "Raid annulé")).toBeTruthy();
      expect(m.container.querySelector("form")).toBeNull();
      m.unmount();
    }
    m = mount(<div />);
  });

  it("forgets the event on /e and the direct link on /j when they lead nowhere", async () => {
    const OTHER = "X".repeat(21) + "9";
    const kept = {
      [EVENT_ID]: { organizer: "O".repeat(43), join: JOIN, participants: { [CANDIDATURE_ID]: PARTICIPANT } },
      [OTHER]: { join: "Q".repeat(43), participants: {} },
    };
    const stored = () => JSON.parse(localStorage.getItem(LINKS_KEY)!) as unknown;

    localStorage.setItem(LINKS_KEY, JSON.stringify(kept));
    serve(() => problem(404, "event.not_found"));
    await render(`/e/${EVENT_ID}`);
    expect(byText(m.container, "Retour à l'accueil")).toBeTruthy();
    expect(stored()).toEqual({ [OTHER]: kept[OTHER] });
    m.unmount();

    localStorage.setItem(LINKS_KEY, JSON.stringify(kept));
    serve(() => problem(404, "auth.not_found"));
    await render(`/j/${JOIN}`);
    expect(byText(m.container, "Retour à l'accueil")).toBeTruthy();
    expect(stored()).toEqual({
      ...kept,
      [EVENT_ID]: { organizer: "O".repeat(43), participants: { [CANDIDATURE_ID]: PARTICIPANT } },
    });
    m.unmount();

    // Ni un lien d'un autre droit, ni le réseau, ni une panne ne retirent quoi que ce soit.
    for (const [path, reply] of [
      [`/j/${JOIN}`, () => problem(403, "auth.wrong_right")],
      [`/j/${JOIN}`, () => Promise.reject(new TypeError("Failed to fetch"))],
      [`/e/${EVENT_ID}`, () => problem(500, "server.internal")],
      [`/e/${EVENT_ID}`, () => problem(429, "limit.rate_limited")],
    ] as const) {
      localStorage.setItem(LINKS_KEY, JSON.stringify(kept));
      serve(reply as () => Response);
      await render(path);
      expect(stored(), path).toEqual(kept);
      m.unmount();
    }
    m = mount(<div />);
  });

  it("shows the dead link and forgets it when the link dies under the candidature", async () => {
    const OTHER = "X".repeat(21) + "9";
    const kept = {
      [EVENT_ID]: { organizer: "O".repeat(43), join: JOIN, participants: {} },
      [OTHER]: { join: "Q".repeat(43), participants: {} },
    };
    const stored = () => JSON.parse(localStorage.getItem(LINKS_KEY)!) as unknown;
    for (const [path, code, after] of [
      [`/e/${EVENT_ID}`, "event.not_found", { [OTHER]: kept[OTHER] }],
      [`/j/${JOIN}`, "auth.not_found", { ...kept, [EVENT_ID]: { organizer: "O".repeat(43), participants: {} } }],
      [`/j/${JOIN}`, "auth.wrong_right", kept],
    ] as const) {
      localStorage.setItem(LINKS_KEY, JSON.stringify(kept));
      vi.useFakeTimers({ toFake: ["setInterval", "clearInterval"] });
      const calls = serve(() => json(publicEvent), () => problem(code === "auth.wrong_right" ? 403 : 404, code));
      await render(path);
      fillValid();
      await submit();
      expect(q('[role="alert"]').textContent, code).toBe("Ce lien ne mène à rien. Il a peut-être expiré.");
      expect(byText(m.container, "Retour à l'accueil"), code).toBeTruthy();
      expect(m.container.querySelector("form"), code).toBeNull();
      expect(stored(), code).toEqual(after);
      // Plus aucune relecture.
      const before = calls.length;
      await act(async () => {
        vi.advanceTimersByTime(REFRESH_MS * 2);
      });
      await flush();
      expect(calls.length, code).toBe(before);
      m.unmount();
      vi.useRealTimers();
    }
    m = mount(<div />);
  });

  it("shows the single dead-link message for an unknown id or secret", async () => {
    for (const [path, code] of [
      [`/e/${EVENT_ID}`, "event.not_found"],
      [`/j/${JOIN}`, "auth.not_found"],
    ]) {
      serve(() => problem(404, code));
      await render(path);
      expect(q('[role="alert"]').textContent).toBe("Ce lien ne mène à rien. Il a peut-être expiré.");
      expect(byText(m.container, "Retour à l'accueil").getAttribute("href")).toBe("/");
      expect(m.container.textContent).not.toContain(JOIN);
      m.unmount();
    }
    m = mount(<div />);
  });
});
