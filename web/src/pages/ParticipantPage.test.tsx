// @vitest-environment happy-dom
// Fuseau fixé avant tout calcul de date : UTC+9, sans heure d'été.
process.env.TZ = "Asia/Tokyo";

import { act } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter, Route, Routes, useLocation, useNavigate, type NavigateFunction } from "react-router";
import { resetClientForTests } from "../api/client";
import { LINKS_KEY } from "../api/links";
import type { CandidatureStatus, ParticipantView } from "../api/types";
import { resetLangForTests } from "../i18n";
import { byText, click, fill, key, mount, type Mounted, spyStorageWrites } from "../test/dom";
import {
  CANDIDATURE_ID,
  EVENT_ID,
  PARTICIPANT,
  createdCandidature,
  participantView,
  publicEvent,
  reference,
} from "../test/fixtures";
import { REFRESH_MS } from "./EventPage";
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
      if (url === "/api/participant") return view();
      throw new Error(`unexpected fetch ${url}`);
    }),
  );
  return calls;
}

const reads = (calls: Call[]) => calls.filter((c) => c.url === "/api/participant").length;
const posts = (calls: Call[]) => calls.filter((c) => c.init?.method === "POST");
const headerOf = (c: Call, name: string) => (c.init?.headers as Record<string, string> | undefined)?.[name];
const sent = (c: Call) => JSON.parse(String(c.init?.body)) as Record<string, unknown>;
const viewWith = (patch: Partial<ParticipantView>, event: Partial<ParticipantView["event"]> = {}) => ({
  ...participantView,
  ...patch,
  event: { ...participantView.event, ...event },
});

async function flush() {
  await act(async () => {
    for (let i = 0; i < 10; i++) await Promise.resolve();
  });
}

/** Accès de test au routeur : navigation et état d'historique courant. */
let go: NavigateFunction;
let historyState: unknown;
function Probe() {
  go = useNavigate();
  historyState = useLocation().state;
  return null;
}

async function render(state?: unknown) {
  m = mount(
    <MemoryRouter initialEntries={[{ pathname: `/p/${PARTICIPANT}`, state }]}>
      <Routes>
        <Route
          path="/p/:secret"
          element={
            <>
              <ParticipantPage reference={reference} origin="https://raid.example.org" />
              <Probe />
            </>
          }
        />
        <Route path="/e/:id" element={<p>event page</p>} />
      </Routes>
    </MemoryRouter>,
  );
  await flush();
}

const q = <T extends Element>(sel: string) => m.container.querySelector<T>(sel);
const badge = () => q<HTMLElement>('[data-testid="status-badge"]')!;
const button = (text: string) =>
  [...m.container.querySelectorAll<HTMLButtonElement>("button")].find((b) => b.textContent === text) ?? null;
const dialog = () => document.querySelector<HTMLElement>('[role="dialog"]');
const live = () => q<HTMLElement>('[aria-live="polite"]')!;

async function tick(n = 1) {
  await act(async () => {
    vi.advanceTimersByTime(REFRESH_MS * n);
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

describe("read", () => {
  it("shows the personal link first, then the status, the summary and the candidature", async () => {
    document.title = "aion2-raid-planner";
    const calls = serve(() => json(participantView));
    await render();
    expect(q("h1")!.textContent).toBe("Ton lien personnel");
    expect(m.container.textContent).toContain(`https://raid.example.org/p/${PARTICIPANT}`);
    expect(m.container.textContent).toContain("Garde ce lien : c'est le seul moyen de revoir ta candidature.");
    expect(q('[data-testid="status"]')!.textContent).toBe("Statut : En attente de l'organisateur");
    expect(q('[data-testid="summary"] h2')!.textContent).toBe("Instance Une · mer. 9 janv. 2030");
    const cand = q<HTMLElement>('[data-testid="candidature"]')!;
    expect(cand.textContent).toContain("Nyra");
    expect(cand.textContent).toContain("Server A");
    expect(cand.textContent).toContain("Asmodiens");
    expect(cand.textContent).toContain("Classe Soin");
    expect(cand.textContent).toContain("Soin");
    expect(cand.textContent).toMatch(/1\s500/);
    const slots = [...m.container.querySelectorAll('[data-testid="candidature-slots"] li')].map((l) => l.textContent);
    expect(slots).toEqual(["✓mer. 05:00mer. 8 janv. 2020", "✓mer. 05:00mer. 9 janv. 2030☆préféré"]);
    // Secret en en-tête seulement, jamais dans l'URL ni le titre.
    expect(headerOf(calls[0], "X-Link-Secret")).toBe(PARTICIPANT);
    for (const c of calls) expect(c.url).not.toContain(PARTICIPANT);
    expect(document.title).not.toContain(PARTICIPANT);
  });

  it("labels and styles the four statuses", async () => {
    const expected: [CandidatureStatus, string, string][] = [
      ["pending", "En attente de l'organisateur", "text-pending"],
      ["accepted", "Acceptée", "text-ink-primary"],
      ["rejected", "Refusée", "line-through"],
      ["withdrawn", "Retirée", "line-through"],
    ];
    for (const [status, label, style] of expected) {
      serve(() => json(viewWith({ status })));
      await render();
      expect(badge().textContent, status).toBe(label);
      expect(badge().className, status).toContain(style);
      if (status === "rejected" || status === "withdrawn") expect(badge().className).toContain("text-ink-disabled");
      m.unmount();
    }
    m = mount(<div />);
  });

  it("shows the Discord link only when accepted", async () => {
    serve(() => json(viewWith({ status: "accepted", discordUrl: "https://discord.gg/abcdef" })));
    await render();
    const link = byText(m.container, "Rejoindre le Discord") as HTMLAnchorElement;
    expect(link.getAttribute("href")).toBe("https://discord.gg/abcdef");
    expect(link.getAttribute("rel")).toBe("noopener noreferrer");
    m.unmount();
    serve(() => json(participantView));
    await render();
    expect(m.container.textContent).not.toContain("Rejoindre le Discord");
  });

  it("renders the status from the navigation state before the first read, then clears that state", async () => {
    vi.stubGlobal("fetch", vi.fn(() => new Promise(() => {})));
    await render({ created: { ...createdCandidature, status: "accepted" }, stored: true });
    expect(badge().textContent).toBe("Acceptée");
    expect(q('[aria-busy="true"]')).not.toBeNull();
    // Consommé : un rechargement ne reprendrait pas ce statut.
    expect(historyState).toBeNull();
  });

  it("shows a first network failure, then hides it after a successful read", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval"] });
    let fail = true;
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => {
        if (fail) throw new TypeError("Failed to fetch");
        return json(participantView);
      }),
    );
    await render();
    expect(q('[role="alert"]')!.textContent).toBe("Ça n'a pas marché. Réessaie dans un moment.");
    fail = false;
    await tick();
    expect(q('[role="alert"]')).toBeNull();
    expect(q('[data-testid="candidature"]')).not.toBeNull();
  });

  it("shows the single dead-link message for an unknown or wrong secret", async () => {
    for (const [status, code] of [
      [404, "auth.not_found"],
      [403, "auth.wrong_right"],
    ] as const) {
      serve(() => problem(status, code));
      await render();
      expect(q('[role="alert"]')!.textContent).toBe("Ce lien ne mène à rien. Il a peut-être expiré.");
      expect(byText(m.container, "Retour à l'accueil").getAttribute("href")).toBe("/");
      expect(m.container.textContent).not.toContain(PARTICIPANT);
      m.unmount();
    }
    m = mount(<div />);
  });
});

describe("local links", () => {
  it("saves the participant link after the first read, from any device", async () => {
    serve(() => json(participantView));
    await render();
    expect(JSON.parse(localStorage.getItem(LINKS_KEY)!)).toEqual({
      [EVENT_ID]: { participants: { [CANDIDATURE_ID]: PARTICIPANT } },
    });
    expect(m.container.textContent).not.toContain("Ce navigateur ne gardera pas le lien");
  });

  it("warns to copy the link when the browser cannot keep it", async () => {
    spyStorageWrites(new DOMException("blocked", "SecurityError"));
    serve(() => json(participantView));
    await render();
    expect(m.container.textContent).toContain("Ce navigateur ne gardera pas le lien : copie-le maintenant.");
  });
});

describe("dead link", () => {
  const OTHER = "X".repeat(21) + "9";
  const kept = {
    [EVENT_ID]: { organizer: "O".repeat(43), participants: { [CANDIDATURE_ID]: PARTICIPANT } },
    [OTHER]: { participants: { ["Y".repeat(22)]: "Q".repeat(43) } },
  };
  const stored = () => JSON.parse(localStorage.getItem(LINKS_KEY)!) as unknown;

  it("forgets the secret on this device when the link leads nowhere", async () => {
    localStorage.setItem(LINKS_KEY, JSON.stringify(kept));
    serve(() => problem(404, "auth.not_found"));
    await render();
    expect(byText(m.container, "Retour à l'accueil")).toBeTruthy();
    expect(stored()).toEqual({ ...kept, [EVENT_ID]: { organizer: "O".repeat(43), participants: {} } });
  });

  it("forgets the link kept at the first read when a refresh finds it dead", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval"] });
    let gone = false;
    serve(() => (gone ? problem(404, "auth.not_found") : json(participantView)));
    await render();
    expect(stored()).toEqual({ [EVENT_ID]: { participants: { [CANDIDATURE_ID]: PARTICIPANT } } });
    gone = true;
    await tick();
    expect(byText(m.container, "Retour à l'accueil")).toBeTruthy();
    expect(stored()).toEqual({});
  });

  it("shows the dead link and forgets the secret when the link dies under an edit or a withdrawal", async () => {
    const withdraw = async () => {
      click(button("Me retirer")!);
      const confirm = [...dialog()!.querySelectorAll("button")].find((b) => b.textContent === "Me retirer")!;
      await act(async () => confirm.click());
      await flush();
    };
    const edit = async () => {
      click(button("Modifier")!);
      await act(async () => q<HTMLButtonElement>('button[type="submit"]')!.click());
      await flush();
    };
    for (const [name, gesture] of [
      ["edit", edit],
      ["withdraw", withdraw],
    ] as const) {
      localStorage.setItem(LINKS_KEY, JSON.stringify(kept));
      vi.useFakeTimers({ toFake: ["setInterval", "clearInterval"] });
      const calls = serve(() => json(participantView), () => problem(404, "auth.not_found"));
      await render();
      await gesture();
      expect(q('[role="alert"]')!.textContent, name).toBe("Ce lien ne mène à rien. Il a peut-être expiré.");
      expect(byText(m.container, "Retour à l'accueil"), name).toBeTruthy();
      expect(dialog(), name).toBeNull();
      expect(stored(), name).toEqual({ ...kept, [EVENT_ID]: { organizer: "O".repeat(43), participants: {} } });
      // Plus aucune relecture.
      const before = reads(calls);
      await tick(2);
      expect(reads(calls), name).toBe(before);
      m.unmount();
      vi.useRealTimers();
    }
    m = mount(<div />);
  });

  it("shows the dead link but forgets nothing when a command meets a wrong right", async () => {
    localStorage.setItem(LINKS_KEY, JSON.stringify(kept));
    serve(() => json(participantView), () => problem(403, "auth.wrong_right"));
    await render();
    click(button("Modifier")!);
    await act(async () => q<HTMLButtonElement>('button[type="submit"]')!.click());
    await flush();
    expect(byText(m.container, "Retour à l'accueil")).toBeTruthy();
    expect(stored()).toEqual(kept);
  });

  it("forgets nothing on a wrong right, a network failure, a quota or an outage", async () => {
    const cases: [string, () => Response][] = [
      ["wrong right", () => problem(403, "auth.wrong_right")],
      ["quota", () => problem(429, "limit.rate_limited")],
      ["outage", () => problem(503, "server.unavailable")],
      [
        "network",
        () => {
          throw new TypeError("Failed to fetch");
        },
      ],
    ];
    for (const [name, reply] of cases) {
      localStorage.setItem(LINKS_KEY, JSON.stringify(kept));
      serve(reply);
      await render();
      expect(stored(), name).toEqual(kept);
      m.unmount();
    }
    m = mount(<div />);
  });
});

describe("edit", () => {
  it("opens the prefilled form, sends the secret in the header and goes back to reading", async () => {
    const updated = viewWith({ cp: 2100, availabilities: [{ slotId: "slot_past", preferred: false }, { slotId: "slot_b", preferred: false }] });
    const calls = serve(() => json(participantView), () => json(updated));
    await render();
    click(button("Modifier")!);
    const form = q<HTMLFormElement>("form")!;
    expect(form.querySelector("h2")!.textContent).toBe("Modifier ta candidature");
    expect(form.querySelector<HTMLInputElement>('input[type="text"]')!.value).toBe("Nyra");
    const [server, cls] = form.querySelectorAll<HTMLSelectElement>("select");
    expect([server.value, cls.value]).toEqual(["s_a", "c1"]);
    const cp = form.querySelector<HTMLInputElement>('input[type="number"]')!;
    expect(cp.value).toBe("1500");
    // Pas de honeypot ni de jeton ; le créneau passé n'est pas proposé.
    expect(form.querySelector('input[name="website"]')).toBeNull();
    expect(form.querySelector('[data-testid="slot-slot_past"]')).toBeNull();
    const slotA = form.querySelector<HTMLButtonElement>('[data-testid="slot-slot_a"] button[aria-label^="Disponible"]')!;
    expect(slotA.getAttribute("aria-pressed")).toBe("true");
    expect(form.querySelector('[data-testid="slot-slot_a"] button[aria-label^="Préféré"]')!.getAttribute("aria-pressed")).toBe("true");
    // Décocher le seul créneau à venir reste permis : la disponibilité passée compte.
    click(slotA);
    click(form.querySelector('[data-testid="slot-slot_b"] button[aria-label^="Disponible"]')!);
    fill(cp, "2100");
    const save = form.querySelector<HTMLButtonElement>('button[type="submit"]')!;
    expect(save.textContent).toBe("Enregistrer");
    await act(async () => save.click());
    await flush();
    const [post] = posts(calls);
    expect(post.url).toBe("/api/participant/candidature");
    expect(headerOf(post, "X-Link-Secret")).toBe(PARTICIPANT);
    expect(sent(post)).toEqual({
      pseudo: "Nyra",
      server: "s_a",
      class: "c1",
      cp: 2100,
      availabilities: [{ slotId: "slot_b", preferred: false }],
    });
    expect(q("form")).toBeNull();
    expect(q('[data-testid="candidature"]')!.textContent).toMatch(/2\s100/);
  });

  it("allows saving with no upcoming slot when a past one is kept", async () => {
    serve(() => json(participantView));
    await render();
    click(button("Modifier")!);
    click(q('[data-testid="slot-slot_a"] button[aria-label^="Disponible"]')!);
    expect(q<HTMLButtonElement>('button[type="submit"]')!.disabled).toBe(false);
    m.unmount();
    serve(() => json(viewWith({ availabilities: [{ slotId: "slot_a", preferred: false }] })));
    await render();
    click(button("Modifier")!);
    click(q('[data-testid="slot-slot_a"] button[aria-label^="Disponible"]')!);
    expect(q<HTMLButtonElement>('button[type="submit"]')!.disabled).toBe(true);
    expect(m.container.textContent).toContain("Coche au moins un créneau.");
  });

  it("shows field errors under their fields and keeps the input", async () => {
    serve(
      () => json(participantView),
      () => problem(400, "candidature.server_out_of_region", { server: "candidature.server_out_of_region" }),
    );
    await render();
    click(button("Modifier")!);
    fill(q<HTMLInputElement>('input[type="text"]')!, "Nyrael");
    await act(async () => q<HTMLButtonElement>('button[type="submit"]')!.click());
    await flush();
    expect(m.container.textContent).toContain("Ce serveur n'est pas dans la région du raid.");
    expect(q("select")!.getAttribute("aria-invalid")).toBe("true");
    expect(q<HTMLInputElement>('input[type="text"]')!.value).toBe("Nyrael");
    expect(q<HTMLButtonElement>('button[type="submit"]')!.textContent).toBe("Enregistrer");
  });

  it("goes back to reading on Cancel, without sending", async () => {
    const calls = serve(() => json(participantView));
    await render();
    click(button("Modifier")!);
    click(button("Annuler")!);
    expect(q("form")).toBeNull();
    expect(posts(calls)).toEqual([]);
  });

  it("keeps a server disabled since the candidature in the choices", async () => {
    serve(() => json(viewWith({ server: "s_off" })));
    await render();
    click(button("Modifier")!);
    expect(q<HTMLSelectElement>("select")!.value).toBe("s_off");
    expect(q<HTMLButtonElement>('button[type="submit"]')!.disabled).toBe(false);
  });

  it("keeps a class disabled since the candidature in the choices", async () => {
    serve(() => json(viewWith({ class: "c_off" }, { countsByClass: { c1: 1, c2: 2, c_off: 1 } })));
    await render();
    click(button("Modifier")!);
    expect(m.container.querySelectorAll<HTMLSelectElement>("select")[1].value).toBe("c_off");
    expect(q<HTMLButtonElement>('button[type="submit"]')!.disabled).toBe(false);
  });

  it("keeps a server and a class missing from the reference, labelled by their id", async () => {
    const calls = serve(
      () => json(viewWith({ server: "s_gone", class: "c_gone" }, { countsByClass: { c1: 1, c_gone: 1 } })),
      () => json(participantView),
    );
    await render();
    click(button("Modifier")!);
    const [server, cls] = m.container.querySelectorAll<HTMLSelectElement>("select");
    expect([server.value, cls.value]).toEqual(["s_gone", "c_gone"]);
    expect(server.selectedOptions[0].textContent).toBe("s_gone");
    expect(cls.selectedOptions[0].textContent).toBe("c_gone");
    const save = q<HTMLButtonElement>('button[type="submit"]')!;
    expect(save.disabled).toBe(false);
    await act(async () => save.click());
    await flush();
    expect(sent(posts(calls)[0])).toMatchObject({ server: "s_gone", class: "c_gone" });
  });

  it("gives focus back to Modifier after Cancel and after Save", async () => {
    serve(() => json(participantView), () => json(participantView));
    await render();
    click(button("Modifier")!);
    click(button("Annuler")!);
    expect(document.activeElement).toBe(button("Modifier"));
    click(button("Modifier")!);
    await act(async () => q<HTMLButtonElement>('button[type="submit"]')!.click());
    await flush();
    expect(document.activeElement).toBe(button("Modifier"));
  });

  it("on a state refusal, leaves the form, shows the error and reloads the view", async () => {
    let current = participantView;
    const calls = serve(
      () => json(current),
      () => {
        current = viewWith({}, { status: "locked" });
        return problem(409, "event.locked");
      },
    );
    await render();
    click(button("Modifier")!);
    await act(async () => q<HTMLButtonElement>('button[type="submit"]')!.click());
    await flush();
    expect(q("form")).toBeNull();
    expect(q('[data-testid="action-error"]')!.textContent).toBe(
      "Le raid est verrouillé : ce changement n'est plus possible.",
    );
    expect(reads(calls)).toBe(2);
    expect(button("Modifier")).toBeNull();
    expect(q('[data-testid="locked"]')).not.toBeNull();
  });

  it("drops the edit form and the sheet when the secret changes", async () => {
    const OTHER = "Q".repeat(42) + "6";
    serve(() => json(participantView));
    await render();
    click(button("Modifier")!);
    expect(q("form")).not.toBeNull();
    await act(async () => go(`/p/${OTHER}`));
    await flush();
    expect(q("form")).toBeNull();
    click(button("Me retirer")!);
    expect(dialog()).not.toBeNull();
    await act(async () => go(`/p/${PARTICIPANT}`));
    await flush();
    expect(dialog()).toBeNull();
  });

  it("never refreshes while editing", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval"] });
    const calls = serve(() => json(participantView));
    await render();
    click(button("Modifier")!);
    await tick(3);
    expect(reads(calls)).toBe(1);
    click(button("Annuler")!);
    await tick();
    expect(reads(calls)).toBe(2);
  });
});

describe("states", () => {
  it("locked: no edit, the reason instead, withdrawal still offered", async () => {
    serve(() => json(viewWith({ status: "accepted" }, { status: "locked" })));
    await render();
    expect(button("Modifier")).toBeNull();
    expect(q('[data-testid="locked"]')!.textContent).toBe(
      "Le raid est verrouillé : tu ne peux plus modifier, seulement te retirer.",
    );
    expect(button("Me retirer")).not.toBeNull();
  });

  it("locked and rejected: neither edit nor withdrawal, with its own reason", async () => {
    serve(() => json(viewWith({ status: "rejected" }, { status: "locked" })));
    await render();
    expect(button("Modifier")).toBeNull();
    expect(button("Me retirer")).toBeNull();
    expect(q('[data-testid="locked"]')!.textContent).toBe("Le raid est verrouillé : ta candidature ne se modifie plus.");
  });

  it("rejected: editable while open, never withdrawable", async () => {
    serve(() => json(viewWith({ status: "rejected" })));
    await render();
    expect(button("Modifier")).not.toBeNull();
    expect(button("Me retirer")).toBeNull();
  });

  it("withdrawn: read only, with a link to sign up again", async () => {
    serve(() => json(viewWith({ status: "withdrawn" })));
    await render();
    expect(button("Modifier")).toBeNull();
    expect(button("Me retirer")).toBeNull();
    expect(byText(m.container, "Candidater à nouveau").getAttribute("href")).toBe(`/e/${EVENT_ID}`);
  });

  it("cancelled: banner, read only, no action", async () => {
    serve(() => json(viewWith({ status: "accepted" }, { status: "cancelled" })));
    await render();
    expect(byText(m.container, "Raid annulé")).toBeTruthy();
    expect(q('[data-testid="actions"]')).toBeNull();
    expect(q('[data-testid="candidature"]')).not.toBeNull();
  });
});

describe("withdraw", () => {
  it("asks for confirmation in a sheet, then withdraws", async () => {
    const calls = serve(() => json(participantView), () => json(viewWith({ status: "withdrawn" })));
    await render();
    const trigger = button("Me retirer")!;
    click(trigger);
    const d = dialog()!;
    expect(d.getAttribute("aria-modal")).toBe("true");
    expect(document.getElementById(d.getAttribute("aria-labelledby")!)!.textContent).toBe(
      "Ta candidature sera retirée. Pour revenir, il faudra candidater à nouveau.",
    );
    expect(document.activeElement?.textContent).toBe("Annuler");
    // Échap ferme sans rien envoyer et rend le focus.
    key(document.body, "Escape");
    expect(dialog()).toBeNull();
    expect(document.activeElement).toBe(trigger);
    expect(posts(calls)).toEqual([]);

    click(trigger);
    const confirm = [...dialog()!.querySelectorAll("button")].find((b) => b.textContent === "Me retirer")!;
    expect(confirm.className).toContain("text-danger");
    await act(async () => confirm.click());
    await flush();
    const [post] = posts(calls);
    expect(post.url).toBe("/api/participant/withdrawal");
    expect(headerOf(post, "X-Link-Secret")).toBe(PARTICIPANT);
    expect(sent(post)).toEqual({});
    expect(dialog()).toBeNull();
    expect(badge().textContent).toBe("Retirée");
    expect(live().textContent).toBe("Statut : Retirée");
    expect(byText(m.container, "Candidater à nouveau")).toBeTruthy();
  });

  it("moves focus to the status once the withdraw button is gone", async () => {
    serve(() => json(participantView), () => json(viewWith({ status: "withdrawn" })));
    await render();
    click(button("Me retirer")!);
    const confirm = [...dialog()!.querySelectorAll("button")].find((b) => b.textContent === "Me retirer")!;
    await act(async () => confirm.click());
    await flush();
    expect(document.activeElement).toBe(q('[data-testid="status"]'));
  });

  it("ignores a refresh started before the withdrawal and answered after it", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval"] });
    let first = true;
    let late: (r: Response) => void = () => {};
    vi.stubGlobal(
      "fetch",
      vi.fn(async (_url: string, init?: RequestInit) => {
        if (init?.method === "POST") return json(viewWith({ status: "withdrawn" }));
        if (first) {
          first = false;
          return json(participantView);
        }
        return new Promise<Response>((resolve) => (late = resolve));
      }),
    );
    await render();
    await tick();
    click(button("Me retirer")!);
    const confirm = [...dialog()!.querySelectorAll("button")].find((b) => b.textContent === "Me retirer")!;
    await act(async () => confirm.click());
    await flush();
    expect(badge().textContent).toBe("Retirée");
    await act(async () => late(json(participantView)));
    await flush();
    expect(badge().textContent).toBe("Retirée");
    expect(button("Me retirer")).toBeNull();
  });

  it("while the withdrawal is pending, the sheet stays open and the button is busy", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async (_url: string, init?: RequestInit) =>
        init?.method === "POST" ? new Promise<Response>(() => {}) : json(participantView),
      ),
    );
    await render();
    click(button("Me retirer")!);
    const buttons = () => [...dialog()!.querySelectorAll("button")];
    await act(async () => buttons()[0].click());
    await flush();
    key(document.body, "Escape");
    expect(dialog()).not.toBeNull();
    expect(buttons()[0].textContent).toBe("Envoi…");
    expect(buttons()[0].disabled).toBe(true);
  });

  it("on a state refusal, closes the sheet, shows the error and reloads the view", async () => {
    let current = participantView;
    const calls = serve(
      () => json(current),
      () => {
        current = viewWith({}, { status: "cancelled" });
        return problem(409, "event.closed");
      },
    );
    await render();
    click(button("Me retirer")!);
    const confirm = [...dialog()!.querySelectorAll("button")].find((b) => b.textContent === "Me retirer")!;
    await act(async () => confirm.click());
    await flush();
    expect(dialog()).toBeNull();
    expect(q('[data-testid="action-error"]')!.textContent).toBe(
      "Ce raid est annulé : plus aucun changement n'est possible.",
    );
    expect(reads(calls)).toBe(2);
    expect(byText(m.container, "Raid annulé")).toBeTruthy();
  });

  it("keeps the sheet open with the error when the withdrawal is refused", async () => {
    serve(() => json(participantView), () => problem(429, "limit.rate_limited"));
    await render();
    click(button("Me retirer")!);
    const confirm = [...dialog()!.querySelectorAll("button")].find((b) => b.textContent === "Me retirer")!;
    await act(async () => confirm.click());
    await flush();
    expect(dialog()!.querySelector('[role="alert"]')!.textContent).toBe("Doucement : réessaie dans un moment.");
  });
});

describe("refresh", () => {
  it("refreshes every 10 s and announces only a status change", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval"] });
    let current = participantView;
    const calls = serve(() => json(current));
    await render();
    expect(live().textContent).toBe("");
    await tick();
    expect(reads(calls)).toBe(2);
    expect(live().textContent).toBe("");
    current = viewWith({ status: "accepted" });
    await tick();
    expect(badge().textContent).toBe("Acceptée");
    expect(live().textContent).toBe("Statut : Acceptée");
  });

  it("stops on a terminal code, on cancelled and on withdrawn", async () => {
    const cases: [string, () => Response][] = [
      ["gone", () => problem(404, "auth.not_found")],
      ["cancelled", () => json(viewWith({}, { status: "cancelled" }))],
      ["withdrawn", () => json(viewWith({ status: "withdrawn" }))],
    ];
    for (const [name, reply] of cases) {
      vi.useFakeTimers({ toFake: ["setInterval", "clearInterval"] });
      const calls = serve(reply);
      await render();
      await tick(3);
      expect(reads(calls), name).toBe(1);
      m.unmount();
      vi.useRealTimers();
    }
    m = mount(<div />);
  });

  it("keeps refreshing after a network failure", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval"] });
    let fail = true;
    const calls: Call[] = [];
    vi.stubGlobal(
      "fetch",
      vi.fn(async (url: string, init?: RequestInit) => {
        calls.push({ url, init });
        if (fail) throw new TypeError("Failed to fetch");
        return json({ ...participantView, event: publicEvent });
      }),
    );
    await render();
    expect(q('[aria-busy="true"]')).not.toBeNull();
    fail = false;
    await tick();
    expect(reads(calls)).toBe(2);
    expect(q('[data-testid="candidature"]')).not.toBeNull();
  });
});
