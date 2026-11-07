// @vitest-environment happy-dom
// Fuseau fixé avant tout calcul de date : UTC+9, sans heure d'été.
process.env.TZ = "Asia/Tokyo";

import { act, useCallback, useState } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { OrganizerCandidature, OrganizerEvent, PublicSlot } from "../api/types";
import { resetLangForTests, setLang } from "../i18n";
import { MATRIX_MEDIA } from "../lib/viewport";
import { click, mount, type Mounted } from "../test/dom";
import { KAEL, LIO, NYRA, NYRA_TWIN, TOMA, organizerEvent, reference } from "../test/fixtures";
import {
  AvailabilityGrid,
  INITIAL_GRID_CHOICE,
  type GridChoice,
  coverageLines,
  defaultSlot,
  keepOrder,
  sortRows,
} from "./AvailabilityGrid";

let m: Mounted;
const NOW = Date.parse("2026-11-07T00:00:00Z");
/** Horloge de la grille, avançable par test. */
let clock = NOW;

/** `matchMedia` simulé : `MATRIX_MEDIA` vaut `wide`, modifiable ensuite avec ses écouteurs. */
function screen(wide: boolean) {
  let matches = wide;
  const listeners = new Set<() => void>();
  vi.stubGlobal(
    "matchMedia",
    (query: string) =>
      ({
        get matches() {
          return query === MATRIX_MEDIA && matches;
        },
        media: query,
        addEventListener: (_: string, l: () => void) => listeners.add(l),
        removeEventListener: (_: string, l: () => void) => listeners.delete(l),
      }) as unknown as MediaQueryList,
  );
  return {
    set(next: boolean) {
      matches = next;
      act(() => {
        for (const l of listeners) l();
      });
    },
  };
}

let update: (v: OrganizerEvent, fresh?: ReadonlySet<string>) => void;
const opened: [string, HTMLButtonElement][] = [];

/** Les choix vivent hors de la grille, comme dans `OrganizerPage`. */
function Harness({ initial }: { initial: OrganizerEvent }) {
  const [state, setState] = useState<{ view: OrganizerEvent; fresh: ReadonlySet<string> }>({
    view: initial,
    fresh: new Set(),
  });
  const [choice, setChoice] = useState<GridChoice>(INITIAL_GRID_CHOICE);
  const onChoice = useCallback((patch: Partial<GridChoice>) => setChoice((c) => ({ ...c, ...patch })), []);
  update = (view, fresh = new Set()) => act(() => setState({ view, fresh }));
  return (
    <AvailabilityGrid
      view={state.view}
      reference={reference}
      now={() => clock}
      fresh={state.fresh}
      onOpen={(id, trigger) => opened.push([id, trigger])}
      choice={choice}
      onChoice={onChoice}
    />
  );
}

function render(view: OrganizerEvent = organizerEvent) {
  m = mount(<Harness initial={view} />);
}

const q = <T extends Element>(sel: string) => m.container.querySelector<T>(sel);
const qa = <T extends Element>(sel: string) => [...m.container.querySelectorAll<T>(sel)];
const tid = <T extends Element = HTMLElement>(id: string) => q<T>(`[data-testid="${id}"]`);
const matrixRows = () => qa<HTMLElement>('[data-testid^="grid-row-"]').map((r) => r.dataset.testid!.slice(9));
const availableRows = () => qa<HTMLElement>('[data-testid="available"] [data-testid^="slot-row-"]').map((r) => r.dataset.testid!.slice(9));
const pressed = (el: Element | null) => el?.getAttribute("aria-pressed");

/** Créneaux à venir supplémentaires, pour dépasser la limite de la matrice. */
function withSlots(n: number): OrganizerEvent {
  const slots: PublicSlot[] = Array.from({ length: n }, (_, i) => ({
    id: `s${i}`,
    startsAt: `2030-02-${String(i + 1).padStart(2, "0")}T12:00:00Z`,
    durationMin: 60,
  }));
  return {
    ...organizerEvent,
    event: { ...organizerEvent.event, slots },
    slots: slots.map((s) => ({ slotId: s.id, availableAccepted: 0, coverage: { c1: 0, c2: 0 } })),
  };
}

beforeEach(() => {
  localStorage.clear();
  vi.spyOn(navigator, "language", "get").mockReturnValue("fr-FR");
  resetLangForTests();
  opened.length = 0;
  clock = NOW;
});

afterEach(() => {
  m.unmount();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  localStorage.clear();
});

describe("view choice", () => {
  it("shows the matrix by default with 3 slots on a wide enough screen, and the time zone once", () => {
    screen(true);
    render();
    expect(tid("matrix")).toBeTruthy();
    expect(tid("by-slot")).toBeNull();
    expect(pressed(tid("view-matrix"))).toBe("true");
    expect(pressed(tid("view-slot"))).toBe("false");
    expect(tid("view-matrix")!.hasAttribute("aria-disabled")).toBe(false);
    expect(tid("view-reason")).toBeNull();
    const zone = qa("p").filter((p) => p.textContent!.startsWith("Heures en "));
    expect(zone).toHaveLength(1);
    expect(zone[0].textContent).toContain("Asia/Tokyo");

    click(tid("view-slot")!);
    expect(tid("matrix")).toBeNull();
    expect(tid("by-slot")).toBeTruthy();
    click(tid("view-matrix")!);
    expect(tid("matrix")).toBeTruthy();
  });

  it("falls back to the per-slot view beyond 4 slots, the matrix button disabled with its reason", () => {
    screen(true);
    render(withSlots(5));
    expect(tid("matrix")).toBeNull();
    expect(tid("by-slot")).toBeTruthy();
    const matrix = tid("view-matrix")!;
    expect(matrix.getAttribute("aria-disabled")).toBe("true");
    expect(pressed(matrix)).toBe("false");
    expect(pressed(tid("view-slot"))).toBe("true");
    expect(tid("view-reason")!.textContent).toBe("4 créneaux au plus");
    expect(matrix.getAttribute("aria-describedby")).toBe(tid("view-reason")!.id);
    click(matrix);
    expect(tid("matrix")).toBeNull();
    // Trop de créneaux et écran étroit : les deux raisons.
    m.unmount();
    screen(false);
    render(withSlots(5));
    expect(tid("view-reason")!.textContent).toBe("4 créneaux au plus · Écran trop étroit");
    // Exactement 4 créneaux : la matrice tient.
    screen(true);
    m.unmount();
    render(withSlots(4));
    expect(tid("matrix")).toBeTruthy();
  });

  it("falls back on a narrow screen, and brings the matrix back once it fits again", () => {
    const media = screen(false);
    render();
    expect(tid("matrix")).toBeNull();
    expect(tid("view-matrix")!.getAttribute("aria-disabled")).toBe("true");
    expect(tid("view-reason")!.textContent).toBe("Écran trop étroit");
    // « Par créneau » touché pendant que la matrice est impossible : le choix « Matrice » reste.
    click(tid("view-slot")!);
    media.set(true);
    expect(tid("matrix")).toBeTruthy();
    expect(tid("view-reason")).toBeNull();

    // Un choix « Par créneau » fait pendant que la matrice tient est gardé, loupe ou pas.
    click(tid("view-slot")!);
    media.set(false);
    media.set(true);
    expect(tid("matrix")).toBeNull();
    expect(pressed(tid("view-slot"))).toBe("true");
  });

  it("reads the width in em, so a 200 % zoom or a larger font switches to the per-slot view", () => {
    expect(MATRIX_MEDIA).toBe("(min-width: 22.5em)");
  });

  it("falls back to the per-slot view without matchMedia", () => {
    vi.stubGlobal("matchMedia", undefined);
    render();
    expect(tid("by-slot")).toBeTruthy();
    expect(tid("view-reason")!.textContent).toBe("Écran trop étroit");
  });
});

describe("matrix", () => {
  it("is a real table: caption, column and row headers, cells announced, nothing clickable", () => {
    screen(true);
    render();
    const table = q<HTMLTableElement>("table")!;
    expect(table.caption!.textContent).toBe("Disponibilités par joueur et par créneau");
    expect(table.tHead).toBeTruthy();
    expect(table.tBodies).toHaveLength(1);
    expect(table.tFoot).toBeTruthy();
    const cols = [...table.tHead!.querySelectorAll("th")];
    expect(cols.map((c) => c.getAttribute("scope"))).toEqual(["col", "col", "col", "col"]);
    expect(cols[0].textContent).toBe("Joueur");
    expect(cols[0].className).toContain("w-[136px]");
    // Jour court et heure sur deux lignes, dans un bouton de sélection.
    const head = tid("col-slot_b")!;
    expect([...head.querySelectorAll("span")].map((s) => s.textContent)).toEqual(["ven.", "05:00"]);
    expect(cols[3].className).toContain("w-tap");

    // Lignes : `accepted` d'abord, puis le nombre de créneaux cochés, puis l'ordre de création.
    expect(matrixRows()).toEqual([KAEL, NYRA_TWIN, NYRA]);
    for (const id of [TOMA, LIO]) expect(tid(`grid-row-${id}`)).toBeNull();
    const rowHeads = [...table.tBodies[0].querySelectorAll("th")];
    expect(rowHeads.every((h) => h.getAttribute("scope") === "row")).toBe(true);
    const cp = (n: number) => new Intl.NumberFormat("fr").format(n);
    expect(rowHeads[0].textContent).toBe(`Kael[SE] · Classe Dégâts · ${cp(2300)}`);
    expect(rowHeads[2].textContent).toBe(`Nyraattentedoublon[SA] · Classe Soin · ${cp(1500)}`);
    const badge = rowHeads[2].querySelector('[data-testid="pending-badge"]')!;
    // Le badge court reprend le style du statut `pending`.
    expect(badge.className).toContain("border-pending bg-pending/10 text-pending");
    expect(rowHeads[0].querySelector('[data-testid="pending-badge"]')).toBeNull();
    expect(rowHeads[0].querySelector("button .truncate")).toBeTruthy();

    // Cellules : ✓ / –, étoile si préféré, texte lu par les lecteurs d'écran, aucun bouton.
    const yes = tid("cell-" + KAEL + "-slot_b")!;
    expect(yes.dataset.available).toBe("true");
    expect(yes.className).toContain("bg-available");
    expect(yes.className).toContain("size-tap");
    expect(yes.querySelector(".sr-only")!.textContent).toBe("Kael, ven. 05:00, disponible, préféré");
    expect(yes.textContent).toContain("✓");
    expect(yes.textContent).toContain("☆");
    const no = tid("cell-" + NYRA_TWIN + "-slot_a")!;
    expect(no.dataset.available).toBe("false");
    expect(no.className).toContain("bg-surface-sunken");
    expect(no.textContent).toContain("–");
    // Ligne en doublon : annoncée par `Pseudo[SRV]`.
    expect(no.querySelector(".sr-only")!.textContent).toBe("nyra[SA], mer. 05:00, non disponible");
    // Créneau passé : dit, pas seulement grisé.
    expect(tid(`cell-${NYRA}-slot_past`)!.querySelector(".sr-only")!.textContent).toBe(
      "Nyra[SA], mer. 05:00, disponible, passé",
    );
    expect(tid("cell-" + KAEL + "-slot_a")!.querySelector(".sr-only")!.textContent).toBe("Kael, mer. 05:00, disponible");
    expect(table.tBodies[0].querySelectorAll("td button, td [tabindex]")).toHaveLength(0);

    // Pied : disponibles parmi les `accepted`, celui du créneau sélectionné en accent.
    expect(table.tFoot!.querySelector("th")!.getAttribute("scope")).toBe("row");
    expect(["slot_past", "slot_a", "slot_b"].map((s) => tid(`foot-${s}`)!.textContent)).toEqual(["0", "1", "2"]);
    expect(tid("foot-slot_a")!.className).toContain("text-accent");
    expect(tid("foot-slot_b")!.className).not.toContain("text-accent");
  });

  it("selects a slot from its column header and shows its coverage", () => {
    screen(true);
    render();
    // Par défaut, le premier créneau à venir.
    expect(pressed(tid("col-slot_a"))).toBe("true");
    expect(tid("coverage")!.querySelector("h3")!.textContent).toBe("mer. 05:00 · 1 dispo parmi les acceptés");
    click(tid("col-slot_b")!);
    expect(pressed(tid("col-slot_b"))).toBe("true");
    expect(pressed(tid("col-slot_a"))).toBe("false");
    expect(tid("foot-slot_b")!.className).toContain("text-accent");
    expect(tid("cell-" + KAEL + "-slot_b")!.className).toContain("outline-accent");
    expect(tid("cell-" + KAEL + "-slot_a")!.className).not.toContain("outline-accent");
    expect(tid("coverage")!.querySelector("h3")!.textContent).toBe("ven. 05:00 · 2 dispo parmi les acceptés");
    // La sélection est partagée avec la vue par créneau.
    click(tid("view-slot")!);
    expect(pressed(tid("chip-slot_b"))).toBe("true");
  });

  it("opens the candidature sheet from a row header", () => {
    screen(true);
    render();
    const button = tid(`grid-row-${NYRA}`)!.querySelector<HTMLButtonElement>("th button")!;
    expect(button.getAttribute("aria-haspopup")).toBe("dialog");
    click(button);
    expect(opened).toEqual([[NYRA, button]]);
  });

  it("greys a past slot, and selects the last slot once every slot is past", () => {
    screen(true);
    render();
    expect(tid("col-slot_past")!.className).toContain("text-ink-disabled");
    expect(tid("col-slot_past")!.querySelector(".sr-only")!.textContent).toBe("passé");
    expect(tid("col-slot_a")!.querySelector(".sr-only")).toBeNull();
    expect(tid("col-slot_a")!.className).not.toContain("text-ink-disabled");
    expect(defaultSlot(organizerEvent.event.slots, Date.parse("2031-01-01T00:00:00Z"))).toBe("slot_b");
    expect(defaultSlot(organizerEvent.event.slots, NOW)).toBe("slot_a");
    expect(defaultSlot([], NOW)).toBeNull();
    click(tid("view-slot")!);
    expect(tid("chip-slot_past")!.className).toContain("text-ink-disabled");
    expect(tid("chip-slot_a")!.className).not.toContain("text-ink-disabled");
  });
});

describe("rows", () => {
  it("filters pending rows without changing the footer, the chips or the coverage", () => {
    screen(true);
    render();
    const filters = ["all", "accepted", "pending"].map((f) => tid(`filter-${f}`)!);
    expect(filters.map((f) => f.textContent)).toEqual(["Tous3", "Acceptés2", "En attente1"]);
    expect(filters.map(pressed)).toEqual(["true", "false", "false"]);
    click(filters[2]);
    expect(filters.map(pressed)).toEqual(["false", "false", "true"]);
    expect(matrixRows()).toEqual([NYRA]);
    expect(["slot_past", "slot_a", "slot_b"].map((s) => tid(`foot-${s}`)!.textContent)).toEqual(["0", "1", "2"]);
    expect(tid("coverage")!.querySelector("h3")!.textContent).toBe("mer. 05:00 · 1 dispo parmi les acceptés");
    click(tid("view-slot")!);
    expect(tid("chip-slot_b")!.textContent).toBe("ven. 05:00 · 2");
    click(filters[1]);
    click(tid("view-matrix")!);
    expect(matrixRows()).toEqual([KAEL, NYRA_TWIN]);
  });

  it("never moves a row on refresh: in-place updates, new rows last with the gold edge, leavers removed", () => {
    screen(true);
    render();
    expect(matrixRows()).toEqual([KAEL, NYRA_TWIN, NYRA]);
    const NEW = "Z".repeat(21) + "9";
    const newcomer: OrganizerCandidature = {
      ...organizerEvent.candidatures[1],
      id: NEW,
      pseudo: "Zed",
      pseudoSrv: "Zed[SE]",
      status: "accepted",
    };
    // Nyra acceptée (elle serait triée en tête), nyra refusée (sort du filtre), Toma reprise.
    const next: OrganizerEvent = {
      ...organizerEvent,
      candidatures: [
        ...organizerEvent.candidatures.map((c) =>
          c.id === NYRA
            ? { ...c, status: "accepted" as const, availabilities: [...c.availabilities, { slotId: "slot_b", preferred: false }] }
            : c.id === NYRA_TWIN
              ? { ...c, status: "rejected" as const }
              : c.id === TOMA
                ? { ...c, status: "accepted" as const }
                : c,
        ),
        newcomer,
      ],
    };
    update(next, new Set([NEW]));
    expect(matrixRows()).toEqual([KAEL, NYRA, TOMA, NEW]);
    // Mise à jour en place : badge parti, cellule cochée.
    expect(tid(`grid-row-${NYRA}`)!.querySelector('[data-testid="pending-badge"]')).toBeNull();
    expect(tid(`cell-${NYRA}-slot_b`)!.dataset.available).toBe("true");
    expect(tid(`grid-row-${NEW}`)!.dataset.fresh).toBe("true");
    expect(tid(`grid-row-${NEW}`)!.querySelector("th button")!.className).toContain("border-l-accent");
    expect(tid(`grid-row-${KAEL}`)!.dataset.fresh).toBeUndefined();
    // Une nouvelle relecture identique ne bouge rien.
    update(next);
    expect(matrixRows()).toEqual([KAEL, NYRA, TOMA, NEW]);
    // Le filtre retrie.
    click(tid("filter-accepted")!);
    click(tid("filter-all")!);
    expect(matrixRows()).toEqual([NYRA, KAEL, NEW, TOMA]);
  });

  it("sorts and keeps order with pure helpers", () => {
    const rows = organizerEvent.candidatures.filter((c) => c.status === "pending" || c.status === "accepted");
    expect(sortRows(rows)).toEqual([KAEL, NYRA_TWIN, NYRA]);
    expect(keepOrder([NYRA, KAEL], rows)).toEqual([NYRA, KAEL, NYRA_TWIN]);
    expect(keepOrder([TOMA, NYRA], rows)).toEqual([NYRA, KAEL, NYRA_TWIN]);
  });

  it("tells duplicates apart: [SRV] on the row, a compact « doublon » mark, Pseudo[SRV] read aloud", () => {
    screen(true);
    render();
    for (const id of [NYRA, NYRA_TWIN]) {
      const head = tid(`grid-row-${id}`)!.querySelector("th")!;
      expect(head.querySelector('[data-testid="duplicate-badge"]')!.textContent).toBe("doublon");
      expect(head.querySelector('[data-testid="duplicate-badge"]')!.className).toContain("text-danger");
      expect(head.textContent).toContain("[SA] · ");
    }
    expect(tid(`grid-row-${KAEL}`)!.querySelector('[data-testid="duplicate-badge"]')).toBeNull();
    expect(tid(`cell-${NYRA_TWIN}-slot_b`)!.querySelector(".sr-only")!.textContent).toBe("nyra[SA], ven. 05:00, disponible");
    expect(tid(`cell-${KAEL}-slot_b`)!.querySelector(".sr-only")!.textContent).toBe("Kael, ven. 05:00, disponible, préféré");
    click(tid("view-slot")!);
    click(tid("chip-slot_b")!);
    expect(tid(`slot-row-${NYRA_TWIN}`)!.querySelector('[data-testid="duplicate-badge"]')).toBeTruthy();
    expect(tid(`slot-row-${KAEL}`)!.querySelector('[data-testid="duplicate-badge"]')).toBeNull();
  });

  it("says when no row matches", () => {
    screen(true);
    render({ ...organizerEvent, candidatures: organizerEvent.candidatures.filter((c) => c.status !== "pending") });
    click(tid("filter-pending")!);
    expect(matrixRows()).toEqual([]);
    expect(m.container.textContent).toContain("Personne ici pour l'instant.");
  });
});

describe("selection and refresh", () => {
  it("freezes the default slot: it does not jump when the first upcoming slot starts", () => {
    screen(true);
    render();
    expect(pressed(tid("col-slot_a"))).toBe("true");
    // Le créneau A commence ; une relecture arrive.
    clock = Date.parse("2030-01-08T21:00:00Z");
    update({ ...organizerEvent });
    expect(pressed(tid("col-slot_a"))).toBe("true");
    expect(tid("coverage")!.querySelector("h3")!.textContent).toBe("mer. 05:00 · 1 dispo parmi les acceptés");
  });

  it("falls back to the default when a refresh drops the picked slot", () => {
    screen(true);
    render();
    click(tid("col-slot_b")!);
    expect(pressed(tid("col-slot_b"))).toBe("true");
    update({
      ...organizerEvent,
      event: { ...organizerEvent.event, slots: organizerEvent.event.slots.filter((s) => s.id !== "slot_b") },
      slots: organizerEvent.slots.filter((s) => s.slotId !== "slot_b"),
    });
    expect(tid("col-slot_b")).toBeNull();
    expect(pressed(tid("col-slot_a"))).toBe("true");
    expect(tid("coverage")!.querySelector("h3")!.textContent).toBe("mer. 05:00 · 1 dispo parmi les acceptés");
  });

  it("updates the footer, the chips and the coverage from a refreshed view", () => {
    screen(true);
    render();
    expect(tid("foot-slot_a")!.textContent).toBe("1");
    update({
      ...organizerEvent,
      slots: organizerEvent.slots.map((s) =>
        s.slotId === "slot_a" ? { ...s, availableAccepted: 3, coverage: { c1: 2, c2: 1 } } : s,
      ),
    });
    expect(["slot_past", "slot_a", "slot_b"].map((s) => tid(`foot-${s}`)!.textContent)).toEqual(["0", "3", "2"]);
    expect(tid("coverage")!.querySelector("h3")!.textContent).toBe("mer. 05:00 · 3 dispo parmi les acceptés");
    expect(tid("coverage-c1")!.textContent).toBe("Classe Soin2 / min 1");
    expect(tid("coverage-c1")!.dataset.under).toBeUndefined();
    expect(tid("coverage-c2")!.textContent).toBe("Classe Dégâts1 / min 1");
    click(tid("view-slot")!);
    expect(tid("chip-slot_a")!.textContent).toBe("mer. 05:00 · 3");
  });

  it("gives the panel a heading, hidden but focusable", () => {
    screen(true);
    render();
    const h2 = q<HTMLHeadingElement>("h2")!;
    expect(h2.textContent).toBe("Disponibilités");
    expect(h2.className).toContain("sr-only");
    expect(h2.tabIndex).toBe(-1);
  });
});

describe("per-slot view", () => {
  it("lists chips, then the coverage, then the available rows, then a closed « Pas dispo » fold", () => {
    screen(false);
    render();
    const chips = qa<HTMLButtonElement>('[data-testid^="chip-"]');
    expect(chips.map((c) => c.textContent)).toEqual(["mer. 05:00 · 0, passé", "mer. 05:00 · 1", "ven. 05:00 · 2"]);
    expect(chips[0].querySelector(".sr-only")!.textContent).toBe(", passé");
    expect(chips.map(pressed)).toEqual(["false", "true", "false"]);
    expect(chips[0].parentElement!.className).toContain("flex-wrap");
    const order = qa<HTMLElement>('[data-testid="by-slot"] [data-testid]').map((e) => e.dataset.testid);
    expect(order.indexOf("coverage")).toBeLessThan(order.indexOf("available"));
    expect(availableRows()).toEqual([KAEL, NYRA]);
    // Préférence et attente signalées.
    expect(tid(`slot-row-${NYRA}`)!.textContent).toContain("☆préféré");
    expect(tid(`slot-row-${NYRA}`)!.querySelector('[data-testid="pending-badge"]')).toBeTruthy();
    expect(tid(`slot-row-${KAEL}`)!.textContent).toContain("[SE] · Classe Dégâts");

    const fold = tid<HTMLButtonElement>("unavailable-toggle")!;
    expect(fold.textContent).toBe("▸Pas dispo (1)");
    expect(fold.getAttribute("aria-expanded")).toBe("false");
    expect(tid("unavailable")).toBeNull();
    click(fold);
    expect(fold.getAttribute("aria-expanded")).toBe("true");
    expect(fold.getAttribute("aria-controls")).toBe(tid("unavailable")!.id);
    expect(qa<HTMLElement>('[data-testid="unavailable"] button').map((b) => b.dataset.testid)).toEqual([
      `slot-row-${NYRA_TWIN}`,
    ]);

    // Choisir un autre créneau.
    click(tid("chip-slot_b")!);
    expect(pressed(tid("chip-slot_b"))).toBe("true");
    expect(availableRows()).toEqual([KAEL, NYRA_TWIN]);
    expect(tid("coverage")!.querySelector("h3")!.textContent).toBe("ven. 05:00 · 2 dispo parmi les acceptés");

    // Une ligne ouvre la feuille.
    click(tid(`slot-row-${KAEL}`)!);
    expect(opened[0][0]).toBe(KAEL);
  });
});

describe("coverage", () => {
  it("labels each class from the reference: under the minimum in danger, above the maximum, plain", () => {
    const bounds = { c1: { min: 2, max: 3 }, c2: { min: 0, max: 1 }, c3: { min: 0, max: 5 } };
    const lines = coverageLines({ c2: 3, ghost: 1, c1: 1, c3: 0 }, bounds, reference, "fr");
    expect(lines).toEqual([
      { class: "c1", label: "Classe Soin", text: "1 / min 2", under: true },
      { class: "c2", label: "Classe Dégâts", text: "3 / max 1", under: false },
      { class: "c3", label: "c3", text: "0", under: false },
      { class: "ghost", label: "ghost", text: "1", under: false },
    ]);
    expect(coverageLines({ c1: 2 }, bounds, reference, "fr")[0].text).toBe("2 / min 2");
  });

  it("shows « Clerc 1 / min 2 » in danger, text only", () => {
    screen(true);
    render({
      ...organizerEvent,
      classBounds: { c1: { min: 2, max: 3 }, c2: { min: 0, max: 10 } },
      slots: organizerEvent.slots.map((s) => (s.slotId === "slot_a" ? { ...s, coverage: { c1: 1, c2: 0 } } : s)),
    });
    const c1 = tid("coverage-c1")!;
    expect(c1.textContent).toBe("Classe Soin1 / min 2");
    expect(c1.dataset.under).toBe("true");
    expect(c1.className).toContain("text-danger");
    expect(c1.className).not.toMatch(/bg-danger/);
    expect(tid("coverage-c2")!.textContent).toBe("Classe Dégâts0");
    expect(tid("coverage-c2")!.className).not.toContain("text-danger");
  });

  it("speaks English too", () => {
    screen(true);
    setLang("en");
    render();
    expect(tid("coverage")!.querySelector("h3")!.textContent).toBe("Wed 05:00 · 1 available among accepted");
    expect(tid("view-matrix")!.textContent).toBe("Grid");
    expect(tid("filter-pending")!.textContent).toBe("Pending1");
    expect(tid(`cell-${KAEL}-slot_a`)!.querySelector(".sr-only")!.textContent).toBe("Kael, Wed 05:00, available");
  });
});
