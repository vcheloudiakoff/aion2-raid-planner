// `/p/<secret>` provisoire, remplacé par la vue participant en 2-3 (EXPERIENCE.md *Liens et
// appareils*) : « Ton lien personnel » en tête, la puce du lien complet, le partage et
// l'avertissement, puis le statut quand la candidature vient d'être envoyée. Aucune requête ; le
// secret n'apparaît que dans la puce, jamais dans `document.title`.

import { useEffect, useRef } from "react";
import { useLocation, useParams } from "react-router";
import type { CreatedCandidature } from "../api/types";
import { CopyChip } from "../components/CopyChip";
import { t, useLang } from "../i18n";

/** État de navigation posé par le formulaire de candidature après le succès. */
export interface ParticipantState {
  created: CreatedCandidature;
  /** Faux si `saveParticipantLink` n'a pas pu écrire. */
  stored: boolean;
}

function isState(v: unknown): v is ParticipantState {
  if (typeof v !== "object" || v === null) return false;
  const s = v as Record<string, unknown>;
  return typeof s.stored === "boolean" && typeof s.created === "object" && s.created !== null;
}

interface Props {
  origin?: string;
}

export function ParticipantPending({ origin = window.location.origin }: Props) {
  useLang();
  const { secret = "" } = useParams();
  const location = useLocation();
  const state = isState(location.state) ? location.state : null;
  const heading = useRef<HTMLHeadingElement>(null);
  const url = `${origin}/p/${secret}`;
  const canShare = typeof navigator.share === "function";

  useEffect(() => {
    heading.current?.focus();
  }, []);

  async function share() {
    try {
      await navigator.share({ url });
    } catch {
      // Partage annulé ou refusé : rien à faire.
    }
  }

  return (
    <main className="mx-auto max-w-content px-3 py-6 min-[360px]:px-4">
      <h1 ref={heading} tabIndex={-1} className="text-title outline-none">
        {t("participant.title")}
      </h1>
      {state && !state.stored && (
        <p role="alert" className="mt-4 rounded-md border border-danger px-3 py-2 text-label text-danger">
          {t("participant.noStorage")}
        </p>
      )}
      <div className="mt-4">
        <CopyChip value={url} label={t("participant.link")} />
      </div>
      {canShare && (
        <button
          type="button"
          onClick={share}
          className="mt-2 min-h-tap rounded-md border border-border px-4 text-label text-ink-primary"
        >
          {t("participant.share")}
        </button>
      )}
      <p className="mt-2 text-meta text-ink-secondary">{t("participant.warning")}</p>
      {state && (state.created.status === "pending" || state.created.status === "accepted") && (
        <p data-testid="status" className="mt-6 text-label">
          {t("participant.status", {
            status: t(state.created.status === "pending" ? "participant.status.pending" : "participant.status.accepted"),
          })}
        </p>
      )}
    </main>
  );
}
