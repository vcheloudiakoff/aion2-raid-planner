// @vitest-environment happy-dom
// Onglet Composition (story 3-3), rendu seul avec un brouillon tenu par le test comme le fait la page :
// réserve ordonnée en trois sections, déplacement, Retirer / Au banc / Échanger, avertissements en
// ligne. Fuseau fixé avant tout calcul de date : UTC+9, sans heure d'été.
process.env.TZ = "Asia/Tokyo";

import { act, useState } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { OrganizerCandidature, OrganizerEvent } from "../api/types";
import { resetLangForTests } from "../i18n";
import type { Placement } from "../lib/placement";
import { click, key, mount, type Mounted } from "../test/dom";
import { organizerEvent, organizerGroup, reference } from "../test/fixtures";
import { Composition, missingRoles } from "./Composition";
import { reserveSections } from "./ReserveSheet";

let m: Mounted;

const cand = (id: string, patch: Partial<OrganizerCandidature>): OrganizerCandidature => ({
  id,
  pseudo: id,
  server: "s_a",
  pseudoSrv: `${id}[SA]`,
  class: "c2",
  role: "dps",
  cp: 1000,
  status: "accepted",
  origin: "direct",
  availabilities: [{ slotId: "slot_a", preferred: false }],
  duplicate: false,
  teams: [],
  ...patch,
});

/** Dans l'ordre de création. `c1` (Soin) a un minimum de 1 : il manque tant que G1 n'en a pas. */
const candidatures = [
  cand("Aran", { cp: 3000 }),
  cand("Bria", { class: "c1", role: "healer", cp: 1000 }),
  cand("Cael", { cp: 2000 }),
  cand("Dorn", { cp: 2000 }),
  cand("Eira", { class: "c1", role: "healer", cp: 5000, availabilities: [{ slotId: "slot_b", preferred: false }] }),
  cand("Fenn", { cp: 1500 }),
  cand("Pend", { status: "pending" }),
  cand("Rejt", { status: "rejected" }),
];

function locked(patch: Partial<OrganizerEvent> = {}): OrganizerEvent {
  return {
    ...organizerEvent,
    event: { ...organizerEvent.event, status: "locked" },
    classBounds: { c1: { min: 1, max: 3 }, c2: { min: 0, max: 10 } },
    candidatures,
    groups: [
      organizerGroup({ id: "g1" }),
      organizerGroup({
        id: "g2",
        slotId: "slot_b",
        starters: [{ candidatureId: "Fenn", party: 1, index: 1, unavailable: true }],
      }),
    ],
    ...patch,
  };
}

interface Seen {
  draft: Placement | null;
  /** Remplace la vue (relecture), l'état « commande en vol » ou le brouillon, comme la page. */
  setView: (v: OrganizerEvent) => void;
  setBusy: (busy: boolean) => void;
  setDraft: (d: Placement | null) => void;
}

function Harness({ initial, seen }: { initial: OrganizerEvent; seen: Seen }) {
  const [view, setView] = useState(initial);
  const [draft, setDraft] = useState<Placement | null>(null);
  const [busy, setBusy] = useState(false);
  Object.assign(seen, { draft, setView, setBusy, setDraft });
  return (
    <Composition
      view={view}
      reference={reference}
      draft={draft}
      onEdit={setDraft}
      onSave={() => {}}
      onDiscard={() => setDraft(null)}
      onSee={() => setDraft(null)}
      changed={0}
      saving={false}
      busy={busy}
      error={null}
    />
  );
}

function render(view: OrganizerEvent = locked()): Seen {
  const seen = { draft: null } as Seen;
  m = mount(<Harness initial={view} seen={seen} />);
  return seen;
}

/** Applique un changement d'état du harnais dans `act`. */
function change(f: () => void) {
  act(f);
}

const q = <T extends Element>(sel: string) => document.querySelector<T>(sel);
const slot = (party: number, index: number) => q<HTMLButtonElement>(`[data-testid="slot-${party}-${index}"]`)!;
const reserveIds = (section: string) =>
  [...document.querySelectorAll<HTMLElement>(`[data-testid="reserve-${section}"] [data-testid^="reserve-"]`)]
    .map((b) => b.dataset.testid!.slice("reserve-".length))
    .filter((id) => id !== "unavailable-toggle");
const action = (name: string) => q<HTMLButtonElement>(`[data-testid="action-${name}"]`)!;
const chip = (id: string) => q<HTMLButtonElement>(`[data-testid="group-chip-${id}"]`)!;
const text = (el: Element | null) => el?.textContent ?? null;

beforeEach(() => {
  vi.spyOn(navigator, "language", "get").mockReturnValue("fr-FR");
  resetLangForTests();
});

afterEach(() => {
  m.unmount();
  vi.restoreAllMocks();
});

describe("composition", () => {
  it("shows the group chips, the state line, both parties slot by slot and the bench", () => {
    render();
    expect([...document.querySelectorAll('[data-testid^="group-chip-"]')].map(text)).toEqual([
      "G1 · mer. 05:00",
      "G2 · ven. 05:00",
    ]);
    expect(chip("g1").getAttribute("aria-pressed")).toBe("true");
    expect(text(q('[data-testid="composition-state"]'))).toBe("0 / 10 · manque 1 Soin");
    for (const party of [1, 2]) {
      expect(text(q(`[data-testid="party-${party}"] h3`))).toBe(`Équipe ${party}`);
      expect(document.querySelectorAll(`[data-testid^="slot-${party}-"]`)).toHaveLength(5);
    }
    expect(text(slot(1, 1))).toBe("1libre");
    expect(text(q('[data-testid="bench-add"]'))).toBe("+ Ajouter au banc");
    // Hors brouillon, la barre est vide.
    expect(q('[data-testid="composition-bar"]')).toBeNull();
    click(chip("g2"));
    expect(text(slot(1, 1))).toBe("1DPS · Classe DégâtsFenn[SA]pas dispo à ce créneau");
    expect(text(q('[data-testid="composition-state"]'))).toBe("1 / 10 · manque 1 Soin");
  });

  it("counts the sleeping groups under the retained ones, and says nothing without any", () => {
    render();
    expect(q('[data-testid="composition-sleeping"]')).toBeNull();
    m.unmount();
    const view = locked();
    render({ ...view, groups: [...view.groups, organizerGroup({ id: "zz", retained: false })] });
    expect(text(q('[data-testid="composition-sleeping"]'))).toBe(
      "1 groupe en sommeil, repris s'il est reverrouillé.",
    );
    // Le groupe en sommeil n'a pas de puce.
    expect(chip("zz")).toBeNull();
  });

  it("lists the reserve in three sections: missing class, then CP, then creation order", () => {
    const seen = render();
    click(slot(1, 1));
    expect(text(q('[data-testid="reserve"] h2'))).toBe("Placer · Équipe 1, slot 1");
    // Bria (Soin manquant) d'abord, puis le CP décroissant, Cael avant Dorn à égalité.
    expect(reserveIds("available")).toEqual(["Bria", "Aran", "Cael", "Dorn"]);
    expect(reserveIds("elsewhere")).toEqual(["Fenn"]);
    expect(text(q('[data-testid="reserve-Fenn"]'))).toContain("dans G2");
    // Pas dispo : replié par défaut, la placer reste permis.
    const toggle = q<HTMLButtonElement>('[data-testid="reserve-unavailable-toggle"]')!;
    expect(text(toggle)).toContain("Pas dispo à ce créneau (1)");
    expect(toggle.getAttribute("aria-expanded")).toBe("false");
    expect(q('[data-testid="reserve-Eira"]')).toBeNull();
    click(toggle);
    expect(reserveIds("unavailable")).toEqual(["Eira"]);
    // Jamais `pending` ni `rejected`.
    expect(q('[data-testid="reserve-Pend"]')).toBeNull();
    expect(q('[data-testid="reserve-Rejt"]')).toBeNull();
    click(q('[data-testid="reserve-Bria"]')!);
    expect(q('[data-testid="reserve"]')).toBeNull();
    expect(seen.draft!.g1.starters).toEqual({ "1.1": "Bria" });
    // Soin couvert : plus de classe manquante, et Bria (déjà dans G1) n'est plus proposée.
    click(slot(1, 2));
    expect(reserveIds("available")).toEqual(["Aran", "Cael", "Dorn"]);
  });

  it("orders the reserve the same way as a pure function", () => {
    const view = locked();
    const s = reserveSections(view, { g1: { starters: {}, bench: [] }, g2: { starters: { "1.1": "Fenn" }, bench: [] } }, "g1", "slot_a");
    expect(s.available.map((c) => c.id)).toEqual(["Bria", "Aran", "Cael", "Dorn"]);
    expect(s.elsewhere).toEqual([{ candidature: candidatures[5], groupId: "g2" }]);
    expect(s.unavailable.map((c) => c.id)).toEqual(["Eira"]);
  });

  it("moves a candidature placed in another group instead of duplicating it", () => {
    const seen = render();
    click(q('[data-testid="bench-add"]')!);
    expect(text(q('[data-testid="reserve"] h2'))).toBe("Placer · Banc");
    click(q('[data-testid="reserve-Fenn"]')!);
    expect(seen.draft).toEqual({
      g1: { starters: {}, bench: ["Fenn"] },
      g2: { starters: {}, bench: [] },
    });
    expect(text(q('[data-testid="bench-1"]'))).toContain("Fenn[SA]");
    click(chip("g2"));
    expect(text(slot(1, 1))).toBe("1libre");
  });

  it("removes, benches and swaps an occupant, across groups too", () => {
    const view = locked({
      groups: [
        organizerGroup({
          id: "g1",
          starters: [
            { candidatureId: "Aran", party: 1, index: 1, unavailable: false },
            { candidatureId: "Cael", party: 1, index: 2, unavailable: false },
            { candidatureId: "Dorn", party: 2, index: 5, unavailable: false },
          ],
        }),
        organizerGroup({
          id: "g2",
          slotId: "slot_b",
          starters: [{ candidatureId: "Fenn", party: 1, index: 1, unavailable: true }],
        }),
      ],
    });
    const seen = render(view);
    // Retirer.
    click(slot(1, 1));
    expect(text(q('[data-testid="slot-actions"] h2'))).toBe("Aran[SA]");
    click(action("remove"));
    expect(seen.draft!.g1.starters).toEqual({ "1.2": "Cael", "2.5": "Dorn" });
    expect(text(slot(1, 1))).toBe("1libre");
    // Au banc.
    click(slot(1, 2));
    click(action("bench"));
    expect(seen.draft!.g1).toEqual({ starters: { "2.5": "Dorn" }, bench: ["Cael"] });
    // Au banc, « Au banc » n'est plus proposé.
    click(q('[data-testid="bench-1"]')!);
    expect(q('[data-testid="action-bench"]')).toBeNull();
    // Échanger : le banc vers un slot libre, puis deux occupants entre G1 et G2.
    click(action("swap"));
    expect(q('[data-testid="slot-actions"]')).toBeNull();
    expect(text(q('[data-testid="swap-hint"] p'))).toBe("Touche la place à échanger avec Cael[SA].");
    expect(q<HTMLElement>('[data-testid="bench-1"]')!.dataset.active).toBe("true");
    click(slot(1, 3));
    expect(seen.draft!.g1).toEqual({ starters: { "1.3": "Cael", "2.5": "Dorn" }, bench: [] });
    expect(q('[data-testid="swap-hint"]')).toBeNull();
    click(slot(2, 5));
    click(action("swap"));
    click(chip("g2"));
    click(slot(1, 1));
    expect(seen.draft).toEqual({
      g1: { starters: { "1.3": "Cael", "2.5": "Fenn" }, bench: [] },
      g2: { starters: { "1.1": "Dorn" }, bench: [] },
    });
    // Annuler un échange ne change rien.
    click(slot(1, 1));
    click(action("swap"));
    click(q('[data-testid="swap-cancel"]')!);
    expect(q('[data-testid="swap-hint"]')).toBeNull();
    expect(seen.draft!.g2).toEqual({ starters: { "1.1": "Dorn" }, bench: [] });
  });

  it("shows the warnings inline: the server's outside a draft, recomputed inside one", () => {
    const view = locked({
      classBounds: { c1: { min: 1, max: 3 }, c2: { min: 0, max: 1 } },
      groups: [
        organizerGroup({
          id: "g1",
          starters: [
            { candidatureId: "Aran", party: 1, index: 1, unavailable: false },
            { candidatureId: "Cael", party: 1, index: 2, unavailable: true },
          ],
          classWarnings: { c1: { count: 0, min: 1, max: 3 }, c2: { count: 2, min: 0, max: 1 } },
        }),
      ],
    });
    render(view);
    // Le serveur dit Cael indisponible : la ligne le montre, même si elle a coché le créneau.
    expect(text(slot(1, 2).querySelector('[data-testid="slot-unavailable"]'))).toBe("pas dispo à ce créneau");
    expect(slot(1, 1).querySelector('[data-testid="slot-unavailable"]')).toBeNull();
    expect(text(slot(1, 1).querySelector('[data-testid="slot-class"]'))).toBe("DPS · Classe Dégâts · 2 / max 1");
    expect([...document.querySelectorAll('[data-testid="composition-classes"] li')].map(text)).toEqual([
      "Classe Soin 0 / min 1",
      "Classe Dégâts 2 / max 1",
    ]);
    // En brouillon : Eira (Soin, pas dispo à ce créneau) placée sans refus.
    click(slot(2, 1));
    click(q<HTMLButtonElement>('[data-testid="reserve-unavailable-toggle"]')!);
    click(q('[data-testid="reserve-Eira"]')!);
    expect(text(slot(2, 1).querySelector('[data-testid="slot-unavailable"]'))).toBe("pas dispo à ce créneau");
    // Recalculé sur le brouillon : Cael a coché le créneau.
    expect(slot(1, 2).querySelector('[data-testid="slot-unavailable"]')).toBeNull();
    expect([...document.querySelectorAll('[data-testid="composition-classes"] li')].map(text)).toEqual([
      "Classe Dégâts 2 / max 1",
    ]);
    expect(text(q('[data-testid="composition-state"]'))).toBe("3 / 10");
  });

  it("says when no group is retained", () => {
    render(locked({ groups: [organizerGroup({ id: "zz", retained: false })] }));
    expect(text(q('[data-testid="composition"] p'))).toBe("Aucun groupe retenu.");
  });

  it("flags an unavailable bench player from the server data and in a draft, and never counts the bench for a bound", () => {
    const view = locked({
      classBounds: { c1: { min: 0, max: 1 }, c2: { min: 0, max: 10 } },
      groups: [
        organizerGroup({
          id: "g1",
          starters: [{ candidatureId: "Bria", party: 1, index: 1, unavailable: false }],
          bench: [{ candidatureId: "Cael", benchIndex: 1, unavailable: true }],
        }),
      ],
    });
    const seen = render(view);
    expect(text(q('[data-testid="bench-1"] [data-testid="slot-unavailable"]'))).toBe("pas dispo à ce créneau");
    // Eira (Soin, pas dispo) au banc, en brouillon : avertie, et la ligne de classe ne bouge pas.
    click(q('[data-testid="bench-add"]')!);
    click(q('[data-testid="reserve-unavailable-toggle"]')!);
    click(q('[data-testid="reserve-Eira"]')!);
    expect(seen.draft!.g1.bench).toEqual(["Cael", "Eira"]);
    expect(text(q('[data-testid="bench-2"] [data-testid="slot-unavailable"]'))).toBe("pas dispo à ce créneau");
    // Recalculé : Cael a coché le créneau.
    expect(q('[data-testid="bench-1"] [data-testid="slot-unavailable"]')).toBeNull();
    expect(q('[data-testid="composition-classes"]')).toBeNull();
    expect(text(slot(1, 1).querySelector('[data-testid="slot-class"]'))).toBe("Soin · Classe Soin");
  });

  it("finds no missing role without the reference rather than a false one", () => {
    const view = locked();
    expect(missingRoles(view, reference, [])).toEqual([{ role: "healer", missing: 1 }]);
    expect(missingRoles(view, null, [])).toEqual([]);
  });

  it("flags a drafted candidature that is no longer accepted or gone", () => {
    const seen = render();
    click(slot(1, 1));
    click(q('[data-testid="reserve-Aran"]')!);
    click(slot(1, 2));
    click(q('[data-testid="reserve-Cael"]')!);
    expect(slot(1, 1).querySelector('[data-testid="slot-gone"]')).toBeNull();
    change(() =>
      seen.setView(
        locked({
          candidatures: candidatures
            .filter((c) => c.id !== "Cael")
            .map((c) => (c.id === "Aran" ? { ...c, status: "rejected" as const } : c)),
        }),
      ),
    );
    expect(text(slot(1, 1).querySelector('[data-testid="slot-gone"]'))).toBe("plus acceptée");
    expect(text(slot(1, 2).querySelector('[data-testid="slot-gone"]'))).toBe("absente");
    expect(slot(1, 2).getAttribute("aria-label")).toBe("Équipe 1, slot 2, Cael, absente, pas dispo à ce créneau");
  });

  it("notes a reserve row placed in a sleeping group", () => {
    render(
      locked({
        groups: [
          organizerGroup({ id: "g1" }),
          organizerGroup({ id: "zz", retained: false, bench: [{ candidatureId: "Aran", benchIndex: 1, unavailable: false }] }),
        ],
      }),
    );
    click(slot(1, 1));
    expect(text(q('[data-testid="reserve-Aran"]'))).toContain("dans un groupe en sommeil");
    expect(text(q('[data-testid="reserve-Cael"]'))).not.toContain("sommeil");
  });

  it("names each spot, marks the swap source pressed, and cancels a swap on Escape", () => {
    render(
      locked({
        groups: [organizerGroup({ id: "g1", starters: [{ candidatureId: "Aran", party: 1, index: 1, unavailable: false }] })],
      }),
    );
    expect(slot(1, 1).getAttribute("aria-label")).toBe("Équipe 1, slot 1, Aran[SA]");
    expect(slot(1, 3).getAttribute("aria-label")).toBe("Équipe 1, slot 3, libre");
    expect(slot(1, 3).getAttribute("aria-haspopup")).toBe("dialog");
    expect(slot(1, 1).hasAttribute("aria-pressed")).toBe(false);
    click(slot(1, 1));
    click(action("swap"));
    expect(slot(1, 1).getAttribute("aria-pressed")).toBe("true");
    expect(slot(1, 3).hasAttribute("aria-haspopup")).toBe(false);
    expect(q('[data-testid="bench-add"]')!.hasAttribute("aria-haspopup")).toBe(false);
    key(document.body, "Escape");
    expect(q('[data-testid="swap-hint"]')).toBeNull();
    expect(slot(1, 1).hasAttribute("aria-pressed")).toBe(false);
    expect(slot(1, 3).getAttribute("aria-haspopup")).toBe("dialog");
  });

  it("drops a pending swap when the draft goes or its source changes, and locks the controls while busy", () => {
    const seen = render();
    click(slot(1, 1));
    click(q('[data-testid="reserve-Aran"]')!);
    click(slot(1, 1));
    click(action("swap"));
    // Brouillon abandonné (enregistré ailleurs) : l'échange tombe, le toucher suivant n'échange rien.
    change(() => seen.setDraft(null));
    expect(q('[data-testid="swap-hint"]')).toBeNull();
    // Hors brouillon, la relecture change l'occupant de la source : l'échange tombe aussi.
    const withAran = (id: string) =>
      locked({ groups: [organizerGroup({ id: "g1", starters: [{ candidatureId: id, party: 1, index: 1, unavailable: false }] })] });
    change(() => seen.setView(withAran("Aran")));
    click(slot(1, 1));
    click(action("swap"));
    expect(q('[data-testid="swap-hint"]')).not.toBeNull();
    change(() => seen.setView(withAran("Dorn")));
    expect(q('[data-testid="swap-hint"]')).toBeNull();
    expect(seen.draft).toBeNull();
    // Commande en vol : puces, annulation d'échange et lignes de réserve inactives.
    click(slot(1, 1));
    click(action("swap"));
    change(() => seen.setBusy(true));
    expect(q<HTMLButtonElement>('[data-testid="swap-cancel"]')!.disabled).toBe(true);
    expect(q<HTMLButtonElement>('[data-testid="group-chip-g1"]')!.disabled).toBe(true);
    change(() => seen.setBusy(false));
    key(document.body, "Escape");
    click(slot(1, 2));
    change(() => seen.setBusy(true));
    expect(q<HTMLButtonElement>('[data-testid="reserve-Aran"]')!.disabled).toBe(true);
  });

  it("never overwrites a spot a refresh filled while the reserve was open: its actions open instead", () => {
    const seen = render();
    click(slot(1, 1));
    expect(q('[data-testid="reserve"]')).not.toBeNull();
    change(() =>
      seen.setView(
        locked({ groups: [organizerGroup({ id: "g1", starters: [{ candidatureId: "Dorn", party: 1, index: 1, unavailable: false }] })] }),
      ),
    );
    click(q('[data-testid="reserve-Aran"]')!);
    expect(seen.draft).toBeNull();
    expect(q('[data-testid="reserve"]')).toBeNull();
    expect(text(q('[data-testid="slot-actions"] h2'))).toBe("Dorn[SA]");
    expect(text(slot(1, 1))).toContain("Dorn[SA]");
  });
});
