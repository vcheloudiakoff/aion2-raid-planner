// Onglet Disponibilités de `/o` (SPEC epic 3 CAP-1, EXPERIENCE.md *Grille joueurs × créneaux*,
// DESIGN.md *Layout & Spacing*). Composant pur, nourri par la relecture de `OrganizerPage` :
// - fuseau une fois en tête, sélecteur « Matrice · Par créneau », filtre Tous / Acceptés / En attente ;
// - matrice si 4 créneaux au plus et `MATRIX_MEDIA` vrai (328 px utiles à 16 px ; la loupe réduit le
//   viewport CSS et bascule d'elle-même), sinon vue par créneau ; la raison s'affiche sous le
//   sélecteur, et le choix « Matrice » revient dès qu'elle redevient possible ;
// - lignes : `pending` et `accepted` seulement, triées une fois (ouverture, changement de filtre) ;
//   ensuite une ligne ne bouge plus, une nouvelle s'ajoute à la fin, une ligne sortie du filtre
//   disparaît ;
// - pied, puces et couverture comptent les seules `accepted`, calculés par le serveur
//   (`OrganizerEvent.slots`) : le filtre ne les change jamais.
// Aucune cellule n'est cliquable ; l'en-tête de ligne ouvre la feuille de la candidature. En `open`,
// le bouton principal « Verrouiller » ouvre la feuille de verrouillage de la page (désactivé, avec sa
// raison, sans créneau à venir) ; en `locked` comme en `cancelled`, la grille est en lecture seule.

import { useEffect, useId, useState, type RefObject } from "react";
import { labelOf } from "../api/reference";
import type {
  OrganizerCandidature,
  OrganizerEvent,
  OrganizerSlot,
  PublicSlot,
  ReferenceSet,
} from "../api/types";
import { t, useLang, type MessageKey } from "../i18n";
import { formatSlot, slotParts, timeZoneLabel } from "../lib/time";
import { MATRIX_MEDIA, useMedia } from "../lib/viewport";
import { STATUS_STYLE, srvOf } from "./CandidatureRow";
import { FactionDot } from "./EventSummary";

/** Au-delà, la matrice ne tient plus dans 328 px : vue par créneau. */
export const MATRIX_MAX_SLOTS = 4;

export type GridFilter = "all" | "accepted" | "pending";
const FILTERS: readonly GridFilter[] = ["all", "accepted", "pending"];
const FILTER_KEY: Record<GridFilter, MessageKey> = {
  all: "grid.filter.all",
  accepted: "grid.filter.accepted",
  pending: "grid.filter.pending",
};

function inFilter(c: OrganizerCandidature, filter: GridFilter): boolean {
  if (filter === "all") return c.status === "pending" || c.status === "accepted";
  return c.status === filter;
}

/** Tri d'ouverture : `accepted` d'abord, puis le nombre de créneaux cochés (décroissant), puis l'ordre
 * de création (celui de `rows`). Rend les ids. */
export function sortRows(rows: readonly OrganizerCandidature[]): string[] {
  const rank = (c: OrganizerCandidature) => (c.status === "accepted" ? 0 : 1);
  return rows
    .map((c, i) => ({ c, i }))
    .sort((a, b) => rank(a.c) - rank(b.c) || b.c.availabilities.length - a.c.availabilities.length || a.i - b.i)
    .map(({ c }) => c.id);
}

/** Ordre après une relecture : les lignes déjà là gardent leur place, celles qui sortent de `rows`
 * disparaissent, les nouvelles s'ajoutent à la fin dans l'ordre de création. */
export function keepOrder(previous: readonly string[], rows: readonly OrganizerCandidature[]): string[] {
  const present = new Set(rows.map((c) => c.id));
  const kept = previous.filter((id) => present.has(id));
  const known = new Set(kept);
  return [...kept, ...rows.filter((c) => !known.has(c.id)).map((c) => c.id)];
}

/** Créneau sélectionné par défaut : le premier à venir, sinon le dernier. */
export function defaultSlot(slots: readonly PublicSlot[], now: number): string | null {
  return (slots.find((s) => Date.parse(s.startsAt) > now) ?? slots[slots.length - 1])?.id ?? null;
}

export interface CoverageLine {
  class: string;
  label: string;
  text: string;
  /** Sous le minimum : texte `danger`. */
  under: boolean;
}

/** Une ligne par classe de la couverture, dans l'ordre de la référence (classes inconnues à la fin,
 * par id), libellée par la référence avec repli sur l'id. */
export function coverageLines(
  coverage: Record<string, number>,
  bounds: OrganizerEvent["classBounds"],
  reference: ReferenceSet | null,
  lang: string,
): CoverageLine[] {
  const classes = reference?.classes ?? [];
  const index = (id: string) => {
    const i = classes.findIndex((c) => c.id === id);
    return i < 0 ? classes.length : i;
  };
  return Object.keys(coverage)
    .sort((a, b) => index(a) - index(b) || (a < b ? -1 : a > b ? 1 : 0))
    .map((id) => {
      const count = coverage[id];
      const bound = bounds[id];
      const known = classes.find((c) => c.id === id);
      const label = known ? labelOf(known.label, lang) : id;
      if (bound && count < bound.min) {
        return { class: id, label, text: t("grid.coverage.min", { count, min: bound.min }), under: true };
      }
      if (bound && count > bound.max) {
        return { class: id, label, text: t("grid.coverage.max", { count, max: bound.max }), under: false };
      }
      if (bound && bound.min > 0) {
        return { class: id, label, text: t("grid.coverage.min", { count, min: bound.min }), under: false };
      }
      return { class: id, label, text: String(count), under: false };
    });
}

/** Choix de l'organisateur dans l'onglet, gardé par la page d'un onglet à l'autre. */
export interface GridChoice {
  /** « Matrice » voulue ; gardé même quand la matrice est impossible. */
  wantMatrix: boolean;
  filter: GridFilter;
  /** Créneau sélectionné ; `null` tant que le défaut n'est pas figé. */
  picked: string | null;
}

export const INITIAL_GRID_CHOICE: GridChoice = { wantMatrix: true, filter: "all", picked: null };

interface Props {
  view: OrganizerEvent;
  reference: ReferenceSet | null;
  now: () => number;
  /** Lignes apparues au dernier rafraîchissement : liseré or. */
  fresh: ReadonlySet<string>;
  onOpen: (id: string, trigger: HTMLButtonElement) => void;
  choice: GridChoice;
  onChoice: (patch: Partial<GridChoice>) => void;
  /** Titre du panneau : le focus y revient quand la ligne qui a ouvert la feuille a disparu. */
  headingRef?: RefObject<HTMLHeadingElement | null>;
  /** Ouvre la feuille Verrouiller ; le bouton n'existe qu'en `open`. */
  onLock?: (trigger: HTMLButtonElement) => void;
  /** Une commande est en vol : rien ne part en attendant. */
  busy?: boolean;
}

export function AvailabilityGrid({
  view,
  reference,
  now,
  fresh,
  onOpen,
  choice,
  onChoice,
  headingRef,
  onLock,
  busy = false,
}: Props) {
  const lang = useLang();
  const ids = useId();
  const wide = useMedia(MATRIX_MEDIA);
  const slots = view.event.slots;
  const tooMany = slots.length > MATRIX_MAX_SLOTS;
  const matrixPossible = !tooMany && wide;
  const { wantMatrix, filter, picked } = choice;
  const mode = matrixPossible && wantMatrix ? "matrix" : "slot";
  const setPicked = (id: string) => onChoice({ picked: id });
  const [showUnavailable, setShowUnavailable] = useState(false);

  const rowsFor = (f: GridFilter) => view.candidatures.filter((c) => inFilter(c, f));
  const [order, setOrder] = useState<{ filter: GridFilter; ids: string[] }>(() => ({
    filter,
    ids: sortRows(rowsFor(filter)),
  }));
  const visible = rowsFor(filter);
  // Relecture : l'ordre établi est gardé ; un autre filtre retrie (motif « informations du rendu
  // précédent » de React).
  const orderIds = order.filter === filter ? keepOrder(order.ids, visible) : sortRows(visible);
  if (order.filter !== filter || orderIds.length !== order.ids.length || orderIds.some((id, i) => id !== order.ids[i])) {
    setOrder({ filter, ids: orderIds });
  }
  const byId = new Map(visible.map((c) => [c.id, c]));
  const rows = orderIds.map((id) => byId.get(id)!);

  function pickFilter(f: GridFilter) {
    if (f !== filter) onChoice({ filter: f });
  }

  const counts: Record<GridFilter, number> = {
    all: rowsFor("all").length,
    accepted: rowsFor("accepted").length,
    pending: rowsFor("pending").length,
  };
  const nowMs = now();
  // Sans choix, ou créneau choisi disparu : le défaut, figé ensuite comme choix pour qu'il ne saute
  // pas quand le premier créneau à venir commence.
  const selectedId = picked !== null && slots.some((s) => s.id === picked) ? picked : defaultSlot(slots, nowMs);
  useEffect(() => {
    if (selectedId !== picked) onChoice({ picked: selectedId });
  }, [selectedId, picked, onChoice]);
  const selected = slots.find((s) => s.id === selectedId) ?? null;
  const stats = new Map(view.slots.map((s) => [s.slotId, s]));
  const statOf = (id: string): OrganizerSlot => stats.get(id) ?? { slotId: id, availableAccepted: 0, coverage: {}, hasGroups: false };
  const isPast = (s: PublicSlot) => Date.parse(s.startsAt) <= nowMs;

  const reasonId = `${ids}-reason`;
  const lockReasonId = `${ids}-lock-reason`;
  const canLockSomething = slots.some((s) => !isPast(s));
  const reason = [tooMany && t("grid.reason.slots"), !wide && t("grid.reason.narrow")].filter(Boolean).join(" · ");
  const segment = "min-h-tap flex-1 px-2 text-label";

  const coverage = selected && (
    <Coverage
      slot={selected}
      stat={statOf(selected.id)}
      bounds={view.classBounds}
      reference={reference}
      lang={lang}
    />
  );

  return (
    <div data-testid="availability" className="mt-4">
      <h2 ref={headingRef} tabIndex={-1} className="sr-only">
        {t("organizer.tab.availability")}
      </h2>
      <p className="text-meta text-ink-secondary">{t("event.timezone", { zone: timeZoneLabel(lang) })}</p>

      {onLock && view.event.status === "open" && (
        <div className="mt-3">
          <button
            type="button"
            data-testid="lock-open"
            aria-haspopup="dialog"
            disabled={!canLockSomething || busy}
            aria-describedby={canLockSomething ? undefined : lockReasonId}
            onClick={(e) => onLock(e.currentTarget)}
            className="min-h-tap w-full rounded-md bg-accent px-4 text-label text-on-accent disabled:bg-surface-sunken disabled:text-ink-disabled"
          >
            {t("lock.open")}
          </button>
          {!canLockSomething && (
            <p id={lockReasonId} className="mt-1 text-meta text-ink-secondary">
              {t("lock.noUpcoming")}
            </p>
          )}
        </div>
      )}

      <div role="group" aria-label={t("grid.view")} className="mt-3 flex overflow-hidden rounded-md border border-border">
        <button
          type="button"
          data-testid="view-matrix"
          aria-pressed={mode === "matrix"}
          aria-disabled={matrixPossible ? undefined : true}
          aria-describedby={matrixPossible ? undefined : reasonId}
          onClick={() => {
            if (matrixPossible) onChoice({ wantMatrix: true });
          }}
          className={`${segment} ${mode === "matrix" ? "bg-surface-raised text-ink-primary shadow-[inset_0_-2px_var(--accent)]" : matrixPossible ? "text-ink-secondary" : "text-ink-disabled"}`}
        >
          {t("grid.matrix")}
        </button>
        <button
          type="button"
          data-testid="view-slot"
          aria-pressed={mode === "slot"}
          onClick={() => {
            // Matrice impossible : rien à choisir, et le choix « Matrice » reste gardé.
            if (matrixPossible) onChoice({ wantMatrix: false });
          }}
          className={`${segment} ${mode === "slot" ? "bg-surface-raised text-ink-primary shadow-[inset_0_-2px_var(--accent)]" : "text-ink-secondary"}`}
        >
          {t("grid.bySlot")}
        </button>
      </div>
      {!matrixPossible && (
        <p id={reasonId} data-testid="view-reason" className="mt-1 text-meta text-ink-secondary">
          {reason}
        </p>
      )}

      <div role="group" aria-label={t("grid.filter")} className="mt-3 flex flex-wrap gap-2">
        {FILTERS.map((f) => (
          <button
            key={f}
            type="button"
            data-testid={`filter-${f}`}
            aria-pressed={filter === f}
            onClick={() => pickFilter(f)}
            className={`inline-flex min-h-tap items-center gap-1.5 rounded-full border-2 px-3 text-label ${filter === f ? "border-accent" : "border-border"}`}
          >
            {t(FILTER_KEY[f])}
            <span className="font-normal text-ink-secondary tabular-nums">{counts[f]}</span>
          </button>
        ))}
      </div>

      {rows.length === 0 && <p className="mt-3 text-body text-ink-secondary">{t("grid.empty")}</p>}

      {mode === "matrix" ? (
        <>
          <Matrix
            slots={slots}
            rows={rows}
            selectedId={selectedId}
            onSelect={setPicked}
            statOf={statOf}
            isPast={isPast}
            reference={reference}
            lang={lang}
            fresh={fresh}
            onOpen={onOpen}
          />
          {coverage && <div className="mt-3">{coverage}</div>}
        </>
      ) : (
        <div data-testid="by-slot" className="mt-3">
          <div role="group" aria-label={t("grid.slots")} className="flex flex-wrap gap-2">
            {slots.map((s) => {
              const on = s.id === selectedId;
              return (
                <button
                  key={s.id}
                  type="button"
                  data-testid={`chip-${s.id}`}
                  aria-pressed={on}
                  onClick={() => setPicked(s.id)}
                  className={`min-h-tap rounded-full border-2 bg-surface-raised px-3 text-label tabular-nums ${on ? "border-accent" : "border-border"} ${isPast(s) ? "text-ink-disabled" : "text-ink-primary"}`}
                >
                  {t("grid.chip", { slot: formatSlot(s.startsAt, lang), count: statOf(s.id).availableAccepted })}
                  {isPast(s) && <span className="sr-only">{`, ${t("grid.past")}`}</span>}
                </button>
              );
            })}
          </div>
          {coverage && <div className="mt-3">{coverage}</div>}
          {selected && (
            <SlotLists
              slot={selected}
              rows={rows}
              reference={reference}
              lang={lang}
              fresh={fresh}
              onOpen={onOpen}
              open={showUnavailable}
              onToggle={() => setShowUnavailable((v) => !v)}
              listId={`${ids}-unavailable`}
            />
          )}
        </div>
      )}
    </div>
  );
}

/** Classe (libellé de la référence, repli sur l'id) et CP d'une candidature. */
function classAndCp(c: OrganizerCandidature, reference: ReferenceSet | null, lang: string) {
  const gameClass = reference?.classes.find((x) => x.id === c.class);
  return {
    className: gameClass ? labelOf(gameClass.label, lang) : c.class,
    cp: new Intl.NumberFormat(lang).format(c.cp),
  };
}

const miniBadge = "shrink-0 rounded-full border px-1.5 text-meta leading-tight";

/** Badge de statut `pending` en version courte, pour tenir dans 136 px. */
function PendingBadge() {
  return (
    <span data-testid="pending-badge" className={`${miniBadge} ${STATUS_STYLE.pending}`}>
      {t("grid.pending")}
    </span>
  );
}

/** Badge « doublon » de la liste des candidatures, en version courte. */
function DuplicateMark() {
  return (
    <span data-testid="duplicate-badge" className={`${miniBadge} border-danger text-danger`}>
      {t("organizer.duplicate")}
    </span>
  );
}

/** Nom annoncé : `Pseudo[SRV]` quand la ligne a un doublon, sinon le pseudo. */
const spoken = (c: OrganizerCandidature) => (c.duplicate ? c.pseudoSrv : c.pseudo);

function Dot({ c, reference }: { c: OrganizerCandidature; reference: ReferenceSet | null }) {
  const server = reference?.servers.find((s) => s.id === c.server);
  return server ? <FactionDot faction={server.faction} /> : null;
}

interface MatrixProps {
  slots: PublicSlot[];
  rows: OrganizerCandidature[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  statOf: (id: string) => OrganizerSlot;
  isPast: (s: PublicSlot) => boolean;
  reference: ReferenceSet | null;
  lang: string;
  fresh: ReadonlySet<string>;
  onOpen: (id: string, trigger: HTMLButtonElement) => void;
}

/** Vrai `<table>` : colonne joueur de 136 px, puis une colonne de 44 px par créneau, séparées de
 * 4 px ; la marge négative absorbe l'espacement extérieur (136 + 4 × 44 + 4 × 4 = 328 px utiles). */
function Matrix({ slots, rows, selectedId, onSelect, statOf, isPast, reference, lang, fresh, onOpen }: MatrixProps) {
  const ring = "outline-2 -outline-offset-2 outline-accent";
  return (
    <table data-testid="matrix" className="-mx-1 mt-3 table-fixed border-separate border-spacing-1">
      <caption className="sr-only">{t("grid.caption")}</caption>
      <thead>
        <tr>
          <th scope="col" className="w-[136px] p-0 pl-1 text-left align-bottom text-meta font-normal text-ink-secondary">
            {t("grid.player")}
          </th>
          {slots.map((s) => {
            const on = s.id === selectedId;
            const { day, time } = slotParts(s.startsAt, lang);
            return (
              <th key={s.id} scope="col" className="w-tap p-0">
                <button
                  type="button"
                  data-testid={`col-${s.id}`}
                  aria-pressed={on}
                  onClick={() => onSelect(s.id)}
                  className={`flex min-h-tap w-tap flex-col items-center justify-center border-b-2 text-label leading-tight tabular-nums ${on ? "border-accent text-accent" : `border-transparent ${isPast(s) ? "text-ink-disabled" : "text-ink-secondary"}`}`}
                >
                  <span>{day}</span>
                  <span>{time}</span>
                  {isPast(s) && <span className="sr-only">{t("grid.past")}</span>}
                </button>
              </th>
            );
          })}
        </tr>
      </thead>
      <tbody>
        {rows.map((c) => {
          const { className, cp } = classAndCp(c, reference, lang);
          const isFresh = fresh.has(c.id);
          return (
            <tr key={c.id} data-testid={`grid-row-${c.id}`} data-fresh={isFresh ? "true" : undefined}>
              <th scope="row" className="w-[136px] p-0 text-left font-normal">
                <button
                  type="button"
                  aria-haspopup="dialog"
                  onClick={(e) => onOpen(c.id, e.currentTarget)}
                  className={`block min-h-tap w-[136px] border-l-[3px] pl-1 text-left text-ink-primary motion-safe:transition-colors motion-safe:duration-700 ${isFresh ? "border-l-accent" : "border-l-transparent"}`}
                >
                  {/* Badges trop larges : ils passent à la ligne plutôt que d'écraser le pseudo. */}
                  <span className="flex flex-wrap items-center gap-x-1">
                    <Dot c={c} reference={reference} />
                    <span className="min-w-0 max-w-full truncate text-label">{c.pseudo}</span>
                    {c.status === "pending" && <PendingBadge />}
                    {c.duplicate && <DuplicateMark />}
                  </span>
                  <span className="block truncate text-meta text-ink-secondary tabular-nums">
                    {srvOf(c)} · {className} · {cp}
                  </span>
                </button>
              </th>
              {slots.map((s) => {
                const a = c.availabilities.find((x) => x.slotId === s.id);
                const slot = formatSlot(s.startsAt, lang);
                let said = t(a ? "grid.cell.available" : "grid.cell.unavailable", { name: spoken(c), slot });
                if (a?.preferred) said = t("grid.cell.preferred", { cell: said });
                if (isPast(s)) said = t("grid.cell.past", { cell: said });
                return (
                  <td
                    key={s.id}
                    data-testid={`cell-${c.id}-${s.id}`}
                    data-available={a ? "true" : "false"}
                    className={`relative size-tap rounded-sm p-0 text-center align-middle font-bold ${a ? "bg-available text-surface-base" : "bg-surface-sunken text-ink-disabled"} ${s.id === selectedId ? ring : ""}`}
                  >
                    <span aria-hidden="true">{a ? "✓" : "–"}</span>
                    {a?.preferred && (
                      <span aria-hidden="true" className="absolute top-0.5 right-1 text-[10px] leading-none">
                        ☆
                      </span>
                    )}
                    <span className="sr-only">{said}</span>
                  </td>
                );
              })}
            </tr>
          );
        })}
      </tbody>
      <tfoot>
        <tr>
          <th scope="row" className="p-0 pl-1 text-left text-meta font-normal text-ink-secondary">
            {t("grid.footer")}
          </th>
          {slots.map((s) => (
            <td
              key={s.id}
              data-testid={`foot-${s.id}`}
              className={`text-center text-label tabular-nums ${s.id === selectedId ? "text-accent" : "text-ink-secondary"}`}
            >
              {statOf(s.id).availableAccepted}
            </td>
          ))}
        </tr>
      </tfoot>
    </table>
  );
}

interface CoverageProps {
  slot: PublicSlot;
  stat: OrganizerSlot;
  bounds: OrganizerEvent["classBounds"];
  reference: ReferenceSet | null;
  lang: string;
}

/** Couverture par classe du créneau sélectionné, parmi les `accepted`. */
function Coverage({ slot, stat, bounds, reference, lang }: CoverageProps) {
  const lines = coverageLines(stat.coverage, bounds, reference, lang);
  return (
    <section data-testid="coverage" className="rounded-md border border-border bg-surface-raised px-3 py-2">
      <h3 className="text-label">
        {t("grid.coverage", { slot: formatSlot(slot.startsAt, lang), count: stat.availableAccepted })}
      </h3>
      <ul className="mt-1">
        {lines.map((l) => (
          <li
            key={l.class}
            data-testid={`coverage-${l.class}`}
            data-under={l.under ? "true" : undefined}
            className={`flex justify-between gap-3 text-label font-normal tabular-nums ${l.under ? "text-danger" : ""}`}
          >
            <span className="min-w-0">{l.label}</span>
            <span className="shrink-0 whitespace-nowrap">{l.text}</span>
          </li>
        ))}
      </ul>
    </section>
  );
}

interface SlotListsProps {
  slot: PublicSlot;
  rows: OrganizerCandidature[];
  reference: ReferenceSet | null;
  lang: string;
  fresh: ReadonlySet<string>;
  onOpen: (id: string, trigger: HTMLButtonElement) => void;
  open: boolean;
  onToggle: () => void;
  listId: string;
}

/** Vue par créneau : les lignes disponibles au créneau, puis le repli « Pas dispo (N) ». */
function SlotLists({ slot, rows, reference, lang, fresh, onOpen, open, onToggle, listId }: SlotListsProps) {
  const at = (c: OrganizerCandidature) => c.availabilities.find((a) => a.slotId === slot.id);
  const available = rows.filter((c) => at(c));
  const unavailable = rows.filter((c) => !at(c));
  const item = (c: OrganizerCandidature) => {
    const { className } = classAndCp(c, reference, lang);
    const isFresh = fresh.has(c.id);
    const preferred = at(c)?.preferred ?? false;
    return (
      <li key={c.id}>
        <button
          type="button"
          data-testid={`slot-row-${c.id}`}
          data-fresh={isFresh ? "true" : undefined}
          aria-haspopup="dialog"
          onClick={(e) => onOpen(c.id, e.currentTarget)}
          className={`flex min-h-tap w-full items-center gap-3 border-b border-l-[3px] border-b-border py-2 pr-1 pl-2 text-left text-ink-primary motion-safe:transition-colors motion-safe:duration-700 ${isFresh ? "border-l-accent" : "border-l-transparent"}`}
        >
          <span className="min-w-0 flex-1">
            {/* À la loupe, le badge passe à la ligne plutôt que d'écraser le pseudo. */}
            <span className="flex flex-wrap items-center gap-x-1">
              <Dot c={c} reference={reference} />
              <span className="min-w-0 max-w-full truncate text-label">{c.pseudo}</span>
              {preferred && (
                <span className="shrink-0 text-accent">
                  <span aria-hidden="true">☆</span>
                  <span className="sr-only">{t("organizer.preferred")}</span>
                </span>
              )}
              {c.status === "pending" && <PendingBadge />}
              {c.duplicate && <DuplicateMark />}
            </span>
            <span className="mt-0.5 block truncate text-meta text-ink-secondary">
              {srvOf(c)} · {className}
            </span>
          </span>
          <span className="shrink-0 text-label text-ink-secondary tabular-nums">
            {new Intl.NumberFormat(lang).format(c.cp)}
          </span>
        </button>
      </li>
    );
  };
  return (
    <>
      {available.length === 0 && rows.length > 0 && (
        <p className="mt-3 text-meta text-ink-secondary">{t("grid.noneAvailable")}</p>
      )}
      <ul data-testid="available" className="mt-2">
        {available.map(item)}
      </ul>
      {unavailable.length > 0 && (
        <>
          <button
            type="button"
            data-testid="unavailable-toggle"
            aria-expanded={open}
            aria-controls={open ? listId : undefined}
            onClick={onToggle}
            className="mt-2 flex min-h-tap w-full items-center gap-2 text-left text-label text-ink-secondary"
          >
            <span aria-hidden="true">{open ? "▾" : "▸"}</span>
            {t("grid.unavailable", { count: unavailable.length })}
          </button>
          {open && (
            <ul id={listId} data-testid="unavailable">
              {unavailable.map(item)}
            </ul>
          )}
        </>
      )}
    </>
  );
}
