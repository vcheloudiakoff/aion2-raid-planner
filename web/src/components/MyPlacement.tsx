// Ma place sur `/p` (SPEC epic 3 CAP-6, EXPERIENCE.md *State Patterns*) : en `locked`, pour une
// candidature `accepted`, la ligne de placement (« Titulaire · Équipe 2, slot 4 · Soutien »,
// « Banc · G1 · jeu. 21:00 » ou « Non placé »), le créneau en heure locale, le nom de salle à copier
// et les coéquipiers que le serveur laisse voir (« Ton équipe » pour un titulaire, « Ton groupe »
// par équipe pour un joueur au banc, ou « Personne d'autre n'est placé pour l'instant. »). En `open`
// après un déverrouillage, pour une candidature acceptée, seulement « Le raid est
// revenu en préparation. ». Rien n'est calculé ici sur les autres joueurs : la liste vient telle
// quelle du serveur.

import { labelOf } from "../api/reference";
import type { ParticipantView, ReferenceSet, Teammate } from "../api/types";
import { t, useLang, type Lang } from "../i18n";
import { roleLabel } from "../lib/labels";
import { formatDay, formatSlot, slotParts } from "../lib/time";
import { CopyChip } from "./CopyChip";

/** La ligne de placement s'affiche : raid verrouillé et candidature acceptée. */
const shown = (view: ParticipantView) => view.event.status === "locked" && view.status === "accepted";

/** Clé de la ligne de placement, indépendante de la langue : elle change quand la place change.
 * `null` quand il n'y a pas de ligne. */
export function placementKey(view: ParticipantView): string | null {
  if (view.backInPreparation) return "back";
  if (!shown(view)) return null;
  const p = view.placement;
  if (!p) return "unplaced";
  const seat = p.seat.kind === "starter" ? `s${p.seat.party}.${p.seat.index}` : `b${p.seat.benchIndex}`;
  return `${p.groupNumber}|${p.slotId}|${seat}`;
}

/** Texte de la ligne de placement (ou du retour en préparation), `null` sans ligne. */
export function placementLine(view: ParticipantView, reference: ReferenceSet | null, lang: Lang): string | null {
  if (view.backInPreparation) return t("participant.backInPreparation");
  if (!shown(view)) return null;
  const p = view.placement;
  if (!p) return t("participant.placement.unplaced");
  if (p.seat.kind === "starter") {
    const role = reference?.classes.find((c) => c.id === view.class)?.role;
    return t("participant.placement.starter", {
      party: p.seat.party,
      index: p.seat.index,
      role: role ? roleLabel(reference, role, lang) : view.class,
    });
  }
  return t("participant.placement.bench", { n: p.groupNumber, slot: formatSlot(p.startsAt, lang) });
}

interface Props {
  view: ParticipantView;
  reference: ReferenceSet | null;
  now: () => number;
}

export function MyPlacement({ view, reference, now }: Props) {
  const lang = useLang();
  const line = placementLine(view, reference, lang);
  if (line === null) return null;
  if (view.backInPreparation) {
    return (
      <p data-testid="back-in-preparation" className="mt-4 rounded-md border border-border px-3 py-2 text-label">
        {line}
      </p>
    );
  }
  const p = view.placement;
  const classLabel = (id: string) => {
    const c = reference?.classes.find((x) => x.id === id);
    return c ? labelOf(c.label, lang) : id;
  };
  const parties = (view.teammates ?? []).filter((party) => party.members.length > 0);
  const bench = p?.seat.kind === "bench";
  const list = (items: Teammate[]) => (
    <ul className="mt-1 flex flex-col gap-1">
      {items.map((m, i) => (
        <li key={i} className="flex flex-wrap items-baseline gap-x-2">
          <span className="font-mono break-all">{m.pseudoSrv}</span>
          <span className="text-meta text-ink-secondary">{classLabel(m.class)}</span>
        </li>
      ))}
    </ul>
  );
  return (
    <section data-testid="placement" aria-labelledby="placement-title" className="mt-4">
      <h2 id="placement-title" className="sr-only">
        {t("participant.placement.title")}
      </h2>
      <p data-testid="placement-line" className="text-label tabular-nums">
        {line}
      </p>
      {p && (
        <>
          <p data-testid="placement-slot" className="mt-2 flex flex-wrap items-baseline gap-x-2 text-label tabular-nums">
            <span className="text-meta text-ink-secondary">{t("participant.placement.slot")}</span>
            <span>{`${formatDay(p.startsAt, lang, new Date(now()))} · ${slotParts(p.startsAt, lang).time}`}</span>
          </p>
          <div className="mt-2">
            <CopyChip value={p.roomName} label={t("participant.placement.room")} />
          </div>
          <section data-testid="teammates" aria-labelledby="teammates-title" className="mt-4">
            <h3 id="teammates-title" className="text-label">
              {t(bench ? "participant.placement.group" : "participant.placement.team")}
            </h3>
            {parties.length === 0 ? (
              <p data-testid="teammates-alone" className="mt-1 text-meta text-ink-secondary">
                {t("participant.placement.alone")}
              </p>
            ) : bench ? (
              parties.map(({ party, members }) => (
                <div key={party} data-testid={`teammates-party-${party}`} className="mt-2">
                  <h4 className="text-meta text-ink-secondary">{t("participant.placement.party", { n: party })}</h4>
                  {list(members)}
                </div>
              ))
            ) : (
              list(parties.flatMap((party) => party.members))
            )}
          </section>
        </>
      )}
    </section>
  );
}
