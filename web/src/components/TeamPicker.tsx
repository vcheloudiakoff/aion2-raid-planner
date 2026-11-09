// Choix des Équipes (décision de Vincent du 2026-11-09, à régulariser dans BMAD) : une bascule ✓
// par Équipe de l'instance, rien de coché = peu importe, et ☆ sur la préférée quand au moins deux
// sont cochées. Partagé par le formulaire de candidature et la section « Mes Équipes » de `/p`.

import { t, useLang } from "../i18n";

export interface TeamValue {
  teams: number[];
  preferredTeam?: number;
}

/** Bascule l'Équipe `n` ; l'étoile tombe si elle n'a plus de sens. */
export function toggleTeam(v: TeamValue, n: number): TeamValue {
  const teams = v.teams.includes(n) ? v.teams.filter((x) => x !== n) : [...v.teams, n].sort((a, b) => a - b);
  const keep = teams.length > 1 && v.preferredTeam !== undefined && teams.includes(v.preferredTeam);
  return { teams, ...(keep && { preferredTeam: v.preferredTeam }) };
}

interface Props {
  parties: number;
  value: TeamValue;
  onChange: (v: TeamValue) => void;
  disabled?: boolean;
}

export function TeamPicker({ parties, value, onChange, disabled = false }: Props) {
  useLang();
  return (
    <ul className="mt-2 flex flex-col gap-2">
      {Array.from({ length: parties }, (_, i) => i + 1).map((n) => {
        const on = value.teams.includes(n);
        const star = value.preferredTeam === n;
        const name = t("event.team", { n });
        return (
          <li key={n} data-testid={`team-${n}`} className="flex items-center gap-2">
            <button
              type="button"
              aria-pressed={on}
              disabled={disabled}
              onClick={() => onChange(toggleTeam(value, n))}
              className={`flex min-h-tap min-w-0 flex-1 items-center gap-3 rounded-md border px-3 text-left text-label ${
                on ? "border-2 border-available text-ink-primary" : "border-border text-ink-secondary"
              }`}
            >
              <span
                aria-hidden="true"
                className={`flex size-6 shrink-0 items-center justify-center rounded-sm ${
                  on ? "bg-available text-on-accent" : "bg-surface-sunken"
                }`}
              >
                {on ? "✓" : "–"}
              </span>
              <span>{name}</span>
            </button>
            {on && value.teams.length > 1 && (
              <button
                type="button"
                aria-pressed={star}
                disabled={disabled}
                aria-label={t("event.team.preferred", { team: name })}
                onClick={() =>
                  onChange(star ? { teams: value.teams } : { teams: value.teams, preferredTeam: n })
                }
                className={`flex size-tap shrink-0 items-center justify-center rounded-md border text-title ${
                  star ? "border-accent text-accent" : "border-border text-ink-secondary"
                }`}
              >
                {star ? "★" : "☆"}
              </button>
            )}
          </li>
        );
      })}
    </ul>
  );
}
