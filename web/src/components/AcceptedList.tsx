// Planning des acceptés sur `/p` (SPEC epic 3 CAP-6, élargi le 2026-11-09 : à régulariser dans
// BMAD). Pour une candidature `accepted`, en `open` comme en `locked`, « Acceptés (N) » : une ligne
// par accepté, dans l'ordre du serveur (création), avec l'icône et le libellé de sa classe,
// `Pseudo[SRV]` en `mono`, ses Équipes souhaitées, puis une colonne par créneau : ✓ disponible,
// ★ préféré. Le pied compte les disponibles par créneau. Repliable, ouvert à son apparition s'il y a
// moins de 15 acceptés. La ligne du lecteur (`acceptedRosterSelf`) porte « toi ».

import { useState } from "react";
import { labelOf } from "../api/reference";
import type { ParticipantView, PublicSlot, ReferenceSet, RosterEntry } from "../api/types";
import { t, useLang } from "../i18n";
import { formatSlot } from "../lib/time";
import { teamsText } from "./CandidatureRow";

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
  return (
    <Roster
      entries={view.acceptedRoster}
      self={view.acceptedRosterSelf}
      slots={view.event.slots}
      reference={reference}
    />
  );
}

interface RosterProps {
  entries: RosterEntry[];
  self: number | undefined;
  slots: PublicSlot[];
  reference: ReferenceSet | null;
}

/** Icône de la classe, lue dans la référence ; rien si elle n'en a pas. */
export function ClassIcon({ classId, reference }: { classId: string; reference: ReferenceSet | null }) {
  const icon = reference?.classes.find((c) => c.id === classId)?.icon;
  if (!icon) return null;
  return (
    <span aria-hidden="true" className="inline-flex size-6 shrink-0 items-center justify-center rounded-sm bg-surface-sunken text-[14px]">
      {icon}
    </span>
  );
}

function Roster({ entries, self, slots, reference }: RosterProps) {
  const lang = useLang();
  const [open, setOpen] = useState(() => entries.length < OPEN_BELOW);
  const classLabel = (id: string) => {
    const c = reference?.classes.find((x) => x.id === id);
    return c ? labelOf(c.label, lang) : id;
  };
  const cols = { gridTemplateColumns: `minmax(10rem, 1fr) repeat(${slots.length}, 3.5rem)` };
  const cell = "flex items-center justify-center text-label tabular-nums";
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
      <div className="mt-1 overflow-x-auto rounded-md border border-border">
        <div className="min-w-max">
          <div style={cols} className="grid border-b border-border bg-surface-sunken py-1 text-meta text-ink-secondary">
            <span className="px-2">{t("participant.accepted.player")}</span>
            {slots.map((s) => (
              <span key={s.id} className="px-1 text-center leading-tight">
                {formatSlot(s.startsAt, lang)}
              </span>
            ))}
          </div>
          <ul>
            {entries.map((m, i) => {
              const by = new Map((m.availabilities ?? []).map((a) => [a.slotId, a.preferred]));
              return (
                <li
                  key={i}
                  data-me={i === self ? "" : undefined}
                  style={cols}
                  className={`grid border-b border-border py-1 last:border-b-0 ${i === self ? "bg-accent/10" : ""}`}
                >
                  <span data-testid="roster-who" className="flex min-w-0 items-center gap-2 px-2">
                    <ClassIcon classId={m.class} reference={reference} />
                    <span className="min-w-0">
                      <span className="block font-mono break-all">{m.pseudoSrv}</span>
                      <span className="flex flex-wrap gap-x-2 text-meta text-ink-secondary">
                        <span>{classLabel(m.class)}</span>
                        {m.teams.length > 0 && <span data-testid="roster-teams">{teamsText(m)}</span>}
                        {i === self && <span className="text-accent">{t("participant.accepted.me")}</span>}
                      </span>
                    </span>
                  </span>
                  {slots.map((s) => {
                    const pref = by.get(s.id);
                    return (
                      <span
                        key={s.id}
                        data-testid={`roster-cell-${s.id}`}
                        className={`${cell} ${pref === true ? "text-accent" : pref === false ? "text-available" : "text-ink-disabled"}`}
                        aria-label={pref === undefined ? t("participant.accepted.no") : pref ? t("participant.accepted.preferred") : t("participant.accepted.yes")}
                      >
                        {pref === true ? "★" : pref === false ? "✓" : "–"}
                      </span>
                    );
                  })}
                </li>
              );
            })}
          </ul>
          <div style={cols} className="grid border-t border-border bg-surface-sunken py-1 text-meta text-ink-secondary">
            <span className="px-2">{t("participant.accepted.available")}</span>
            {slots.map((s) => (
              <span key={s.id} data-testid={`roster-count-${s.id}`} className={cell}>
                {entries.filter((m) => (m.availabilities ?? []).some((a) => a.slotId === s.id)).length}
              </span>
            ))}
          </div>
        </div>
      </div>
    </details>
  );
}
