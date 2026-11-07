// Bornes par classe (EXPERIENCE.md *Component Patterns*, Stepper), partagées par la création et les
// réglages : une ligne par classe, minimum et maximum en Stepper, `0 ≤ min ≤ max ≤ cap`. Une erreur
// du serveur sur `classBounds.<id>` ou sous lui s'affiche sous sa classe et s'efface quand la ligne
// change. Les lignes et leurs libellés (référence, repli sur l'id) viennent de l'appelant.

import type { Bound, FieldErrors } from "../lib/form";
import { t, useLang } from "../i18n";
import { Stepper } from "./Stepper";

export interface BoundRow {
  id: string;
  name: string;
  bound: Bound;
}

interface Props {
  rows: BoundRow[];
  /** Joueurs que prend l'instance : plafond du maximum. */
  cap: number;
  errors: FieldErrors;
  onChange: (id: string, bound: Bound) => void;
}

export function BoundsEditor({ rows, cap, errors, onChange }: Props) {
  useLang();
  return (
    <div>
      <p className="block text-label">{t("editor.bounds")}</p>
      <ul className="mt-2 flex flex-col gap-3">
        {rows.map(({ id, name, bound: b }) => {
          const path = `classBounds.${id}`;
          const bad = errors.hasNested(path);
          const change = (next: Bound) => {
            errors.clearField(path);
            onChange(id, next);
          };
          return (
            <li key={id} data-testid={`bound-${id}`}>
              <p className="text-body">{name}</p>
              <div className="mt-1 flex flex-wrap items-center gap-x-4 gap-y-2">
                <span className="flex items-center gap-2">
                  <span className="w-7 text-meta text-ink-secondary">{t("editor.bound.min")}</span>
                  <Stepper
                    value={b.min}
                    min={0}
                    max={b.max}
                    label={t("editor.bound.minOf", { class: name })}
                    invalid={bad}
                    describedBy={bad ? errors.errorId(path) : undefined}
                    onChange={(min) => change({ ...b, min })}
                  />
                </span>
                <span className="flex items-center gap-2">
                  <span className="w-7 text-meta text-ink-secondary">{t("editor.bound.max")}</span>
                  <Stepper
                    value={b.max}
                    min={b.min}
                    max={Math.max(cap, b.min)}
                    label={t("editor.bound.maxOf", { class: name })}
                    invalid={bad}
                    describedBy={bad ? errors.errorId(path) : undefined}
                    onChange={(max) => change({ ...b, max })}
                  />
                </span>
              </div>
              {errors.errorLine(path, true)}
            </li>
          );
        })}
      </ul>
    </div>
  );
}
