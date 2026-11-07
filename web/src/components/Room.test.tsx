// @vitest-environment happy-dom
// Onglet Salle (story 3-4), rendu seul avec une vue tenue par le test comme le fait la page : cartes
// dans l'ordre des créneaux, recette en deux ou trois lignes, puces à copier, banc, « Copier le
// message Discord » et son repli, « Régénérer le nom » derrière « … » et une confirmation. Fuseau
// fixé avant tout calcul de date : UTC+9, sans heure d'été.
process.env.TZ = "Asia/Tokyo";

import { act, useState } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { OrganizerEvent } from "../api/types";
import { resetLangForTests, setLang } from "../i18n";
import { discordMessage } from "../lib/discord";
import { click, key, mount, type Mounted } from "../test/dom";
import { KAEL, NYRA, NYRA_TWIN, TOMA, organizerEvent, organizerGroup, reference } from "../test/fixtures";
import { TOAST_MS } from "./CopyChip";
import { Room, type RegenerateOutcome } from "./Room";

const ORIGIN = "https://raid.example.org";
let m: Mounted;

/** G1 sur `slot_a` (Kael en 1.1, nyra en 2.5, banc Toma puis Nyra ; nyra et Toma n'ont pas coché
 * le créneau), un groupe en sommeil, G2 vide sur `slot_b`. */
function locked(patch: Partial<OrganizerEvent["event"]> = {}): OrganizerEvent {
  return {
    ...organizerEvent,
    event: { ...organizerEvent.event, status: "locked", ...patch },
    groups: [
      organizerGroup({
        id: "g1",
        starters: [
          { candidatureId: KAEL, party: 1, index: 1, unavailable: false },
          { candidatureId: NYRA_TWIN, party: 2, index: 5, unavailable: true },
        ],
        bench: [
          { candidatureId: NYRA, benchIndex: 2, unavailable: false },
          { candidatureId: TOMA, benchIndex: 1, unavailable: true },
        ],
      }),
      organizerGroup({ id: "zz", retained: false }),
      organizerGroup({ id: "g2", slotId: "slot_b" }),
    ],
  };
}

interface Seen {
  setView: (v: OrganizerEvent) => void;
  /** Une commande en vol ailleurs, comme la page. */
  setBusy: (busy: boolean) => void;
  view: OrganizerEvent;
}

function Harness({
  initial,
  seen,
  onRegenerate = async () => ({ close: true }),
  regenerating = false,
  draft = false,
  withReference = true,
}: {
  initial: OrganizerEvent;
  seen: Seen;
  onRegenerate?: (groupId: string) => Promise<RegenerateOutcome>;
  regenerating?: boolean;
  draft?: boolean;
  withReference?: boolean;
}) {
  const [view, setView] = useState(initial);
  const [busy, setBusy] = useState(false);
  Object.assign(seen, { setView, setBusy, view });
  return (
    <Room
      view={view}
      reference={withReference ? reference : null}
      origin={ORIGIN}
      onRegenerate={onRegenerate}
      regenerating={regenerating}
      busy={regenerating || busy}
      draft={draft}
    />
  );
}

function render(initial: OrganizerEvent = locked(), props: Partial<Parameters<typeof Harness>[0]> = {}) {
  const seen = {} as Seen;
  m = mount(<Harness initial={initial} seen={seen} {...props} />);
  return seen;
}

const q = <T extends Element>(sel: string) => document.querySelector<T>(sel);
const qa = (sel: string) => [...document.querySelectorAll<HTMLElement>(sel)];
const card = (id: string) => q<HTMLElement>(`[data-testid="room-card-${id}"]`)!;
const recipe = (id: string) => [...card(id).querySelectorAll('[data-testid="room-recipe"] li')].map((li) => li.textContent);
const dialog = () => q<HTMLElement>('[role="dialog"]');
const inDialog = (label: string) =>
  [...(dialog()?.querySelectorAll<HTMLButtonElement>("button") ?? [])].find((b) => b.textContent === label)!;

async function flush() {
  await act(async () => {
    for (let i = 0; i < 10; i++) await Promise.resolve();
  });
}

async function press(b: Element | null) {
  await act(async () => (b as HTMLElement).click());
  await flush();
}

let writeText: ReturnType<typeof vi.fn>;

beforeEach(() => {
  localStorage.clear();
  vi.spyOn(navigator, "language", "get").mockReturnValue("fr-FR");
  resetLangForTests();
  writeText = vi.fn(async () => {});
  Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true, writable: true });
});

afterEach(() => {
  m.unmount();
  vi.restoreAllMocks();
  vi.useRealTimers();
  localStorage.clear();
});

describe("room tab", () => {
  it("shows one card per retained group, in slot order, the sleeping group only counted", () => {
    render();
    expect(qa('[data-testid^="room-card-"]').map((c) => c.dataset.testid)).toEqual(["room-card-g1", "room-card-g2"]);
    expect(qa("article h3").map((h) => h.textContent)).toEqual(["G1 · mer. 05:00", "G2 · ven. 05:00"]);
    expect(q('[data-testid="room-sleeping"]')!.textContent).toBe("1 groupe en sommeil, repris s'il est reverrouillé.");
    // Un seul bouton principal, en tête.
    const primary = q<HTMLButtonElement>('[data-testid="room-copy-discord"]')!;
    expect(primary.textContent).toBe("Copier le message Discord");
    expect(q('[data-testid="room"]')!.querySelectorAll(".bg-accent")).toHaveLength(1);
  });

  it("puts the room name first, in display size, in a copy chip", async () => {
    render();
    const chip = card("g1").querySelector<HTMLButtonElement>('[data-variant="display"]')!;
    expect(chip.textContent).toBe("I1-G1");
    expect(chip.className).toContain("text-display");
    expect(chip.getAttribute("aria-label")).toBe("Copier : Nom de salle");
    await press(chip);
    expect(writeText).toHaveBeenLastCalledWith("I1-G1");
  });

  it("writes the recipe in three lines on an all-faction raid, two otherwise, by instance kind", () => {
    render();
    expect(recipe("g1")).toEqual(["Crée une salle de Sanctuaire : Instance Une", "Nom : I1-G1", "Faction : All"]);
    m.unmount();
    render(locked({ factionScope: "asmodian" }));
    expect(recipe("g1")).toEqual(["Crée une salle de Sanctuaire : Instance Une", "Nom : I1-G1"]);
    m.unmount();
    render(locked({ instance: "mono", factionScope: "elyos" }));
    expect(recipe("g2")).toEqual(["Crée une salle d'expédition : Mono", "Nom : I1-G2"]);
    m.unmount();
    setLang("en");
    render();
    expect(recipe("g1")).toEqual(["Create a Sanctuary room: Instance One", "Name: I1-G1", "Faction: All"]);
  });

  it("lists Party 1 then Party 2 slot by slot, with the role and a copy chip, free slots greyed", async () => {
    render();
    const slot = (party: number, index: number) => q<HTMLElement>(`[data-testid="room-slot-g1-${party}-${index}"]`)!;
    expect(slot(1, 1).textContent).toBe("1DPSKael[SE]");
    expect(slot(2, 5).textContent).toBe("5Soinnyra[SA]pas dispo à ce créneau");
    expect(slot(1, 2).textContent).toBe("2libre");
    expect(slot(1, 2).querySelector(".text-ink-disabled")!.textContent).toBe("libre");
    expect(slot(1, 2).querySelector("button")).toBeNull();
    const chip = slot(2, 5).querySelector("button")!;
    expect(chip.getAttribute("aria-label")).toBe("Copier : Équipe 2, slot 5, nyra[SA]");
    await press(chip);
    expect(writeText).toHaveBeenLastCalledWith("nyra[SA]");
    // Disposition figée : deux équipes de cinq.
    expect(card("g1").querySelectorAll('[data-testid^="room-slot-g1-"]')).toHaveLength(10);
    expect([...card("g1").querySelectorAll("h4:not(.sr-only)")].map((h) => h.textContent)).toEqual([
      "Équipe 1",
      "Équipe 2",
      "Banc",
    ]);
  });

  it("lists the bench in order with copy chips, and drops it when empty", async () => {
    render();
    const bench = q<HTMLElement>('[data-testid="room-bench-g1"]')!;
    const chips = [...bench.querySelectorAll("button")];
    expect(chips.map((b) => b.textContent)).toEqual(["Toma[SE]", "Nyra[SA]"]);
    await press(chips[1]);
    expect(writeText).toHaveBeenLastCalledWith("Nyra[SA]");
    expect(q('[data-testid="room-bench-g2"]')).toBeNull();
    // Sans aucun placement, chaque slot est libre.
    expect(card("g2").textContent).not.toMatch(/\[S[AE]\]/);
    expect(card("g2").querySelectorAll(".text-ink-disabled")).toHaveLength(10);
  });

  it("copies the whole Discord message, then says Copié for 1.5 s", async () => {
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
    const seen = render();
    await press(q('[data-testid="room-copy-discord"]'));
    expect(writeText).toHaveBeenCalledTimes(1);
    expect(writeText).toHaveBeenLastCalledWith(discordMessage(seen.view, reference, "fr", ORIGIN));
    expect(q('[role="status"]')!.textContent).toBe("Copié");
    await act(async () => {
      vi.advanceTimersByTime(TOAST_MS);
    });
    expect(q('[role="status"]')).toBeNull();
    expect(q('[data-testid="room-message"]')).toBeNull();
  });

  it("copies even without any placement", async () => {
    const view = { ...locked(), groups: [organizerGroup({ id: "g1" })] };
    render(view);
    await press(q('[data-testid="room-copy-discord"]'));
    const text = String(writeText.mock.calls[0][0]);
    expect(text).toContain("1. libre");
    expect(text).not.toMatch(/Banc/);
  });

  it("shows the message selected in a read-only box when the clipboard fails", async () => {
    writeText.mockRejectedValue(new Error("denied"));
    const seen = render();
    await press(q('[data-testid="room-copy-discord"]'));
    const box = q<HTMLTextAreaElement>('[data-testid="room-message"]')!;
    const text = discordMessage(seen.view, reference, "fr", ORIGIN);
    expect(box.readOnly).toBe(true);
    expect(box.value).toBe(text);
    expect(document.activeElement).toBe(box);
    expect([box.selectionStart, box.selectionEnd]).toEqual([0, text.length]);
    expect(q('[role="status"]')!.textContent).toMatch(/Copie impossible/);
    // Un essai réussi ensuite range la zone.
    writeText.mockResolvedValue(undefined);
    await press(q('[data-testid="room-copy-discord"]'));
    expect(q('[data-testid="room-message"]')).toBeNull();
  });

  it("regenerates the name behind … and a confirmation, then shows the new name", async () => {
    let seen: Seen;
    const onRegenerate = vi.fn(async (groupId: string) => {
      const v = seen.view;
      seen.setView({
        ...v,
        groups: v.groups.map((g) => (g.id === groupId ? { ...g, roomName: "I1-NEW1" } : g)),
      });
      return { close: true } as const;
    });
    seen = render(locked(), { onRegenerate });
    const more = q<HTMLButtonElement>('[data-testid="room-more-g1"]')!;
    expect(more.getAttribute("aria-label")).toBe("Plus d'actions · G1");
    expect(more.getAttribute("aria-expanded")).toBe("false");
    expect(q('[data-testid="room-regenerate-g1"]')).toBeNull();
    click(more);
    expect(more.getAttribute("aria-expanded")).toBe("true");
    // Échap referme le menu et rend le focus à « … ».
    q<HTMLButtonElement>('[data-testid="room-regenerate-g1"]')!.focus();
    key(document.activeElement!, "Escape");
    expect(q('[data-testid="room-regenerate-g1"]')).toBeNull();
    expect(more.getAttribute("aria-expanded")).toBe("false");
    expect(document.activeElement).toBe(more);
    // Un toucher hors de la carte le referme ; un toucher dans la carte non.
    click(more);
    click(card("g1").querySelector("h3")!);
    expect(q('[data-testid="room-regenerate-g1"]')).not.toBeNull();
    click(card("g2").querySelector("h3")!);
    expect(q('[data-testid="room-regenerate-g1"]')).toBeNull();
    click(more);
    click(q('[data-testid="room-regenerate-g1"]')!);
    expect(dialog()!.textContent).toContain("Le nom change. Celui que tu as posté sur Discord deviendra faux.");
    // « Garder le nom » ne fait rien.
    click(inDialog("Garder le nom"));
    expect(dialog()).toBeNull();
    expect(onRegenerate).not.toHaveBeenCalled();
    expect(document.activeElement).toBe(more);

    click(more);
    click(q('[data-testid="room-regenerate-g1"]')!);
    await press(inDialog("Régénérer"));
    expect(onRegenerate).toHaveBeenCalledWith("g1");
    expect(dialog()).toBeNull();
    expect(card("g1").querySelector('[data-variant="display"]')!.textContent).toBe("I1-NEW1");
    expect(recipe("g1")[1]).toBe("Nom : I1-NEW1");
    expect(document.activeElement).toBe(more);
  });

  it("keeps the sheet open with the error when the command fails, closes it on a state refusal", async () => {
    const onRegenerate = vi
      .fn<(groupId: string) => Promise<RegenerateOutcome>>()
      .mockResolvedValueOnce({ close: false, error: "Hors ligne." });
    render(locked(), { onRegenerate });
    click(q('[data-testid="room-more-g2"]')!);
    click(q('[data-testid="room-regenerate-g2"]')!);
    await press(inDialog("Régénérer"));
    expect(dialog()).not.toBeNull();
    expect(dialog()!.querySelector('[role="alert"]')!.textContent).toBe("Hors ligne.");
    // Refus d'état : la page l'a dit et relit ; la feuille se ferme.
    onRegenerate.mockResolvedValueOnce({ close: true });
    await press(inDialog("Régénérer"));
    expect(onRegenerate).toHaveBeenLastCalledWith("g2");
    expect(dialog()).toBeNull();
  });

  it("closes the confirmation when its group is no longer retained", async () => {
    const seen = render();
    click(q('[data-testid="room-more-g2"]')!);
    click(q('[data-testid="room-regenerate-g2"]')!);
    expect(dialog()).not.toBeNull();
    act(() => seen.setView({ ...seen.view, groups: seen.view.groups.filter((g) => g.id !== "g2") }));
    await flush();
    expect(dialog()).toBeNull();
  });

  it("disables Régénérer while the command is in flight", () => {
    render(locked(), { regenerating: true });
    click(q('[data-testid="room-more-g1"]')!);
    expect(q<HTMLButtonElement>('[data-testid="room-regenerate-g1"]')!.disabled).toBe(true);
  });

  it("disables the confirm button while another command is in flight, and keeps the sheet on a busy refusal", async () => {
    const onRegenerate = vi
      .fn<(groupId: string) => Promise<RegenerateOutcome>>()
      .mockResolvedValue({ close: false, error: null });
    const seen = render(locked(), { onRegenerate });
    click(q('[data-testid="room-more-g1"]')!);
    click(q('[data-testid="room-regenerate-g1"]')!);
    act(() => seen.setBusy(true));
    expect(inDialog("Régénérer").disabled).toBe(true);
    act(() => seen.setBusy(false));
    expect(inDialog("Régénérer").disabled).toBe(false);
    // Refus « une commande est déjà en vol » : la feuille reste, sans erreur.
    await press(inDialog("Régénérer"));
    expect(onRegenerate).toHaveBeenCalledTimes(1);
    expect(dialog()).not.toBeNull();
    expect(dialog()!.querySelector('[role="alert"]')).toBeNull();
  });

  it("copies one group's message from its card: its block, then the public link", async () => {
    const seen = render();
    await press(q('[data-testid="room-copy-group-g2"]'));
    expect(q('[data-testid="room-copy-group-g2"]')!.textContent).toBe("Copier le message de G2");
    const text = String(writeText.mock.calls[0][0]);
    expect(text).toBe(discordMessage(seen.view, reference, "fr", ORIGIN, "g2"));
    expect(text).toContain("Salle : I1-G2");
    expect(text).not.toContain("I1-G1");
    expect(text.split("\n").at(-1)).toBe(`${ORIGIN}/e/${seen.view.event.id}`);
    expect(q('[role="status"]')!.textContent).toBe("Copié");
  });

  it("says to copy group by group when the whole message is over 2,000 characters", () => {
    render();
    expect(q('[data-testid="room-too-long"]')).toBeNull();
    m.unmount();
    const many = Array.from({ length: 20 }, (_, i) => organizerGroup({ id: `g${i + 10}` }));
    const view = { ...locked(), groups: many };
    expect(discordMessage(view, reference, "fr", ORIGIN).length).toBeGreaterThan(2000);
    render(view);
    expect(q('[data-testid="room-too-long"]')!.textContent).toBe(
      "Le message dépasse les 2 000 caractères permis par Discord : copie-le groupe par groupe.",
    );
    m.unmount();
    setLang("en");
    render(view);
    expect(q('[data-testid="room-too-long"]')!.textContent).toBe(
      "The message is over Discord's 2,000-character limit: copy it group by group.",
    );
  });

  it("warns that an unsaved composition draft is not in the message", () => {
    render(locked(), { draft: true });
    expect(q('[data-testid="room-draft"]')!.textContent).toBe(
      "Ton brouillon de composition n'est pas enregistré : ce message reprend la dernière version enregistrée.",
    );
    m.unmount();
    render();
    expect(q('[data-testid="room-draft"]')).toBeNull();
  });

  it("marks starters and bench players who did not tick the slot, on the card only", async () => {
    const seen = render();
    expect(card("g1").querySelectorAll('[data-testid="room-unavailable"]')).toHaveLength(2);
    expect(q('[data-testid="room-slot-g1-1-1"] [data-testid="room-unavailable"]')).toBeNull();
    expect(q('[data-testid="room-bench-g1"] li')!.textContent).toBe("1Toma[SE]pas dispo à ce créneau");
    await press(q('[data-testid="room-copy-discord"]'));
    expect(String(writeText.mock.calls[0][0])).not.toContain("pas dispo");
    expect(writeText).toHaveBeenLastCalledWith(discordMessage(seen.view, reference, "fr", ORIGIN));
  });

  it("uses a neutral recipe line when the instance kind is unknown", () => {
    render(locked(), { withReference: false });
    expect(recipe("g1")).toEqual(["Crée la salle : i1", "Nom : I1-G1", "Faction : All"]);
  });

  it("rebuilds the fallback from the current view, reselects it on each failure, and drops the toast", async () => {
    const seen = render();
    // Une copie réussie, puis un échec : le toast « Copié » s'en va.
    await press(q('[data-testid="room-copy-discord"]'));
    expect(q('[role="status"]')!.textContent).toBe("Copié");
    writeText.mockRejectedValue(new Error("denied"));
    await press(q('[data-testid="room-copy-discord"]'));
    expect([...document.querySelectorAll('[role="status"]')].map((e) => e.textContent)).toEqual([
      expect.stringMatching(/^Copie impossible/),
    ]);
    const box = () => q<HTMLTextAreaElement>('[data-testid="room-message"]')!;
    // Relecture ou nouveau nom : la zone suit la vue courante.
    act(() =>
      seen.setView({
        ...seen.view,
        groups: seen.view.groups.map((g) => (g.id === "g1" ? { ...g, roomName: "I1-NEW9" } : g)),
      }),
    );
    expect(box().value).toBe(discordMessage(seen.view, reference, "fr", ORIGIN));
    expect(box().value).toContain("Salle : I1-NEW9");
    // Un second échec identique refocalise et resélectionne.
    q<HTMLButtonElement>('[data-testid="room-copy-discord"]')!.focus();
    box().setSelectionRange(0, 0);
    await press(q('[data-testid="room-copy-discord"]'));
    expect(document.activeElement).toBe(box());
    expect([box().selectionStart, box().selectionEnd]).toEqual([0, box().value.length]);
    // L'échec d'une copie de groupe montre le message de ce groupe.
    await press(q('[data-testid="room-copy-group-g2"]'));
    expect(box().value).toBe(discordMessage(seen.view, reference, "fr", ORIGIN, "g2"));
  });
});
