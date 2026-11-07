// Stepper (EXPERIENCE.md, Component Patterns) : boutons − / + de 44 px et valeur au centre, saisissable.
// La valeur saisie n'est remontée que si c'est un entier dans [min, max] ; sinon elle revient à la
// dernière valeur valide quand le champ perd le focus.

import { useState } from "react";
import { t, useLang } from "../i18n";

interface Props {
  value: number;
  min: number;
  max: number;
  onChange: (value: number) => void;
  /** Nom complet du réglage (« Minimum Clerc »), pour le lecteur d'écran. */
  label: string;
  invalid?: boolean;
  describedBy?: string;
}

export function Stepper({ value, min, max, onChange, label, invalid, describedBy }: Props) {
  useLang();
  const [draft, setDraft] = useState<string | null>(null);
  const set = (n: number) => {
    setDraft(null);
    onChange(Math.min(max, Math.max(min, n)));
  };
  const button = "flex size-tap shrink-0 items-center justify-center rounded-md border border-border text-title text-ink-primary disabled:text-ink-disabled";
  return (
    <div role="group" aria-label={label} className="flex items-center gap-1">
      <button
        type="button"
        aria-label={t("stepper.decrease", { label })}
        disabled={value <= min}
        onClick={() => set(value - 1)}
        className={button}
      >
        −
      </button>
      <input
        type="number"
        inputMode="numeric"
        min={min}
        max={max}
        aria-label={label}
        aria-invalid={invalid || undefined}
        aria-describedby={describedBy}
        value={draft ?? String(value)}
        onChange={(e) => {
          const text = e.target.value;
          setDraft(text);
          const n = Number(text);
          if (text !== "" && Number.isInteger(n) && n >= min && n <= max) onChange(n);
        }}
        onBlur={() => setDraft(null)}
        className="h-tap w-11 min-w-0 rounded-md border border-border bg-surface-sunken text-center text-body tabular-nums text-ink-primary"
      />
      <button
        type="button"
        aria-label={t("stepper.increase", { label })}
        disabled={value >= max}
        onClick={() => set(value + 1)}
        className={button}
      >
        +
      </button>
    </div>
  );
}
