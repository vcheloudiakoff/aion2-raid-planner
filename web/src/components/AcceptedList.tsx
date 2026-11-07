// Liste des acceptés sur `/p` (SPEC epic 3 CAP-6, AD-4 *Données exposées*) : pour une candidature
// `accepted`, en `open` comme en `locked`, « Acceptés (N) » avec `Pseudo[SRV]` en `mono` et le
// libellé de classe de chaque accepté, dans l'ordre du serveur (création). Repliable, ouverte à son
// apparition s'il y a moins de 15 acceptés. La ligne du lecteur, au rang que donne le serveur
// (`acceptedRosterSelf`), porte « toi ». Rien n'est calculé ici sur les autres joueurs : la liste
// vient telle quelle du serveur.

import { useState } from "react";
import { labelOf } from "../api/reference";
import type { ParticipantView, ReferenceSet, RosterEntry } from "../api/types";
import { t, useLang } from "../i18n";

/** En dessous de ce nombre d'acceptés, la section s'ouvre d'elle-même. */
export const OPEN_BELOW = 15;

interface Props {
  view: ParticipantView;
  reference: ReferenceSet | null;
}

/** Rien sans liste ; la section n'est montée qu'à l'apparition de la liste, pour que son état
 * replié par défaut se décide sur le premier compte reçu. */
export function AcceptedList({ view, reference }: Props) {
  if (!view.acceptedRoster) return null;
  return <Roster entries={view.acceptedRoster} self={view.acceptedRosterSelf} reference={reference} />;
}

interface RosterProps {
  entries: RosterEntry[];
  self: number | undefined;
  reference: ReferenceSet | null;
}

function Roster({ entries, self, reference }: RosterProps) {
  const lang = useLang();
  const [open, setOpen] = useState(() => entries.length < OPEN_BELOW);
  const classLabel = (id: string) => {
    const c = reference?.classes.find((x) => x.id === id);
    return c ? labelOf(c.label, lang) : id;
  };
  return (
    <details
      data-testid="accepted"
      open={open}
      onToggle={(e) => setOpen((e.currentTarget as HTMLDetailsElement).open)}
      className="mt-4"
    >
      <summary className="flex min-h-tap cursor-pointer items-center text-label">
        {t("participant.accepted.title", { n: entries.length })}
      </summary>
      <ul className="mt-1 flex flex-col gap-1">
        {entries.map((m, i) => (
          <li key={i} data-me={i === self ? "" : undefined} className="flex flex-wrap items-baseline gap-x-2">
            <span className="font-mono break-all">{m.pseudoSrv}</span>
            <span className="text-meta text-ink-secondary">{classLabel(m.class)}</span>
            {i === self && <span className="text-meta text-accent">{t("participant.accepted.me")}</span>}
          </li>
        ))}
      </ul>
    </details>
  );
}
