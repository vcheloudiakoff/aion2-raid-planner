// Slot de salle de l'onglet Composition (DESIGN.md `room-slot`, EXPERIENCE.md *Écran de
// composition*) : un bouton d'au moins 52 px, le numéro, puis le rôle et la classe du titulaire en
// `meta` et `Pseudo[SRV]` en `mono`, ou « libre » en `ink-disabled`. Avertissements en ligne, jamais
// par la couleur seule : « pas dispo à ce créneau » sous le nom, et « 3 / max 2 » sur la ligne de
// classe ; en brouillon, « plus acceptée » ou « absente » ; un refus du serveur sur cette place.
// Cible active (réserve ouverte, source d'un échange) : bordure or pointillée. Nom accessible
// explicite (« Équipe 1, slot 3, libre »), source d'un échange en `aria-pressed`.

import type { OrganizerCandidature } from "../api/types";
import { t } from "../i18n";

export interface SlotPerson {
  candidature: OrganizerCandidature | null;
  /** Repli quand la candidature n'est plus dans la vue. */
  id: string;
  role: string;
  className: string;
  unavailable: boolean;
  /** « 3 / max 2 » si la classe sort de ses bornes dans ce groupe. */
  classWarning: string | null;
  /** En brouillon : « plus acceptée » ou « absente », sinon `null`. */
  gone: string | null;
}

interface Props {
  /** Numéro affiché : slot dans l'équipe, ou rang au banc. */
  n: number;
  person: SlotPerson | null;
  /** Nom accessible : « Équipe 1, slot 3, libre », avertissements compris. */
  label: string;
  /** Refus du serveur sur cette place, déjà traduit. */
  error?: string | null;
  active: boolean;
  /** Un échange attend sa cible : pas de feuille au toucher. */
  swapping?: boolean;
  /** Source de l'échange en cours. */
  swapSource?: boolean;
  testId: string;
  onTap: (trigger: HTMLButtonElement) => void;
  disabled?: boolean;
}

export function RoomSlot({
  n,
  person,
  label,
  error = null,
  active,
  swapping = false,
  swapSource = false,
  testId,
  onTap,
  disabled = false,
}: Props) {
  return (
    <button
      type="button"
      data-testid={testId}
      data-active={active ? "true" : undefined}
      aria-label={label}
      aria-pressed={swapSource ? true : undefined}
      aria-haspopup={swapping ? undefined : "dialog"}
      disabled={disabled}
      onClick={(e) => onTap(e.currentTarget)}
      className={`flex min-h-[52px] w-full items-center gap-3 rounded-md border bg-surface-raised px-3 py-1 text-left ${active ? "border-2 border-dashed border-accent" : error ? "border-danger" : "border-border"}`}
    >
      <span className="w-5 shrink-0 text-label tabular-nums text-ink-secondary">
        {n}
      </span>
      {person ? (
        <span className="min-w-0 flex-1">
          <span data-testid="slot-class" className="block text-meta text-ink-secondary [overflow-wrap:anywhere]">
            {person.role} · {person.className}
            {person.classWarning && <span className="text-danger"> · {person.classWarning}</span>}
          </span>
          <span className="block truncate font-mono text-[15px] leading-[1.4] font-medium text-ink-primary">
            {person.candidature?.pseudoSrv ?? person.id}
          </span>
          {person.gone && (
            <span data-testid="slot-gone" className="block text-meta text-danger">
              {person.gone}
            </span>
          )}
          {person.unavailable && (
            <span data-testid="slot-unavailable" className="block text-meta text-danger">
              {t("composition.unavailable")}
            </span>
          )}
          {error && (
            <span data-testid="slot-error" className="block text-meta text-danger">
              {error}
            </span>
          )}
        </span>
      ) : (
        <span className="min-w-0 flex-1">
          <span className="block text-label text-ink-disabled">{t("composition.free")}</span>
          {error && (
            <span data-testid="slot-error" className="block text-meta text-danger">
              {error}
            </span>
          )}
        </span>
      )}
    </button>
  );
}
