// Rôles demandés par Équipe (décision de Vincent du 2026-11-09, à régulariser dans BMAD) : pour
// chaque Équipe de l'instance, un Stepper par rôle, la somme bornée par la taille d'Équipe. Un
// signal pour l'organisateur, jamais un refus de placement ni de candidature.

import type { FieldErrors } from "../lib/form";
import { t, useLang } from "../i18n";
import { Stepper } from "./Stepper";

interface Props {
  parties: number;
  partySize: number;
  roles: { id: string; name: string }[];
  value: Record<string, number>[];
  errors: FieldErrors;
  onChange: (value: Record<string, number>[]) => void;
}

export function TeamRolesEditor({ parties, partySize, roles, value, errors, onChange }: Props) {
  useLang();
  const teams = Array.from({ length: parties }, (_, i) => value[i] ?? {});
  return (
    <div>
      <p className="block text-label">{t("editor.teamRoles")}</p>
      <p className="text-meta text-ink-secondary">{t("editor.teamRoles.hint")}</p>
      <ul className="mt-2 flex flex-col gap-4">
        {teams.map((team, i) => {
          const total = Object.values(team).reduce((a, n) => a + n, 0);
          const path = `teamRoles.${i}`;
          const teamName = t("event.team", { n: i + 1 });
          return (
            <li key={i} data-testid={`team-roles-${i + 1}`}>
              <p className="text-body">
                {teamName} <span className="text-meta text-ink-secondary">{`${total} / ${partySize}`}</span>
              </p>
              <div className="mt-1 flex flex-col gap-2">
                {roles.map((r) => {
                  const n = team[r.id] ?? 0;
                  return (
                    <span key={r.id} className="flex items-center gap-2">
                      <span className="w-20 text-meta text-ink-secondary">{r.name}</span>
                      <Stepper
                        value={n}
                        min={0}
                        max={partySize - total + n}
                        label={t("editor.teamRoles.of", { role: r.name, team: teamName })}
                        onChange={(next) => {
                          errors.clearField("teamRoles");
                          const all = teams.map((x) => ({ ...x }));
                          all[i][r.id] = next;
                          onChange(all);
                        }}
                      />
                    </span>
                  );
                })}
              </div>
              {errors.errorLine(path, true)}
            </li>
          );
        })}
      </ul>
      {errors.errorLine("teamRoles")}
    </div>
  );
}
