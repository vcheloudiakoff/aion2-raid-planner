// Ligne de candidature vue par l'organisateur (DESIGN.md *Components*, EXPERIENCE.md *Component
// Patterns*) : un bouton qui ouvre la feuille de la candidature, jamais une action par balayage.
// Pseudo, `[SRV]` avec le point de faction, classe, CP aligné à droite en chiffres tabulaires ; badge
// de statut coloré pour chaque statut, badge « doublon » si elle est signalée. Une ligne
// nouvelle porte un liseré or ; il s'efface en transition, sauf sous `prefers-reduced-motion`.

import { labelOf } from "../api/reference";
import type { CandidatureStatus, OrganizerCandidature, ReferenceSet } from "../api/types";
import { t, useLang, type MessageKey } from "../i18n";
import { FactionDot } from "./EventSummary";

const STATUS_KEY: Record<CandidatureStatus, MessageKey> = {
  pending: "organizer.status.pending",
  accepted: "organizer.status.accepted",
  rejected: "organizer.status.rejected",
  withdrawn: "organizer.status.withdrawn",
};

/** Badge de statut, couleur doublée du libellé (jamais la couleur seule) : `accepted` en vert,
 * `pending` en bleu, `rejected` en rouge, `withdrawn` barré en `ink-disabled`. Partagé avec `/p`. */
export const STATUS_STYLE: Record<CandidatureStatus, string> = {
  pending: "border-pending bg-pending/10 text-pending",
  accepted: "border-available bg-available/10 text-available",
  rejected: "border-danger bg-danger/10 text-danger",
  withdrawn: "border-border text-ink-disabled line-through",
};

const badge = "inline-block rounded-full border px-2 py-0.5 text-meta";

export function StatusBadge({ status }: { status: CandidatureStatus }) {
  useLang();
  return (
    <span data-testid="status-badge" data-status={status} className={`${badge} ${STATUS_STYLE[status]}`}>
      {t(STATUS_KEY[status])}
    </span>
  );
}

export function DuplicateBadge() {
  useLang();
  return (
    <span data-testid="duplicate-badge" className={`${badge} border-danger text-danger`}>
      {t("organizer.duplicate")}
    </span>
  );
}

/** `[SRV]` lu dans `pseudoSrv`, composé par le serveur : le client ne recompose jamais `Pseudo[SRV]`
 * (AD-6). */
export function srvOf(c: OrganizerCandidature): string {
  return c.pseudoSrv.startsWith(c.pseudo) ? c.pseudoSrv.slice(c.pseudo.length) : c.pseudoSrv;
}

interface Props {
  candidature: OrganizerCandidature;
  reference: ReferenceSet | null;
  /** Ligne apparue au dernier rafraîchissement : liseré or. */
  fresh: boolean;
  onOpen: (id: string, trigger: HTMLButtonElement) => void;
}

export function CandidatureRow({ candidature: c, reference, fresh, onOpen }: Props) {
  const lang = useLang();
  const server = reference?.servers.find((s) => s.id === c.server);
  const gameClass = reference?.classes.find((x) => x.id === c.class);
  return (
    <li>
      <button
        type="button"
        data-testid={`row-${c.id}`}
        data-fresh={fresh ? "true" : undefined}
        aria-haspopup="dialog"
        onClick={(e) => onOpen(c.id, e.currentTarget)}
        className={`flex min-h-tap w-full items-center gap-3 border-b border-l-[3px] border-b-border py-2 pr-1 pl-2 text-left text-ink-primary motion-safe:transition-colors motion-safe:duration-700 ${fresh ? "border-l-accent" : "border-l-transparent"}`}
      >
        <span className="min-w-0 flex-1">
          <span className="block truncate text-label">{c.pseudo}</span>
          <span className="mt-0.5 flex flex-wrap items-center gap-x-2 text-meta text-ink-secondary">
            <span className="inline-flex items-center gap-1">
              {server && reference && (
                <>
                  <FactionDot faction={server.faction} />
                  <span data-testid="faction" className="sr-only">
                    {labelOf(reference.factions[server.faction], lang)}
                  </span>
                </>
              )}
              {srvOf(c)}
            </span>
            <span>{gameClass ? labelOf(gameClass.label, lang) : c.class}</span>
          </span>
        </span>
        <span className="flex shrink-0 flex-col items-end gap-1">
          <span className="text-label tabular-nums">{new Intl.NumberFormat(lang).format(c.cp)}</span>
          <span className="flex flex-wrap justify-end gap-1">
            {c.duplicate && <DuplicateBadge />}
            <StatusBadge status={c.status} />
          </span>
        </span>
      </button>
    </li>
  );
}
