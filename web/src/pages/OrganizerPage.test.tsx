// @vitest-environment happy-dom
// Fuseau fixé avant tout calcul de date : UTC+9, sans heure d'été.
process.env.TZ = "Asia/Tokyo";

import { act } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter, Route, Routes } from "react-router";
import { resetClientForTests } from "../api/client";
import { LINKS_KEY } from "../api/links";
import type { OrganizerCandidature, OrganizerEvent } from "../api/types";
import { resetLangForTests } from "../i18n";
import { byText, click, fill, key, mount, type Mounted, spyStorageWrites } from "../test/dom";
import { EVENT_ID, JOIN, KAEL, LIO, NYRA, NYRA_TWIN, ORGANIZER, TOMA, organizerEvent, reference } from "../test/fixtures";
import { REFRESH_MS } from "./EventPage";
import { FRESH_MS, OrganizerPage } from "./OrganizerPage";

let m: Mounted;
type Call = { url: string; init?: RequestInit };

const json = (body: unknown, status = 200) =>
  new Response(JSON.stringify(body), { status, headers: { "Content-Type": "application/json" } });
const problem = (status: number, code: string) =>
  new Response(JSON.stringify({ type: "about:blank", title: "x", status, code }), {
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
const headerOf = (c: Call, name: string) => (c.init?.headers as Record<string, string> | undefined)?.[name];
const sent = (c: Call) => JSON.parse(String(c.init?.body)) as Record<string, unknown>;

/** Vue avec des candidatures changées par id, et un patch sur l'événement. */
function viewWith(
  patch: Record<string, Partial<OrganizerCandidature>> = {},
  event: Partial<OrganizerEvent["event"]> = {},
  extra: Partial<OrganizerEvent> = {},
): OrganizerEvent {
  return {
    ...organizerEvent,
    ...extra,
    event: { ...organizerEvent.event, ...event },
    candidatures: (extra.candidatures ?? organizerEvent.candidatures).map((c) => ({ ...c, ...patch[c.id] })),
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

const q = <T extends Element>(sel: string) => m.container.querySelector<T>(sel);
const row = (id: string) => q<HTMLButtonElement>(`[data-testid="row-${id}"]`)!;
const rowIds = () =>
  [...m.container.querySelectorAll<HTMLElement>('[data-testid^="row-"]')].map((r) => r.dataset.testid!.slice(4));
const tabs = () => [...m.container.querySelectorAll<HTMLButtonElement>('[role="tab"]')];
const dialogs = () => document.querySelectorAll<HTMLElement>('[role="dialog"]');
const dialog = () => document.querySelector<HTMLElement>('[role="dialog"]');
const inDialog = (text: string) =>
  [...(dialog()?.querySelectorAll<HTMLButtonElement>("button") ?? [])].find((b) => b.textContent === text) ?? null;
const fresh = () =>
  [...m.container.querySelectorAll<HTMLElement>('[data-fresh="true"]')].map((r) => r.dataset.testid!.slice(4));

async function tick(n = 1) {
  await act(async () => {
    vi.advanceTimersByTime(REFRESH_MS * n);
  });
  await flush();
}

async function press(b: HTMLButtonElement | null) {
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
  vi.useRealTimers();
  localStorage.clear();
});

describe("page", () => {
  it("shows the instance and date, the tab bar, and every candidature in the order received", async () => {
    document.title = "aion2-raid-planner";
    const calls = serve(() => json(organizerEvent));
    await render();
    expect(q("h1")!.textContent).toBe("Instance Une · mer. 9 janv. 2030");
    expect(rowIds()).toEqual([NYRA, KAEL, NYRA_TWIN, TOMA, LIO]);
    // Secret en en-tête seulement : ni dans l'URL, ni dans la page, ni dans le titre.
    expect(headerOf(calls[0], "X-Link-Secret")).toBe(ORGANIZER);
    for (const c of calls) expect(c.url).not.toContain(ORGANIZER);
    expect(m.container.textContent).not.toContain(ORGANIZER);
    expect(document.title).not.toContain(ORGANIZER);
  });

  it("follows the tablist pattern with three tabs, Candidatures selected first", async () => {
    serve(() => json(organizerEvent));
    await render();
    const list = q('[role="tablist"]')!;
    expect(list.className).toContain("sticky");
    expect(tabs().map((t) => [t.textContent, t.getAttribute("aria-selected"), t.tabIndex])).toEqual([
      ["Candidatures", "true", 0],
      ["Disponibilités", "false", -1],
      ["Réglages", "false", -1],
    ]);
    // À 320 px, un libellé passe sur deux lignes plutôt que de déborder.
    for (const tab of tabs()) {
      expect(tab.className).toContain("min-w-0");
      expect(tab.className).toContain("[overflow-wrap:anywhere]");
      expect(tab.className).not.toContain("flex-1");
    }
    const panel = q('[role="tabpanel"]')!;
    expect(panel.getAttribute("aria-labelledby")).toBe(tabs()[0].id);
    expect(tabs()[0].getAttribute("aria-controls")).toBe(panel.id);
    // Les onglets inactifs ne pointent vers aucun panneau absent.
    expect(tabs()[1].hasAttribute("aria-controls")).toBe(false);
    expect(tabs()[2].hasAttribute("aria-controls")).toBe(false);
    expect(m.container.querySelectorAll('[role="tabpanel"]')).toHaveLength(1);

    click(tabs()[1]);
    expect(tabs()[1].getAttribute("aria-selected")).toBe("true");
    expect(q('[data-testid="candidatures"]')).toBeNull();
    expect(q('[data-testid="availability"]')).toBeTruthy();

    click(tabs()[2]);
    expect(tabs()[2].getAttribute("aria-selected")).toBe("true");
    expect(q('[data-testid="availability"]')).toBeNull();
    // Réglages : le formulaire de l'onglet (détaillé dans `EventSettings.test.tsx`).
    expect(q('[data-testid="settings"] button[type="submit"]')!.textContent).toBe("Enregistrer");

    // Flèches : la sélection suit le focus, et boucle.
    key(tabs()[2], "ArrowRight");
    expect(tabs()[0].getAttribute("aria-selected")).toBe("true");
    expect(document.activeElement).toBe(tabs()[0]);
    key(tabs()[0], "ArrowRight");
    expect(tabs()[1].getAttribute("aria-selected")).toBe("true");
    expect(document.activeElement).toBe(tabs()[1]);
    key(tabs()[1], "ArrowLeft");
    expect(tabs()[0].getAttribute("aria-selected")).toBe("true");
    key(tabs()[0], "ArrowLeft");
    expect(tabs()[2].getAttribute("aria-selected")).toBe("true");
    key(tabs()[2], "Home");
    expect(tabs()[0].getAttribute("aria-selected")).toBe("true");
    key(tabs()[0], "End");
    expect(tabs()[2].getAttribute("aria-selected")).toBe("true");
  });

  it("cancelled: Réglages is read-only, with the summary and the Discord link or its absence", async () => {
    serve(() => json(viewWith({}, { status: "cancelled" })));
    await render();
    click(tabs()[2]);
    const settings = q<HTMLElement>('[data-testid="settings"]')!;
    expect(settings.querySelector('[data-testid="summary"] h2')!.textContent).toBe("Instance Une · mer. 9 janv. 2030");
    const discord = byText(settings, "https://discord.gg/abcdef") as HTMLAnchorElement;
    expect(discord.getAttribute("href")).toBe("https://discord.gg/abcdef");
    expect(discord.getAttribute("rel")).toBe("noopener noreferrer");
    // Lecture seule : aucun champ ni bouton de réglage ; seul « Dupliquer » reste.
    expect(settings.querySelectorAll("input, select, textarea")).toHaveLength(0);
    expect([...settings.querySelectorAll("button")].map((b) => b.textContent)).toEqual(["Dupliquer"]);
    m.unmount();

    serve(() => json(viewWith({}, { status: "cancelled" }, { discordUrl: undefined })));
    await render();
    click(tabs()[2]);
    expect(q('[data-testid="settings"]')!.textContent).toContain("Aucun lien Discord.");
  });

  it("shows the empty state", async () => {
    serve(() => json(viewWith({}, {}, { candidatures: [] })));
    await render();
    expect(byText(m.container, "Aucune candidature pour l'instant.")).toBeTruthy();
    expect(q('[data-testid="candidatures"]')).toBeNull();
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
      expect(m.container.textContent).not.toContain(ORGANIZER);
      m.unmount();
    }
    m = mount(<div />);
  });

  it("shows a first network failure, then the list after a successful read", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval"] });
    let fail = true;
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => {
        if (fail) throw new TypeError("Failed to fetch");
        return json(organizerEvent);
      }),
    );
    await render();
    expect(q('[aria-busy="true"]')).not.toBeNull();
    expect(q('[role="alert"]')!.textContent).toBe("Ça n'a pas marché. Réessaie dans un moment.");
    fail = false;
    await tick();
    expect(q('[role="alert"]')).toBeNull();
    expect(rowIds()).toHaveLength(5);
  });

  it("cancelled: banner, readable list, no action in the sheet", async () => {
    serve(() => json(viewWith({}, { status: "cancelled" })));
    await render();
    expect(byText(m.container, "Raid annulé")).toBeTruthy();
    expect(rowIds()).toHaveLength(5);
    for (const id of [NYRA, KAEL, TOMA]) {
      click(row(id));
      expect(inDialog("Accepter"), id).toBeNull();
      expect(inDialog("Refuser"), id).toBeNull();
      expect(inDialog("Fermer"), id).not.toBeNull();
      key(document.body, "Escape");
    }
  });
});

describe("rows", () => {
  it("show pseudo, [SRV] with its faction, class and CP, and a badge unless accepted", async () => {
    serve(() => json(organizerEvent));
    await render();
    const nyra = row(NYRA);
    expect(nyra.textContent).toContain("Nyra");
    expect(nyra.textContent).toContain("[SA]");
    expect(nyra.textContent).toContain("Classe Soin");
    expect(nyra.textContent).toMatch(/1\s500/);
    expect(nyra.querySelector(".bg-faction-asmodian")).not.toBeNull();
    expect(nyra.querySelector(".tabular-nums")!.textContent).toMatch(/1\s500/);
    expect(nyra.getAttribute("aria-haspopup")).toBe("dialog");
    // La faction ne passe pas par la couleur seule : son nom est lu par le lecteur d'écran.
    const faction = nyra.querySelector<HTMLElement>('[data-testid="faction"]')!;
    expect(faction.textContent).toBe("Asmodiens");
    expect(faction.className).toContain("sr-only");
    expect(row(KAEL).querySelector('[data-testid="faction"]')!.textContent).toBe("Élyséens");

    const badge = (id: string) => row(id).querySelector<HTMLElement>('[data-testid="status-badge"]');
    expect(row(KAEL).querySelector(".bg-faction-elyos")).not.toBeNull();
    expect(row(KAEL).textContent).toContain("[SE]");
    // Chaque statut a son badge : couleur doublée du libellé.
    for (const [id, label, style] of [
      [KAEL, "Acceptée", "text-available"],
      [NYRA, "En attente", "text-pending"],
      [TOMA, "Refusée", "text-danger"],
      [LIO, "Retirée", "text-ink-disabled"],
    ]) {
      expect(badge(id)!.textContent, id).toBe(label);
      expect(badge(id)!.className, id).toContain(style);
    }
    expect(badge(TOMA)!.className).not.toContain("line-through");
    expect(badge(LIO)!.className).toContain("line-through");
  });

  it("flag duplicates with a danger badge on both lines", async () => {
    serve(() => json(organizerEvent));
    await render();
    const dup = (id: string) => row(id).querySelector<HTMLElement>('[data-testid="duplicate-badge"]');
    for (const id of [NYRA, NYRA_TWIN]) {
      expect(dup(id)!.textContent, id).toBe("doublon");
      expect(dup(id)!.className, id).toContain("text-danger");
    }
    for (const id of [KAEL, TOMA, LIO]) expect(dup(id), id).toBeNull();
  });

  it("fall back to the ids for a server and a class missing from the reference", async () => {
    serve(() =>
      json(viewWith({ [NYRA]: { server: "s_gone", class: "c_gone", role: "c_gone", pseudoSrv: "Nyra[s_gone]" } })),
    );
    await render();
    expect(row(NYRA).textContent).toContain("[s_gone]");
    expect(row(NYRA).textContent).toContain("c_gone");
    expect(row(NYRA).querySelector('[data-testid="faction"]')).toBeNull();
    // Dans la feuille, l'id de classe n'est pas répété en guise de rôle.
    click(row(NYRA));
    expect(dialog()!.textContent).toContain("c_gone");
    expect(dialog()!.textContent).not.toContain("c_gonec_gone");
  });
});

describe("sheet", () => {
  it("opens on a tap with Pseudo[SRV] to copy and the candidature details", async () => {
    const writeText = vi.fn(async () => {});
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true, writable: true });
    serve(() => json(organizerEvent));
    await render();
    const trigger = row(NYRA);
    click(trigger);
    expect(dialogs()).toHaveLength(1);
    const d = dialog()!;
    expect(d.getAttribute("aria-modal")).toBe("true");
    expect(document.getElementById(d.getAttribute("aria-labelledby")!)!.textContent).toBe("Nyra");
    const chip = d.querySelector<HTMLButtonElement>('button[aria-label="Copier : Nyra[SA]"]')!;
    expect(chip.textContent).toContain("Nyra[SA]");
    await press(chip);
    expect(writeText).toHaveBeenCalledWith("Nyra[SA]");
    expect(d.textContent).toContain("Server A");
    expect(d.textContent).toContain("Asmodiens");
    expect(d.textContent).toContain("Classe Soin");
    expect(d.textContent).toContain("Soin");
    expect(d.textContent).toMatch(/1\s500/);
    expect(d.textContent).toContain("lien public");
    expect(d.querySelector('[data-testid="sheet-status"]')!.textContent).toBe("doublonEn attente");
    const slots = [...d.querySelectorAll('[data-testid="sheet-slots"] li')].map((l) => l.textContent);
    expect(slots).toEqual(["✓mer. 05:00mer. 8 janv. 2020", "✓mer. 05:00mer. 9 janv. 2030☆préféré"]);
    // Fermer rend le focus à la ligne.
    click(inDialog("Fermer")!);
    expect(dialog()).toBeNull();
    expect(document.activeElement).toBe(trigger);
  });

  it("shows the direct origin", async () => {
    serve(() => json(organizerEvent));
    await render();
    click(row(KAEL));
    expect(dialog()!.textContent).toContain("lien direct");
  });

  it("still offers the decisions on a locked raid", async () => {
    serve(() => json(viewWith({}, { status: "locked" })));
    await render();
    click(row(NYRA));
    expect(inDialog("Accepter")).not.toBeNull();
    key(document.body, "Escape");
    click(row(KAEL));
    expect(inDialog("Refuser")).not.toBeNull();
    expect(inDialog("Accepter")).toBeNull();
  });

  it("offers only the actions AD-4 allows", async () => {
    serve(() => json(organizerEvent));
    await render();
    const cases: [string, string[]][] = [
      [NYRA, ["Accepter", "Refuser", "Fermer"]],
      [KAEL, ["Refuser", "Fermer"]],
      [TOMA, ["Accepter", "Fermer"]],
      [LIO, ["Fermer"]],
    ];
    for (const [id, actions] of cases) {
      click(row(id));
      const buttons = [...dialog()!.querySelectorAll('[data-testid="sheet-actions"] button')].map((b) => b.textContent);
      expect(buttons, id).toEqual(actions);
      key(document.body, "Escape");
    }
    click(row(LIO));
    expect(dialog()!.textContent).toContain("Le joueur s'est retiré : plus rien à décider.");
  });

  it("accepts: sends the id in the body and the secret in the header, then applies the view", async () => {
    const accepted = viewWith({ [NYRA]: { status: "accepted" } });
    const calls = serve(() => json(organizerEvent), () => json(accepted));
    await render();
    click(row(NYRA));
    const accept = inDialog("Accepter")!;
    expect(accept.className).toContain("bg-accent");
    // Un toucher donne le focus au bouton (sauf Safari) : il va disparaître avec la décision.
    act(() => accept.focus());
    await press(accept);
    const [post] = posts(calls);
    expect(post.url).toBe("/api/organizer/candidature-status");
    expect(headerOf(post, "X-Link-Secret")).toBe(ORGANIZER);
    expect(sent(post)).toEqual({ candidatureId: NYRA, status: "accepted" });
    expect(row(NYRA).querySelector('[data-testid="status-badge"]')!.textContent).toBe("Acceptée");
    // La feuille relit sa candidature : statut à jour, « Accepter » parti, focus sur le statut.
    expect(dialog()!.querySelector('[data-testid="sheet-status"]')!.textContent).toBe("doublonAcceptée");
    expect(inDialog("Accepter")).toBeNull();
    expect(inDialog("Refuser")).not.toBeNull();
    expect(document.activeElement).toBe(dialog()!.querySelector('[data-testid="sheet-status"]'));
    // La réponse d'une commande ne pose aucun liseré.
    expect(fresh()).toEqual([]);
  });

  it("rejects only after a destructive confirmation in the same sheet", async () => {
    const rejected = viewWith({ [KAEL]: { status: "rejected" } });
    const calls = serve(() => json(organizerEvent), () => json(rejected));
    await render();
    click(row(KAEL));
    click(inDialog("Refuser")!);
    expect(dialogs()).toHaveLength(1);
    const d = dialog()!;
    const message = document.getElementById(d.getAttribute("aria-labelledby")!)!;
    expect(message.textContent).toBe("Kael[SE] verra sa candidature refusée. Tu pourras encore l'accepter plus tard.");
    expect(posts(calls)).toEqual([]);
    expect(document.activeElement?.textContent).toBe("Annuler");
    // « Annuler » revient au détail, sans rien envoyer.
    click(inDialog("Annuler")!);
    expect(inDialog("Accepter")).toBeNull();
    expect(document.activeElement).toBe(inDialog("Refuser"));
    expect(posts(calls)).toEqual([]);

    click(inDialog("Refuser")!);
    const confirm = inDialog("Refuser")!;
    expect(confirm.className).toContain("text-danger");
    await press(confirm);
    expect(sent(posts(calls)[0])).toEqual({ candidatureId: KAEL, status: "rejected" });
    expect(row(KAEL).querySelector('[data-testid="status-badge"]')!.textContent).toBe("Refusée");
    expect(dialog()!.querySelector('[data-testid="sheet-status"]')!.textContent).toBe("Refusée");
    expect(inDialog("Accepter")).not.toBeNull();
  });

  it("while a decision is pending, the buttons are busy and Escape does nothing", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async (_url: string, init?: RequestInit) =>
        init?.method === "POST" ? new Promise<Response>(() => {}) : json(organizerEvent),
      ),
    );
    await render();
    click(row(NYRA));
    await press(inDialog("Accepter"));
    expect(inDialog("Envoi…")!.disabled).toBe(true);
    expect(inDialog("Refuser")!.disabled).toBe(true);
    key(document.body, "Escape");
    expect(dialog()).not.toBeNull();
  });

  it("on a state refusal, shows the error, closes the sheet and reloads the view", async () => {
    for (const code of ["event.closed", "candidature.transition_forbidden", "candidature.not_found"]) {
      let current = organizerEvent;
      const calls = serve(
        () => json(current),
        () => {
          current = viewWith({ [NYRA]: { status: "withdrawn" } });
          return problem(code === "candidature.not_found" ? 404 : 409, code);
        },
      );
      await render();
      click(row(NYRA));
      await press(inDialog("Accepter"));
      expect(dialog(), code).toBeNull();
      expect(q('[data-testid="action-error"]'), code).not.toBeNull();
      expect(reads(calls), code).toBe(2);
      expect(row(NYRA).querySelector('[data-testid="status-badge"]')!.textContent, code).toBe("Retirée");
      m.unmount();
    }
    m = mount(<div />);
  });

  it("names the state refusal in the page", async () => {
    serve(
      () => json(organizerEvent),
      () => problem(409, "candidature.transition_forbidden"),
    );
    await render();
    click(row(NYRA));
    await press(inDialog("Accepter"));
    expect(q('[data-testid="action-error"]')!.textContent).toBe(
      "Ce changement n'est pas possible pour cette candidature.",
    );
  });

  it("keeps the state refusal through its reload, then clears it on the next refresh", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval"] });
    let current = organizerEvent;
    const calls = serve(
      () => json(current),
      () => {
        current = viewWith({ [NYRA]: { status: "withdrawn" } });
        return problem(409, "candidature.transition_forbidden");
      },
    );
    await render();
    click(row(NYRA));
    await press(inDialog("Accepter"));
    // La relecture lancée par le refus a répondu : le message reste.
    expect(reads(calls)).toBe(2);
    expect(q('[data-testid="action-error"]')).not.toBeNull();
    await tick();
    expect(reads(calls)).toBe(3);
    expect(q('[data-testid="action-error"]')).toBeNull();
  });

  it("clears the state refusal when a sheet is opened", async () => {
    serve(
      () => json(organizerEvent),
      () => problem(409, "event.closed"),
    );
    await render();
    click(row(NYRA));
    await press(inDialog("Accepter"));
    expect(q('[data-testid="action-error"]')).not.toBeNull();
    click(row(KAEL));
    expect(q('[data-testid="action-error"]')).toBeNull();
  });

  it("brings focus back into the sheet after an error that left it on the page body", async () => {
    let answer: (r: Response) => void = () => {};
    vi.stubGlobal(
      "fetch",
      vi.fn(async (_url: string, init?: RequestInit) =>
        init?.method === "POST" ? new Promise<Response>((resolve) => (answer = resolve)) : json(organizerEvent),
      ),
    );
    await render();
    click(row(NYRA));
    // Un navigateur retire le focus d'un bouton désactivé pendant l'envoi ; happy-dom ne le fait
    // pas : le focus est mis sur la page avant le toucher, `click()` ne le déplaçant pas.
    act(() => (document.activeElement as HTMLElement).blur());
    expect(document.activeElement).toBe(document.body);
    await press(inDialog("Accepter"));
    expect(document.activeElement).toBe(document.body);
    await act(async () => answer(problem(429, "limit.rate_limited")));
    await flush();
    expect(dialog()!.querySelector('[role="alert"]')).not.toBeNull();
    expect(document.activeElement).toBe(inDialog("Accepter"));

    // Même chose dans l'étape de confirmation : le bouton « Refuser » de la confirmation.
    click(inDialog("Refuser")!);
    expect(document.activeElement).toBe(inDialog("Annuler"));
    act(() => (document.activeElement as HTMLElement).blur());
    await press(inDialog("Refuser"));
    expect(document.activeElement).toBe(document.body);
    await act(async () => answer(problem(429, "limit.rate_limited")));
    await flush();
    expect(document.activeElement).toBe(inDialog("Refuser"));
    expect(inDialog("Annuler")).not.toBeNull();
  });

  it("disables the decisions while a Réglages command is in flight", async () => {
    let answer: (r: Response) => void = () => {};
    vi.stubGlobal(
      "fetch",
      vi.fn(async (_url: string, init?: RequestInit) =>
        init?.method === "POST" ? new Promise<Response>((resolve) => (answer = resolve)) : json(organizerEvent),
      ),
    );
    await render();
    click(tabs()[2]);
    fill(q<HTMLTextAreaElement>('[data-testid="settings"] textarea')!, "Go");
    await press(q<HTMLButtonElement>('[data-testid="settings"] button[type="submit"]'));
    click(tabs()[0]);
    click(row(NYRA));
    expect(inDialog("Accepter")!.disabled).toBe(true);
    expect(inDialog("Refuser")!.disabled).toBe(true);
    await act(async () => answer(json(viewWith({}, { note: "Go" }))));
    await flush();
    expect(inDialog("Accepter")!.disabled).toBe(false);
    expect(inDialog("Refuser")!.disabled).toBe(false);
  });

  it("keeps the sheet open with the error on another refusal", async () => {
    serve(
      () => json(organizerEvent),
      () => problem(429, "limit.rate_limited"),
    );
    await render();
    click(row(NYRA));
    await press(inDialog("Accepter"));
    expect(dialog()!.querySelector('[role="alert"]')!.textContent).toBe("Doucement : réessaie dans un moment.");
    expect(inDialog("Accepter")!.disabled).toBe(false);
  });

  it("shows the dead link when the link dies under a decision", async () => {
    serve(
      () => json(organizerEvent),
      () => problem(404, "auth.not_found"),
    );
    await render();
    click(row(NYRA));
    await press(inDialog("Accepter"));
    expect(dialog()).toBeNull();
    expect(byText(m.container, "Retour à l'accueil")).toBeTruthy();
  });
});

describe("refresh", () => {
  it("keeps refreshing every 10 s while the sheet is open, and the sheet follows its candidature", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval"] });
    let current = organizerEvent;
    const calls = serve(() => json(current));
    await render();
    click(row(NYRA));
    expect(inDialog("Accepter")).not.toBeNull();
    current = viewWith({ [NYRA]: { status: "withdrawn" } });
    await tick();
    expect(reads(calls)).toBe(2);
    expect(dialog()!.querySelector('[data-testid="sheet-status"]')!.textContent).toBe("doublonRetirée");
    expect(inDialog("Accepter")).toBeNull();
    expect(inDialog("Refuser")).toBeNull();
  });

  it("drops the confirmation step when a refresh makes the rejection impossible", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval"] });
    let current = organizerEvent;
    serve(() => json(current));
    await render();
    click(row(KAEL));
    click(inDialog("Refuser")!);
    expect(inDialog("Annuler")).not.toBeNull();
    current = viewWith({ [KAEL]: { status: "withdrawn" } });
    await tick();
    expect(inDialog("Annuler")).toBeNull();
    expect(inDialog("Refuser")).toBeNull();
    expect(dialog()!.textContent).toContain("Retirée");
  });

  it("marks a new line with a gold edge for 3 s, never on the first load", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval", "setTimeout", "clearTimeout"] });
    const [first, ...rest] = organizerEvent.candidatures;
    let current = viewWith({}, {}, { candidatures: [first] });
    serve(() => json(current));
    await render();
    expect(rowIds()).toEqual([NYRA]);
    expect(fresh()).toEqual([]);
    expect(row(NYRA).className).toContain("border-l-transparent");

    current = viewWith({}, {}, { candidatures: [first, rest[0]] });
    await tick();
    expect(rowIds()).toEqual([NYRA, KAEL]);
    expect(fresh()).toEqual([KAEL]);
    expect(row(KAEL).className).toContain("border-l-accent");
    // Le liseré s'efface en transition, coupée par `prefers-reduced-motion`.
    expect(row(KAEL).className).toContain("motion-safe:transition-colors");

    // Une relecture sans nouveauté ne relance rien ; au bout de 3 s, le liseré disparaît.
    await act(async () => {
      vi.advanceTimersByTime(FRESH_MS - 1);
    });
    expect(fresh()).toEqual([KAEL]);
    await act(async () => {
      vi.advanceTimersByTime(1);
    });
    expect(fresh()).toEqual([]);
    expect(row(KAEL).className).toContain("border-l-transparent");
  });

  it("never marks a line that arrives with a command response", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval", "setTimeout", "clearTimeout"] });
    const [first, second] = organizerEvent.candidatures;
    const calls = serve(
      () => json(viewWith({}, {}, { candidatures: [first, second] })),
      () => json(viewWith({ [NYRA]: { status: "accepted" } }, {}, { candidatures: [first, second] })),
    );
    // Premier chargement avec une seule ligne, puis la commande en rend deux.
    vi.mocked(fetch).mockImplementationOnce(async (url, init) => {
      calls.push({ url: String(url), init });
      return json(viewWith({}, {}, { candidatures: [first] }));
    });
    await render();
    expect(rowIds()).toEqual([NYRA]);
    click(row(NYRA));
    await press(inDialog("Accepter"));
    expect(rowIds()).toEqual([NYRA, KAEL]);
    expect(fresh()).toEqual([]);
    // La relecture suivante la connaît déjà : toujours rien.
    await tick();
    expect(fresh()).toEqual([]);
  });

  it("ignores a refresh started before a decision and answered after it", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval"] });
    let first = true;
    let late: (r: Response) => void = () => {};
    vi.stubGlobal(
      "fetch",
      vi.fn(async (_url: string, init?: RequestInit) => {
        if (init?.method === "POST") return json(viewWith({ [NYRA]: { status: "accepted" } }));
        if (first) {
          first = false;
          return json(organizerEvent);
        }
        return new Promise<Response>((resolve) => (late = resolve));
      }),
    );
    await render();
    await tick();
    click(row(NYRA));
    await press(inDialog("Accepter"));
    expect(row(NYRA).querySelector('[data-testid="status-badge"]')!.textContent).toBe("Acceptée");
    await act(async () => late(json(organizerEvent)));
    await flush();
    expect(row(NYRA).querySelector('[data-testid="status-badge"]')!.textContent).toBe("Acceptée");
    expect(inDialog("Accepter")).toBeNull();
  });

  it("does not refresh while a decision is in flight", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval"] });
    let answer: (r: Response) => void = () => {};
    const calls: Call[] = [];
    vi.stubGlobal(
      "fetch",
      vi.fn(async (url: string, init?: RequestInit) => {
        calls.push({ url, init });
        if (init?.method === "POST") return new Promise<Response>((resolve) => (answer = resolve));
        return json(organizerEvent);
      }),
    );
    await render();
    click(row(NYRA));
    await press(inDialog("Accepter"));
    await tick(2);
    expect(reads(calls)).toBe(1);
    await act(async () => answer(json(viewWith({ [NYRA]: { status: "accepted" } }))));
    await flush();
    await tick();
    expect(reads(calls)).toBe(2);
  });

  it("does not refresh while the page is hidden", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval"] });
    const calls = serve(() => json(organizerEvent));
    await render();
    const visibility = vi.spyOn(document, "visibilityState", "get").mockReturnValue("hidden");
    await tick(3);
    expect(reads(calls)).toBe(1);
    visibility.mockReturnValue("visible");
    await tick();
    expect(reads(calls)).toBe(2);
  });

  it("stops on a terminal code and on a cancelled raid", async () => {
    const cases: [string, () => Response][] = [
      ["gone", () => problem(404, "auth.not_found")],
      ["cancelled", () => json(viewWith({}, { status: "cancelled" }))],
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
    let fail = false;
    const calls: Call[] = [];
    vi.stubGlobal(
      "fetch",
      vi.fn(async (url: string, init?: RequestInit) => {
        calls.push({ url, init });
        if (fail) throw new TypeError("Failed to fetch");
        return json(organizerEvent);
      }),
    );
    await render();
    fail = true;
    await tick();
    // Les dernières données restent, sans alerte.
    expect(rowIds()).toHaveLength(5);
    expect(q('[role="alert"]')).toBeNull();
    fail = false;
    await tick();
    expect(reads(calls)).toBe(3);
  });
});

describe("availability tab", () => {
  /** Écran assez large pour la matrice. */
  function wide() {
    vi.stubGlobal(
      "matchMedia",
      (query: string) =>
        ({
          matches: query === "(min-width: 22.5em)",
          media: query,
          addEventListener: () => {},
          removeEventListener: () => {},
        }) as unknown as MediaQueryList,
    );
  }
  const gridRows = () =>
    [...m.container.querySelectorAll<HTMLElement>('[data-testid^="grid-row-"]')].map((r) => r.dataset.testid!.slice(9));
  const gridButton = (id: string) => q<HTMLButtonElement>(`[data-testid="grid-row-${id}"] th button`)!;

  it("opens the candidature sheet from a matrix row, and focus comes back to it", async () => {
    wide();
    serve(() => json(organizerEvent));
    await render();
    click(tabs()[1]);
    expect(gridRows()).toEqual([KAEL, NYRA_TWIN, NYRA]);
    gridButton(NYRA).focus();
    click(gridButton(NYRA));
    expect(dialogs()).toHaveLength(1);
    expect(dialog()!.querySelector("h2")!.textContent).toBe("Nyra");
    expect(inDialog("Accepter")).toBeTruthy();
    click(inDialog("Fermer")!);
    expect(dialog()).toBeNull();
    expect(document.activeElement).toBe(gridButton(NYRA));
  });

  it("keeps the view, the filter and the picked slot across a tab switch", async () => {
    wide();
    serve(() => json(organizerEvent));
    await render();
    click(tabs()[1]);
    click(q('[data-testid="col-slot_b"]')!);
    click(q('[data-testid="filter-pending"]')!);
    click(q('[data-testid="view-slot"]')!);
    click(tabs()[0]);
    click(tabs()[1]);
    expect(q('[data-testid="view-slot"]')!.getAttribute("aria-pressed")).toBe("true");
    expect(q('[data-testid="filter-pending"]')!.getAttribute("aria-pressed")).toBe("true");
    expect(q('[data-testid="chip-slot_b"]')!.getAttribute("aria-pressed")).toBe("true");
    click(tabs()[2]);
    click(tabs()[1]);
    click(q('[data-testid="view-matrix"]')!);
    expect(q('[data-testid="col-slot_b"]')!.getAttribute("aria-pressed")).toBe("true");
    expect(gridRows()).toEqual([NYRA]);
  });

  it("returns focus to the panel heading when the row behind the sheet leaves the grid", async () => {
    wide();
    serve(
      () => json(organizerEvent),
      () => json(viewWith({ [NYRA]: { status: "accepted" } })),
    );
    await render();
    click(tabs()[1]);
    click(q('[data-testid="filter-pending"]')!);
    click(gridButton(NYRA));
    await press(inDialog("Accepter"));
    // Acceptée : elle sort du filtre « En attente ».
    expect(q(`[data-testid="grid-row-${NYRA}"]`)).toBeNull();
    click(inDialog("Fermer")!);
    expect(dialog()).toBeNull();
    const h2 = q<HTMLHeadingElement>('[data-testid="availability"] h2')!;
    expect(h2.textContent).toBe("Disponibilités");
    expect(document.activeElement).toBe(h2);
  });

  it("is read-only on a locked or cancelled raid: no field, no lock button", async () => {
    wide();
    for (const status of ["locked", "cancelled"] as const) {
      serve(() => json(viewWith({}, { status })));
      await render();
      click(tabs()[1]);
      const panel = q<HTMLElement>('[data-testid="availability"]')!;
      expect(gridRows()).toEqual([KAEL, NYRA_TWIN, NYRA]);
      expect(panel.querySelectorAll("input, select, textarea, form")).toHaveLength(0);
      const labels = [...panel.querySelectorAll("button")].map((b) => b.textContent ?? "");
      expect(labels.some((l) => /Verrouiller|Accepter|Refuser|Enregistrer/.test(l)), status).toBe(false);
      expect(panel.querySelectorAll("td button")).toHaveLength(0);
      m.unmount();
    }
    serve(() => json(organizerEvent));
    await render();
  });

  it("a refresh that changes a status never moves the other rows; a new row comes last with the gold edge", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval", "setTimeout", "clearTimeout"] });
    wide();
    let current = organizerEvent;
    serve(() => json(current));
    await render();
    click(tabs()[1]);
    expect(gridRows()).toEqual([KAEL, NYRA_TWIN, NYRA]);

    const NEW = "Z".repeat(21) + "9";
    current = viewWith(
      { [NYRA]: { status: "accepted", availabilities: [{ slotId: "slot_a", preferred: true }, { slotId: "slot_b", preferred: false }] } },
      {},
      {
        candidatures: [
          ...organizerEvent.candidatures,
          { ...organizerEvent.candidatures[1], id: NEW, pseudo: "Zed", pseudoSrv: "Zed[SE]", status: "pending" },
        ],
      },
    );
    await tick();
    // Nyra, acceptée et cochée partout, serait triée en tête : elle reste à sa place.
    expect(gridRows()).toEqual([KAEL, NYRA_TWIN, NYRA, NEW]);
    expect(q(`[data-testid="grid-row-${NYRA}"] [data-testid="pending-badge"]`)).toBeNull();
    expect(q<HTMLElement>(`[data-testid="grid-row-${NEW}"]`)!.dataset.fresh).toBe("true");
    expect(gridButton(NEW).className).toContain("border-l-accent");
    await act(async () => {
      vi.advanceTimersByTime(FRESH_MS);
    });
    expect(gridButton(NEW).className).toContain("border-l-transparent");

    // Une ligne refusée au rafraîchissement sort de la grille, les autres ne bougent pas.
    current = viewWith({ [NYRA_TWIN]: { status: "rejected" } }, {}, { candidatures: current.candidatures });
    await tick();
    expect(gridRows()).toEqual([KAEL, NYRA, NEW]);

    // Rouvrir l'onglet retrie.
    click(tabs()[0]);
    click(tabs()[1]);
    expect(gridRows()).toEqual([NYRA, KAEL, NEW]);
  });
});

describe("locked tabs", () => {
  const empty = { starters: [], bench: [], classWarnings: {} };
  const groups: OrganizerEvent["groups"] = [
    { id: "g1", slotId: "slot_a", occurrence: 1, retained: true, parties: 2, partySize: 5, roomLabel: "I1", roomName: "I1-K7Q2", ...empty },
    // En sommeil : ni numéro ni ligne.
    { id: "g2", slotId: "slot_a", occurrence: 2, retained: false, parties: 2, partySize: 5, roomLabel: "I1", roomName: "I1-AAAA", ...empty },
    { id: "g3", slotId: "slot_b", occurrence: 1, retained: true, parties: 2, partySize: 5, roomLabel: "I1", roomName: "I1-M3NP", ...empty },
  ];
  const locked = () => viewWith({}, { status: "locked" }, { groups });
  const opened = () => viewWith({}, {}, { groups });
  const labels = () => tabs().map((t) => t.textContent);

  it("adds Composition and Salle between Disponibilités and Réglages on a locked raid only", async () => {
    serve(() => json(locked()));
    await render();
    expect(labels()).toEqual(["Candidatures", "Disponibilités", "Composition", "Salle", "Réglages"]);
    // Le motif tablist couvre les cinq onglets.
    key(tabs()[0], "End");
    expect(tabs()[4].getAttribute("aria-selected")).toBe("true");
    key(tabs()[4], "ArrowLeft");
    expect(tabs()[3].getAttribute("aria-selected")).toBe("true");
    m.unmount();
    for (const status of ["open", "cancelled"] as const) {
      serve(() => json(viewWith({}, { status }, { groups })));
      await render();
      expect(labels(), status).toEqual(["Candidatures", "Disponibilités", "Réglages"]);
      m.unmount();
    }
    m = mount(<div />);
  });

  it("list the retained groups G1… in Salle, without any action", async () => {
    serve(() => json(locked()));
    await render();
    click(tabs()[3]);
    const panel = q<HTMLElement>('[data-testid="room"]')!;
    expect(panel.querySelector("h2")!.textContent).toBe(tabs()[3].textContent);
    expect([...panel.querySelectorAll("li")].map((li) => li.textContent)).toEqual([
      "G1 · mer. 05:00 · I1-K7Q2",
      "G2 · ven. 05:00 · I1-M3NP",
    ]);
    // Le groupe en sommeil n'a pas de ligne, mais il est compté.
    expect(q('[data-testid="room-sleeping"]')!.textContent).toBe("1 groupe en sommeil, repris s'il est reverrouillé.");
    expect(panel.querySelectorAll("button, input, a")).toHaveLength(0);
  });

  it("go back to Disponibilités when the current tab disappears on refresh", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval", "setTimeout", "clearTimeout"] });
    let current = locked();
    serve(() => json(current));
    await render();
    click(tabs()[3]);
    expect(q('[data-testid="room"]')).not.toBeNull();
    current = opened();
    await tick();
    expect(labels()).toEqual(["Candidatures", "Disponibilités", "Réglages"]);
    expect(tabs()[1].getAttribute("aria-selected")).toBe("true");
    expect(q('[data-testid="availability"]')).not.toBeNull();
    // Reverrouillé : l'onglet courant reste Disponibilités.
    current = locked();
    await tick();
    expect(tabs()[1].getAttribute("aria-selected")).toBe("true");
    // Réglages survit au changement d'état.
    click(tabs()[4]);
    current = opened();
    await tick();
    expect(tabs()[2].textContent).toBe("Réglages");
    expect(tabs()[2].getAttribute("aria-selected")).toBe("true");
  });

  it("offer Verrouiller in the grid of an open raid only", async () => {
    serve(() => json(opened()));
    await render();
    click(tabs()[1]);
    expect(q('[data-testid="lock-open"]')!.textContent).toBe("Verrouiller");
    m.unmount();
    for (const status of ["locked", "cancelled"] as const) {
      serve(() => json(viewWith({}, { status }, { groups })));
      await render();
      click(tabs()[1]);
      expect(q('[data-testid="availability"]'), status).not.toBeNull();
      expect(q('[data-testid="lock-open"]'), status).toBeNull();
      m.unmount();
    }
    m = mount(<div />);
  });

  it("close an open lock sheet when a refresh says the raid is locked or cancelled", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval", "setTimeout", "clearTimeout"] });
    for (const status of ["locked", "cancelled"] as const) {
      let current = opened();
      const calls = serve(() => json(current));
      await render();
      click(tabs()[1]);
      click(q<HTMLButtonElement>('[data-testid="lock-open"]')!);
      expect(document.querySelector('[data-testid="lock-sheet"]'), status).not.toBeNull();
      current = viewWith({}, { status }, { groups });
      await tick();
      expect(document.querySelector('[data-testid="lock-sheet"]'), status).toBeNull();
      expect(posts(calls), status).toEqual([]);
      m.unmount();
    }
    m = mount(<div />);
  });
});

describe("local links", () => {
  it("saves the organizer link after the first read and keeps an existing direct link", async () => {
    localStorage.setItem(LINKS_KEY, JSON.stringify({ [EVENT_ID]: { join: JOIN, participants: {} } }));
    serve(() => json(organizerEvent));
    await render();
    expect(JSON.parse(localStorage.getItem(LINKS_KEY)!)).toEqual({
      [EVENT_ID]: { organizer: ORGANIZER, join: JOIN, participants: {} },
    });
  });

  it("keeps the page working when the browser cannot keep the link, and says to copy it now", async () => {
    spyStorageWrites(new DOMException("blocked", "SecurityError"));
    serve(() => json(organizerEvent));
    await render();
    expect(rowIds()).toHaveLength(5);
    // Visible quel que soit l'onglet.
    const warning = "Ce navigateur ne gardera pas ce lien : copie-le maintenant, depuis l'onglet Réglages.";
    expect(q('[role="alert"]')!.textContent).toBe(warning);
    click(tabs()[2]);
    expect(q('[role="alert"]')!.textContent).toBe(warning);
    expect(q('[data-testid="my-link"]')).not.toBeNull();
  });

  it("offers the organizer link to copy at the top of Réglages, never in the title", async () => {
    const writeText = vi.fn(async () => {});
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true, writable: true });
    serve(() => json(organizerEvent));
    await render();
    expect(q('[data-testid="my-link"]')).toBeNull();
    expect(m.container.textContent).not.toContain(ORGANIZER);
    click(tabs()[2]);
    const block = q<HTMLElement>('[data-testid="my-link"]')!;
    expect(block.compareDocumentPosition(q('[data-testid="settings"]')!) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(block.querySelector("h2")!.textContent).toBe("Ton lien d'organisateur");
    expect(block.textContent).toContain("Garde ce lien : c'est le seul moyen de gérer ton raid.");
    const chip = block.querySelector("button")!;
    expect(chip.textContent).toBe(`${window.location.origin}/o/${ORGANIZER}`);
    expect(chip.getAttribute("aria-label")).toBe("Copier : Ton lien d'organisateur");
    await press(chip);
    expect(writeText).toHaveBeenCalledWith(`${window.location.origin}/o/${ORGANIZER}`);
    expect(byText(m.container, "Copié")).toBeTruthy();
    expect(m.container.textContent).not.toContain("Ce navigateur ne gardera pas");
    expect(document.title).not.toContain(ORGANIZER);
  });

  it("shows the public link, and the direct link kept on this device", async () => {
    localStorage.setItem(LINKS_KEY, JSON.stringify({ [EVENT_ID]: { organizer: ORGANIZER, join: JOIN, participants: {} } }));
    serve(() => json(organizerEvent));
    await render();
    click(tabs()[2]);
    expect(q('[data-testid="public-link"] button')!.textContent).toBe(`${window.location.origin}/e/${EVENT_ID}`);
    expect(q('[data-testid="join-link"] button')!.textContent).toBe(`${window.location.origin}/j/${JOIN}`);
  });

  it("says where the direct link lives when this device does not have it", async () => {
    localStorage.removeItem(LINKS_KEY);
    serve(() => json(organizerEvent));
    await render();
    click(tabs()[2]);
    expect(q('[data-testid="public-link"] button')!.textContent).toBe(`${window.location.origin}/e/${EVENT_ID}`);
    expect(q('[data-testid="join-link"] button')).toBeNull();
    expect(q('[data-testid="join-link"]')!.textContent).toContain(
      "Le lien direct n'est gardé que dans le navigateur qui a créé le raid.",
    );
  });

  it("offers the link on a cancelled raid too", async () => {
    serve(() => json(viewWith({}, { status: "cancelled" })));
    await render();
    click(tabs()[2]);
    expect(q('[data-testid="my-link"] button')!.textContent).toBe(`${window.location.origin}/o/${ORGANIZER}`);
  });
});

describe("dead link", () => {
  const OTHER = "X".repeat(21) + "9";
  const kept = {
    [EVENT_ID]: { organizer: ORGANIZER, join: JOIN, participants: { [NYRA]: "Q".repeat(43) } },
    [OTHER]: { organizer: "W".repeat(43), participants: {} },
  };
  const stored = () => JSON.parse(localStorage.getItem(LINKS_KEY)!) as unknown;

  it("forgets the secret on this device when the link leads nowhere, at once or on a refresh", async () => {
    localStorage.setItem(LINKS_KEY, JSON.stringify(kept));
    serve(() => problem(404, "auth.not_found"));
    await render();
    expect(byText(m.container, "Retour à l'accueil")).toBeTruthy();
    // Le lien direct part avec lui : seul, il n'aurait aucune ligne sur l'accueil.
    expect(stored()).toEqual({ ...kept, [EVENT_ID]: { participants: { [NYRA]: "Q".repeat(43) } } });
    m.unmount();

    // Raid purgé pendant que la page est ouverte : le lien gardé au premier chargement s'oublie.
    localStorage.setItem(LINKS_KEY, JSON.stringify({ [OTHER]: kept[OTHER] }));
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval"] });
    let gone = false;
    serve(() => (gone ? problem(404, "auth.not_found") : json(organizerEvent)));
    await render();
    expect(stored()).toEqual({ [OTHER]: kept[OTHER], [EVENT_ID]: { organizer: ORGANIZER, participants: {} } });
    gone = true;
    await tick();
    expect(byText(m.container, "Retour à l'accueil")).toBeTruthy();
    expect(stored()).toEqual({ [OTHER]: kept[OTHER] });
  });

  it("forgets the secret when the link dies under a command", async () => {
    serve(
      () => json(organizerEvent),
      () => problem(404, "auth.not_found"),
    );
    await render();
    expect(stored()).toEqual({ [EVENT_ID]: { organizer: ORGANIZER, participants: {} } });
    click(row(NYRA));
    await press(inDialog("Accepter"));
    expect(byText(m.container, "Retour à l'accueil")).toBeTruthy();
    expect(stored()).toEqual({});
  });

  it("forgets nothing on a wrong right, a missing secret, a network failure, a quota or an outage", async () => {
    const cases: [string, () => Response][] = [
      ["wrong right", () => problem(403, "auth.wrong_right")],
      ["missing", () => problem(401, "auth.missing")],
      ["quota", () => problem(429, "limit.rate_limited")],
      ["outage", () => problem(500, "server.internal")],
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

  it("forgets nothing when a command meets a wrong right or a network failure", async () => {
    for (const reply of [() => problem(403, "auth.wrong_right"), () => Promise.reject(new TypeError("x"))]) {
      localStorage.clear();
      serve(() => json(organizerEvent), reply as () => Response);
      await render();
      click(row(NYRA));
      await press(inDialog("Accepter"));
      expect(stored()).toEqual({ [EVENT_ID]: { organizer: ORGANIZER, participants: {} } });
      m.unmount();
    }
    m = mount(<div />);
  });
});
