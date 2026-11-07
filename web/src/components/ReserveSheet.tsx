// Réserve de l'onglet Composition (EXPERIENCE.md *Écran de composition*, *Réserve*) : la feuille
// ouverte par le toucher d'une cible libre (slot ou banc). Candidatures `accepted` seulement, en trois
// sections :
// 1. disponibles au créneau du groupe et placées dans aucun groupe retenu, la classe qui manque
//    d'abord (sous son minimum dans ce groupe), puis le CP décroissant, puis l'ordre de création ;
// 2. « Placées dans un autre groupe » : le toucher déplace la candidature ;
// 3. « Pas dispo à ce créneau », repliée par défaut : la placer est permis, avec un avertissement.
// Une candidature déjà dans ce groupe n'y figure pas (« Échanger » la déplace). Une candidature
// placée dans un groupe en sommeil porte « dans un groupe en sommeil » : la placer l'en retire. Le
// toucher place et ferme la feuille ; rien ne part au serveur.

import { useId, useRef, useState, type RefObject } from "react";
import { labelOf } from "../api/reference";
import type { OrganizerCandidature, OrganizerEvent, ReferenceSet } from "../api/types";
import { t, useLang } from "../i18n";
import { seatOf, type Placement } from "../lib/placement";
import { Sheet } from "./Sheet";

export interface ReserveSections {
  available: OrganizerCandidature[];
  /** Placées dans un autre groupe retenu, avec son id. */
  elsewhere: { candidature: OrganizerCandidature; groupId: string }[];
  unavailable: OrganizerCandidature[];
}

/** Les trois sections de la réserve pour une cible du groupe `groupId` (créneau `slotId`). */
export function reserveSections(
  view: OrganizerEvent,
  placement: Placement,
  groupId: string,
  slotId: string,
): ReserveSections {
  const accepted = view.candidatures.filter((c) => c.status === "accepted");
  const starters = Object.values(placement[groupId]?.starters ?? {});
  const classOf = new Map(view.candidatures.map((c) => [c.id, c.class]));
  const count = (cls: string) => starters.filter((id) => classOf.get(id) === cls).length;
  const missing = (cls: string) => {
    const bound = view.classBounds[cls];
    return bound !== undefined && count(cls) < bound.min;
  };
  const order = new Map(view.candidatures.map((c, i) => [c.id, i]));
  const rank = (a: OrganizerCandidature, b: OrganizerCandidature) =>
    Number(missing(b.class)) - Number(missing(a.class)) || b.cp - a.cp || order.get(a.id)! - order.get(b.id)!;
  const free = (c: OrganizerCandidature) => c.availabilities.some((a) => a.slotId === slotId);
  const out: ReserveSections = { available: [], elsewhere: [], unavailable: [] };
  for (const c of accepted) {
    const seat = seatOf(placement, c.id);
    if (seat?.groupId === groupId) continue;
    if (seat) out.elsewhere.push({ candidature: c, groupId: seat.groupId });
    else if (free(c)) out.available.push(c);
    else out.unavailable.push(c);
  }
  out.available.sort(rank);
  out.unavailable.sort(rank);
  out.elsewhere.sort((a, b) => order.get(a.candidature.id)! - order.get(b.candidature.id)!);
  return out;
}

interface Props {
  view: OrganizerEvent;
  reference: ReferenceSet | null;
  placement: Placement;
  groupId: string;
  slotId: string;
  /** « Équipe 1, slot 3 » ou « Banc ». */
  target: string;
  /** Numéro « G1 » d'un groupe retenu. */
  groupNumber: (groupId: string) => number;
  /** Une commande est en vol : aucun choix. */
  busy?: boolean;
  onPick: (candidatureId: string) => void;
  onClose: () => void;
  trigger: RefObject<HTMLElement | null>;
  fallbackFocus?: RefObject<HTMLElement | null>;
}

export function ReserveSheet({
  view,
  reference,
  placement,
  groupId,
  slotId,
  target,
  groupNumber,
  busy = false,
  onPick,
  onClose,
  trigger,
  fallbackFocus,
}: Props) {
  const lang = useLang();
  const ids = useId();
  const heading = useRef<HTMLHeadingElement>(null);
  const [showUnavailable, setShowUnavailable] = useState(false);
  const sections = reserveSections(view, placement, groupId, slotId);
  const asleep = new Set(
    view.groups.filter((g) => !g.retained).flatMap((g) => [...g.starters, ...g.bench].map((s) => s.candidatureId)),
  );
  const empty = sections.available.length + sections.elsewhere.length + sections.unavailable.length === 0;

  const sleepNote = (c: OrganizerCandidature) => (asleep.has(c.id) ? t("composition.reserve.sleeping") : undefined);
  const row = (c: OrganizerCandidature, note?: string) => {
    const cls = reference?.classes.find((x) => x.id === c.class);
    return (
      <li key={c.id}>
        <button
          type="button"
          data-testid={`reserve-${c.id}`}
          disabled={busy}
          onClick={() => onPick(c.id)}
          className="flex min-h-tap w-full items-center gap-3 border-b border-border py-2 text-left"
        >
          <span className="min-w-0 flex-1">
            <span className="block truncate font-mono text-[15px] leading-[1.4] font-medium text-ink-primary">
              {c.pseudoSrv}
            </span>
            <span className="block text-meta text-ink-secondary">
              {cls ? labelOf(cls.label, lang) : c.class}
              {note && ` · ${note}`}
            </span>
          </span>
          <span className="shrink-0 text-meta text-ink-secondary tabular-nums">{c.cp}</span>
        </button>
      </li>
    );
  };

  return (
    <Sheet labelledBy={`${ids}-title`} onClose={onClose} trigger={trigger} initialFocus={heading} fallbackFocus={fallbackFocus}>
      <div data-testid="reserve" className="max-h-[70vh] overflow-y-auto">
        <h2 id={`${ids}-title`} ref={heading} tabIndex={-1} className="text-title outline-none">
          {t("composition.reserve.title", { target })}
        </h2>
        {empty && <p className="mt-3 text-body text-ink-secondary">{t("composition.reserve.empty")}</p>}
        {sections.available.length > 0 && (
          <section data-testid="reserve-available" className="mt-3">
            <h3 className="text-label text-ink-secondary">{t("composition.reserve.available")}</h3>
            <ul>{sections.available.map((c) => row(c, sleepNote(c)))}</ul>
          </section>
        )}
        {sections.elsewhere.length > 0 && (
          <section data-testid="reserve-elsewhere" className="mt-3">
            <h3 className="text-label text-ink-secondary">{t("composition.reserve.elsewhere")}</h3>
            <ul>
              {sections.elsewhere.map(({ candidature, groupId: other }) =>
                row(candidature, t("composition.reserve.in", { n: groupNumber(other) })),
              )}
            </ul>
          </section>
        )}
        {sections.unavailable.length > 0 && (
          <section data-testid="reserve-unavailable" className="mt-3">
            <button
              type="button"
              data-testid="reserve-unavailable-toggle"
              aria-expanded={showUnavailable}
              aria-controls={`${ids}-unavailable`}
              onClick={() => setShowUnavailable((v) => !v)}
              className="flex min-h-tap w-full items-center gap-2 text-left text-label text-ink-secondary"
            >
              <span aria-hidden="true">{showUnavailable ? "▾" : "▸"}</span>
              {t("composition.reserve.unavailable", { count: sections.unavailable.length })}
            </button>
            {showUnavailable && <ul id={`${ids}-unavailable`}>{sections.unavailable.map((c) => row(c, sleepNote(c)))}</ul>}
          </section>
        )}
        <button
          type="button"
          onClick={onClose}
          className="mt-5 min-h-tap w-full rounded-md border border-border px-4 text-label text-ink-primary"
        >
          {t("composition.close")}
        </button>
      </div>
    </Sheet>
  );
}
