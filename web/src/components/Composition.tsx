// Onglet Composition de `/o` (SPEC epic 3 CAP-3, EXPERIENCE.md *Écran de composition*), chargé à la
// demande (`React.lazy`, chunk séparé, AD-19) avec ses sous-modules : `RoomSlot`, `ReserveSheet` et
// `SlotActions`. Composant piloté : le brouillon et le bandeau vivent dans `OrganizerPage` (ils
// survivent à un changement d'onglet) ; ici, l'affichage et les gestes.
// - Puces « G1 · jeu. 21:00 » des groupes retenus, sélection unique ; dessous, l'état « 8 / 10 ·
//   manque 1 Soin » (manques par rôle d'après les minimums, comme les comptes publics) et les classes
//   hors bornes.
// - Équipe 1 puis Équipe 2, slots 1 à `partySize` (disposition figée du groupe), puis le banc
//   ordonné et « + Ajouter au banc ».
// - Cible libre : la réserve. Place occupée : « Retirer », « Au banc », « Échanger » (puis toucher une
//   autre place, de ce groupe ou d'un autre). Une candidature n'est jamais à deux places.
// - Avertissements en ligne, jamais bloquants : hors brouillon, ceux du serveur ; en brouillon, les
//   mêmes règles recalculées sur le brouillon.
// - Barre fixée en bas : en brouillon « Enregistrer » (principal) et « Abandonner » (confirmé) ; hors
//   brouillon, vide (emplacement de « Composer automatiquement », epic 4).
// - Bandeau « N candidatures ont changé depuis ton brouillon » + « Voir » (confirmé) ; le brouillon
//   n'est jamais écrasé. En brouillon, une candidature qui n'est plus `accepted` ou disparue le dit ;
//   un refus structurel désigne en ligne ses places fautives.
// - Un échange en cours s'annule par Échap, à l'enregistrement, à l'abandon du brouillon, ou si
//   l'occupant de la source change ; pendant l'échange, la source est `aria-pressed`.

import { useEffect, useId, useRef, useState, type RefObject } from "react";
import { labelOf } from "../api/reference";
import type { ClassWarning, OrganizerCandidature, OrganizerEvent, OrganizerGroup, ReferenceSet } from "../api/types";
import { t, useLang, type MessageKey } from "../i18n";
import {
  isBench,
  occupant,
  placeAt,
  placementOf,
  retainedGroups,
  sameSeat,
  slotKey,
  swapSeats,
  toBench,
  unplace,
  type GroupPlacement,
  type Placement,
  type Seat,
} from "../lib/placement";
import { formatSlot } from "../lib/time";
import { ConfirmSheet } from "./ConfirmSheet";
import { roleLabel } from "../lib/labels";
import { SleepingGroups } from "./SleepingGroups";
import { ReserveSheet } from "./ReserveSheet";
import { RoomSlot, type SlotPerson } from "./RoomSlot";
import { SlotActions } from "./SlotActions";
import { teamsText } from "./CandidatureRow";

/** Avertissements d'un groupe : candidatures hors créneau, classes hors bornes. */
export interface GroupWarnings {
  unavailable: ReadonlySet<string>;
  classWarnings: Record<string, ClassWarning>;
}

/** Hors brouillon, les avertissements du serveur ; en brouillon, recalculés sur `gp` avec les mêmes
 * règles (`domain::placement::warnings`) : indisponible si le créneau du groupe n'est pas coché,
 * classe bornée dont le compte de titulaires sort de `[min, max]`. */
export function groupWarnings(view: OrganizerEvent, group: OrganizerGroup, gp: GroupPlacement | null): GroupWarnings {
  if (!gp) {
    return {
      unavailable: new Set(
        [...group.starters, ...group.bench].filter((s) => s.unavailable).map((s) => s.candidatureId),
      ),
      classWarnings: group.classWarnings,
    };
  }
  const byId = new Map(view.candidatures.map((c) => [c.id, c]));
  const placed = [...Object.values(gp.starters), ...gp.bench];
  const unavailable = new Set(
    placed.filter((id) => !byId.get(id)?.availabilities.some((a) => a.slotId === group.slotId)),
  );
  const classes = Object.values(gp.starters).map((id) => byId.get(id)?.class);
  const classWarnings: Record<string, ClassWarning> = {};
  for (const [cls, b] of Object.entries(view.classBounds)) {
    const count = classes.filter((c) => c === cls).length;
    if (count < b.min || count > b.max) classWarnings[cls] = { count, min: b.min, max: b.max };
  }
  return { unavailable, classWarnings };
}

/** Manques par rôle d'un groupe : somme des minimums des classes bornées de chaque rôle, moins ses
 * titulaires de ce rôle ; strictement positifs, triés par id de rôle. Sans référence, le rôle d'une
 * classe est inconnu : aucun manque plutôt qu'un faux. */
export function missingRoles(
  view: OrganizerEvent,
  reference: ReferenceSet | null,
  starters: readonly string[],
): { role: string; missing: number }[] {
  if (!reference) return [];
  const roleOfClass = (cls: string) => reference.classes.find((c) => c.id === cls)?.role ?? cls;
  const wanted = new Map<string, number>();
  for (const [cls, b] of Object.entries(view.classBounds)) {
    const role = roleOfClass(cls);
    wanted.set(role, (wanted.get(role) ?? 0) + b.min);
  }
  const byId = new Map(view.candidatures.map((c) => [c.id, c]));
  const have = new Map<string, number>();
  for (const id of starters) {
    const role = byId.get(id)?.role;
    if (role !== undefined) have.set(role, (have.get(role) ?? 0) + 1);
  }
  return [...wanted]
    .map(([role, want]) => ({ role, missing: want - (have.get(role) ?? 0) }))
    .filter((m) => m.missing > 0)
    .sort((a, b) => (a.role < b.role ? -1 : a.role > b.role ? 1 : 0));
}

/** « 3 / max 2 » ou « 1 / min 2 ». */
function warningText(w: ClassWarning): string {
  return w.count > w.max
    ? t("grid.coverage.max", { count: w.count, max: w.max })
    : t("grid.coverage.min", { count: w.count, min: w.min });
}

function plural(lang: string, count: number, one: MessageKey, other: MessageKey): MessageKey {
  return new Intl.PluralRules(lang).select(count) === "one" ? one : other;
}

export interface CompositionProps {
  view: OrganizerEvent;
  reference: ReferenceSet | null;
  /** Placement du brouillon ; `null` hors brouillon (le placement du serveur s'affiche). */
  draft: Placement | null;
  /** Tout geste passe par ici : la page crée le brouillon au premier. */
  onEdit: (next: Placement) => void;
  onSave: () => void;
  onDiscard: () => void;
  /** « Voir » : abandonne le brouillon et relit la vue. */
  onSee: () => void;
  /** Candidatures changées depuis le début du brouillon. */
  changed: number;
  /** Enregistrement en vol. */
  saving: boolean;
  /** Une commande est en vol : aucun geste. */
  busy: boolean;
  /** Erreur du dernier enregistrement, déjà traduite. */
  error: string | null;
  /** Places désignées par les champs d'un refus structurel, avec leur message. */
  seatErrors?: readonly SeatError[];
  /** Repli du focus quand le déclencheur d'une feuille a disparu. */
  fallbackFocus?: RefObject<HTMLElement | null>;
}

/** Une place refusée par le serveur (`placements.N.<champ>`), et le texte de son code. */
export interface SeatError {
  seat: Seat;
  message: string;
}

type OpenSheet =
  | { kind: "reserve"; seat: Seat }
  | { kind: "actions"; seat: Seat; id: string }
  | { kind: "discard" }
  | { kind: "see" }
  | null;

export function Composition({
  view,
  reference,
  draft,
  onEdit,
  onSave,
  onDiscard,
  onSee,
  changed,
  saving,
  busy,
  error,
  seatErrors = [],
  fallbackFocus,
}: CompositionProps) {
  const lang = useLang();
  const ids = useId();
  const groups = retainedGroups(view);
  const [picked, setPicked] = useState<string | null>(null);
  const [sheet, setSheet] = useState<OpenSheet>(null);
  const trigger = useRef<HTMLElement | null>(null);
  const heading = useRef<HTMLHeadingElement>(null);
  const placement = draft ?? placementOf(view);
  /** Source d'un échange en cours, et son occupant au moment du choix. */
  const [swapping, setSwapping] = useState<{ seat: Seat; id: string } | null>(null);
  const swapFrom = swapping?.seat ?? null;
  const startSwap = (seat: Seat, id: string) => setSwapping({ seat, id });
  const cancelSwap = () => setSwapping(null);
  // Échange caduc : brouillon abandonné ou enregistré, ou occupant de la source changé par une
  // relecture ; le toucher suivant ne doit pas échanger à l'aveugle.
  const hadDraft = useRef(draft !== null);
  useEffect(() => {
    const dropped = hadDraft.current && draft === null;
    hadDraft.current = draft !== null;
    if (swapping && (dropped || occupant(placement, swapping.seat) !== swapping.id)) setSwapping(null);
  });
  // Échap annule l'échange (aucune feuille n'est ouverte pendant un échange).
  useEffect(() => {
    if (!swapping) return;
    const onKey = (e: globalThis.KeyboardEvent) => {
      if (e.key === "Escape") setSwapping(null);
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [swapping]);
  const current = groups.find((g) => g.group.id === picked) ?? groups[0];
  const byId = new Map(view.candidatures.map((c) => [c.id, c]));
  const numberOf = (groupId: string) => groups.find((g) => g.group.id === groupId)?.n ?? 0;
  const slotOf = (id: string) => view.event.slots.find((s) => s.id === id);
  const classLabel = (cls: string) => {
    const known = reference?.classes.find((c) => c.id === cls);
    return known ? labelOf(known.label, lang) : cls;
  };
  const closeSheet = () => setSheet(null);
  const edit = (next: Placement) => {
    if (!busy) onEdit(next);
  };

  const bar = draft !== null && (
    <div data-testid="composition-bar" className="fixed inset-x-0 bottom-0 z-20 border-t border-border bg-surface-base">
      <div className="mx-auto flex max-w-content flex-col gap-2 px-3 py-3 min-[360px]:px-4">
        <p className="text-meta text-ink-secondary">{t("composition.draft")}</p>
        {error && (
          <p role="alert" data-testid="composition-error" className="text-label text-danger">
            {error}
          </p>
        )}
        <div className="flex gap-2">
          <button
            type="button"
            data-testid="composition-save"
            disabled={busy}
            onClick={() => {
              cancelSwap();
              onSave();
            }}
            className="min-h-tap flex-1 rounded-md bg-accent px-4 text-label text-on-accent disabled:bg-surface-sunken disabled:text-ink-disabled"
          >
            {saving ? t("composition.sending") : t("composition.save")}
          </button>
          <button
            type="button"
            data-testid="composition-discard"
            disabled={busy}
            onClick={(e) => {
              trigger.current = e.currentTarget;
              setSheet({ kind: "discard" });
            }}
            className="min-h-tap flex-1 rounded-md border border-border px-4 text-label text-ink-primary disabled:text-ink-disabled"
          >
            {t("composition.discard")}
          </button>
        </div>
      </div>
    </div>
  );

  if (!current) {
    return (
      <section data-testid="composition" className="mt-4">
        <h2 className="sr-only">{t("organizer.tab.composition")}</h2>
        <p className="text-body text-ink-secondary">{t("groups.empty")}</p>
        {bar}
      </section>
    );
  }

  const group = current.group;
  const gp = placement[group.id] ?? { starters: {}, bench: [] };
  const warnings = groupWarnings(view, group, draft ? gp : null);
  const starters = Object.values(gp.starters);
  const missing = missingRoles(view, reference, starters);
  const state = [
    t("composition.state", { count: starters.length, total: group.parties * group.partySize }),
    ...missing.map((m) => t("event.missing", { count: m.missing, role: roleLabel(reference, m.role, lang) })),
  ].join(" · ");
  const classLines = Object.entries(warnings.classWarnings).map(([cls, w]) => ({
    cls,
    text: t("composition.classWarning", { class: classLabel(cls), text: warningText(w) }),
  }));

  const person = (id: string, party?: number): SlotPerson => {
    const c: OrganizerCandidature | null = byId.get(id) ?? null;
    const w = c ? warnings.classWarnings[c.class] : undefined;
    // En brouillon, une candidature disparue de la vue ou qui n'est plus `accepted` ferait échouer
    // l'enregistrement : elle le dit avant.
    const gone = !draft
      ? null
      : !c
        ? t("composition.absent")
        : c.status !== "accepted"
          ? t("composition.notAccepted")
          : null;
    return {
      candidature: c,
      id,
      role: c ? roleLabel(reference, c.role, lang) : "",
      className: c ? classLabel(c.class) : "",
      unavailable: warnings.unavailable.has(id),
      classWarning: w ? warningText(w) : null,
      gone,
      teams: c && group.parties > 1 ? teamsText(c) : "",
      offTeam: !!c && party !== undefined && c.teams.length > 0 && !c.teams.includes(party),
    };
  };
  /** Rôles demandés pour l'Équipe `party` moins ses titulaires de ce rôle, strictement positifs. */
  const teamMissing = (party: number): { role: string; missing: number }[] => {
    const wanted = view.teamRoles?.[party - 1] ?? {};
    const have = new Map<string, number>();
    for (const index of Array.from({ length: group.partySize }, (_, i) => i + 1)) {
      const role = byId.get(gp.starters[slotKey(party, index)] ?? "")?.role;
      if (role !== undefined) have.set(role, (have.get(role) ?? 0) + 1);
    }
    return Object.entries(wanted)
      .map(([role, want]) => ({ role, missing: want - (have.get(role) ?? 0) }))
      .filter((m) => m.missing > 0)
      .sort((a, b) => (a.role < b.role ? -1 : a.role > b.role ? 1 : 0));
  };
  const errorAt = (seat: Seat) => seatErrors.find((e) => sameSeat(e.seat, seat))?.message ?? null;
  /** Nom accessible : la place, l'occupant ou « libre », puis chaque avertissement. */
  const slotLabel = (place: string, p: SlotPerson | null, error: string | null) =>
    [
      place,
      p?.gone,
      p?.unavailable ? t("composition.unavailable") : null,
      p?.offTeam ? t("composition.offTeam") : null,
      p?.classWarning ? `${p.className} ${p.classWarning}` : null,
      error,
    ]
      .filter((x): x is string => !!x)
      .join(", ");
  const who = (p: SlotPerson | null) => (p ? (p.candidature?.pseudoSrv ?? p.id) : t("composition.free"));

  /** Toucher une place : fin d'un échange, actions d'une place occupée, ou réserve d'une place
   * libre. */
  function tap(seat: Seat, from: HTMLButtonElement) {
    if (busy) return;
    if (swapFrom) {
      const from = swapFrom;
      cancelSwap();
      if (!sameSeat(from, seat)) edit(swapSeats(placement, from, seat));
      return;
    }
    trigger.current = from;
    const id = occupant(placement, seat);
    setSheet(id ? { kind: "actions", seat, id } : { kind: "reserve", seat });
  }

  const targetText = (seat: Seat) =>
    isBench(seat)
      ? t("composition.target.bench")
      : t("composition.target.slot", { party: seat.party, index: seat.index });
  const activeSeat: Seat | null =
    swapFrom ?? (sheet?.kind === "reserve" || sheet?.kind === "actions" ? sheet.seat : null);
  const isActive = (seat: Seat) => activeSeat !== null && sameSeat(activeSeat, seat);

  const parties = Array.from({ length: group.parties }, (_, i) => i + 1);
  const indexes = Array.from({ length: group.partySize }, (_, i) => i + 1);
  const benchAdd: Seat = { groupId: group.id, bench: gp.bench.length };

  return (
    <section data-testid="composition" className={`mt-4 ${draft ? "pb-40" : ""}`}>
      <h2 ref={heading} tabIndex={-1} className="sr-only">
        {t("organizer.tab.composition")}
      </h2>

      {draft && changed > 0 && (
        <div
          role="status"
          data-testid="composition-changed"
          className="mb-3 flex items-center gap-3 rounded-md border border-accent px-3 py-2"
        >
          <p className="min-w-0 flex-1 text-label">
            {t(plural(lang, changed, "composition.changed.one", "composition.changed.other"), { count: changed })}
          </p>
          <button
            type="button"
            data-testid="composition-see"
            disabled={busy}
            onClick={(e) => {
              trigger.current = e.currentTarget;
              setSheet({ kind: "see" });
            }}
            className="min-h-tap shrink-0 rounded-md border border-border px-3 text-label text-ink-primary"
          >
            {t("composition.see")}
          </button>
        </div>
      )}

      <div role="group" aria-label={t("composition.groups")} className="flex flex-wrap gap-2">
        {groups.map(({ n, group: g }) => {
          const on = g.id === group.id;
          const slot = slotOf(g.slotId);
          return (
            <button
              key={g.id}
              type="button"
              data-testid={`group-chip-${g.id}`}
              aria-pressed={on}
              disabled={busy}
              onClick={() => setPicked(g.id)}
              className={`min-h-tap rounded-full border-2 bg-surface-raised px-3 text-label tabular-nums ${on ? "border-accent" : "border-border"}`}
            >
              {t("composition.chip", { n, slot: slot ? formatSlot(slot.startsAt, lang) : "" })}
            </button>
          );
        })}
      </div>
      <p data-testid="composition-state" className="mt-2 text-label tabular-nums">
        {state}
      </p>
      {classLines.length > 0 && (
        <ul data-testid="composition-classes" className="mt-1">
          {classLines.map((l) => (
            <li key={l.cls} className="text-meta text-danger tabular-nums">
              {l.text}
            </li>
          ))}
        </ul>
      )}

      {swapFrom && (
        <div data-testid="swap-hint" role="status" className="mt-3 flex items-center gap-3">
          <p className="min-w-0 flex-1 text-label">
            {t("composition.swapHint", {
              name: byId.get(swapping?.id ?? "")?.pseudoSrv ?? swapping?.id ?? "",
            })}
          </p>
          <button
            type="button"
            data-testid="swap-cancel"
            disabled={busy}
            onClick={cancelSwap}
            className="min-h-tap shrink-0 rounded-md border border-border px-3 text-label text-ink-primary"
          >
            {t("composition.swapCancel")}
          </button>
        </div>
      )}

      {parties.map((party) => (
        <section key={party} data-testid={`party-${party}`} className="mt-4">
          <h3 id={`${ids}-party-${party}`} className="text-label text-ink-secondary">
            {t("composition.party", { n: party })}
          </h3>
          {teamMissing(party).length > 0 && (
            <p data-testid={`party-missing-${party}`} className="text-meta text-danger">
              {teamMissing(party)
                .map((m) => t("event.missing", { count: m.missing, role: roleLabel(reference, m.role, lang) }))
                .join(" · ")}
            </p>
          )}
          <ol aria-labelledby={`${ids}-party-${party}`} className="mt-1 flex flex-col gap-1">
            {indexes.map((index) => {
              const seat: Seat = { groupId: group.id, party, index };
              const id = gp.starters[slotKey(party, index)];
              const p = id ? person(id, party) : null;
              const error = errorAt(seat);
              return (
                <li key={index}>
                  <RoomSlot
                    n={index}
                    person={p}
                    label={slotLabel(t("composition.slot.label", { party, index, who: who(p) }), p, error)}
                    error={error}
                    active={isActive(seat)}
                    swapping={swapFrom !== null}
                    swapSource={swapFrom !== null && sameSeat(swapFrom, seat)}
                    testId={`slot-${party}-${index}`}
                    disabled={busy}
                    onTap={(el) => tap(seat, el)}
                  />
                </li>
              );
            })}
          </ol>
        </section>
      ))}

      <section data-testid="bench" className="mt-4">
        <h3 id={`${ids}-bench`} className="text-label text-ink-secondary">
          {t("composition.bench")}
        </h3>
        {gp.bench.length > 0 && (
          <ol aria-labelledby={`${ids}-bench`} className="mt-1 flex flex-col gap-1">
            {gp.bench.map((id, i) => {
              const seat: Seat = { groupId: group.id, bench: i };
              const p = person(id);
              const error = errorAt(seat);
              return (
                <li key={id}>
                  <RoomSlot
                    n={i + 1}
                    person={p}
                    label={slotLabel(t("composition.bench.label", { n: i + 1, who: who(p) }), p, error)}
                    error={error}
                    active={isActive(seat)}
                    swapping={swapFrom !== null}
                    swapSource={swapFrom !== null && sameSeat(swapFrom, seat)}
                    testId={`bench-${i + 1}`}
                    disabled={busy}
                    onTap={(el) => tap(seat, el)}
                  />
                </li>
              );
            })}
          </ol>
        )}
        <button
          type="button"
          data-testid="bench-add"
          aria-haspopup={swapFrom ? undefined : "dialog"}
          disabled={busy}
          data-active={isActive(benchAdd) ? "true" : undefined}
          onClick={(e) => tap(benchAdd, e.currentTarget)}
          className={`mt-1 min-h-tap w-full rounded-md border border-dashed px-3 text-left text-label ${isActive(benchAdd) ? "border-2 border-accent" : "border-border"} text-ink-primary`}
        >
          {t("composition.benchAdd")}
        </button>
      </section>

      <SleepingGroups view={view} testId="composition" />

      {/* Hors brouillon, la barre est vide : « Composer automatiquement » y viendra (epic 4). */}
      {bar}

      {sheet?.kind === "reserve" && (
        <ReserveSheet
          view={view}
          reference={reference}
          placement={placement}
          groupId={sheet.seat.groupId}
          slotId={group.slotId}
          target={targetText(sheet.seat)}
          groupNumber={numberOf}
          busy={busy}
          onPick={(id) => {
            // Une relecture a rempli la place pendant que la réserve était ouverte : rien n'est
            // écrasé, ses actions s'ouvrent à la place.
            const taken = occupant(placement, sheet.seat);
            if (taken && taken !== id) {
              setSheet({ kind: "actions", seat: sheet.seat, id: taken });
              return;
            }
            closeSheet();
            edit(placeAt(placement, id, sheet.seat));
          }}
          onClose={closeSheet}
          trigger={trigger}
          fallbackFocus={fallbackFocus ?? heading}
        />
      )}
      {sheet?.kind === "actions" && (
        <SlotActions
          name={byId.get(sheet.id)?.pseudoSrv ?? sheet.id}
          canBench={!isBench(sheet.seat)}
          onRemove={() => {
            closeSheet();
            edit(unplace(placement, sheet.id));
          }}
          onBench={() => {
            closeSheet();
            edit(toBench(placement, sheet.id));
          }}
          onSwap={() => {
            closeSheet();
            startSwap(sheet.seat, sheet.id);
          }}
          onClose={closeSheet}
          trigger={trigger}
          fallbackFocus={fallbackFocus ?? heading}
        />
      )}
      {sheet?.kind === "discard" && (
        <ConfirmSheet
          message={t("composition.discardConfirm")}
          confirmLabel={t("composition.discard")}
          cancelLabel={t("composition.discardKeep")}
          busyLabel={t("composition.sending")}
          onConfirm={() => {
            closeSheet();
            cancelSwap();
            onDiscard();
          }}
          onClose={closeSheet}
          trigger={trigger}
          fallbackFocus={fallbackFocus ?? heading}
        />
      )}
      {sheet?.kind === "see" && (
        <ConfirmSheet
          message={t("composition.seeConfirm")}
          confirmLabel={t("composition.seeAction")}
          cancelLabel={t("composition.discardKeep")}
          busyLabel={t("composition.sending")}
          onConfirm={() => {
            closeSheet();
            cancelSwap();
            onSee();
          }}
          onClose={closeSheet}
          trigger={trigger}
          fallbackFocus={fallbackFocus ?? heading}
        />
      )}
    </section>
  );
}

export default Composition;
