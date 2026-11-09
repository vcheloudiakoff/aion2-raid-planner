// Icône de classe : dessins originaux en SVG (trait `currentColor`), choisis par le champ `icon` de
// `classes.toml` et colorés selon le rôle de la classe. Une valeur inconnue des dessins s'affiche
// telle quelle (un emoji, par exemple) ; une classe sans `icon` n'a pas d'icône.

import type { ReactNode } from "react";
import type { ReferenceSet } from "../api/types";

/** Dessins disponibles pour `icon`, sur une grille 24 × 24. */
export const GLYPHS: Record<string, ReactNode> = {
  sword: (
    <>
      <path d="M20 4v4L10 18l-4-4L16 4z" />
      <path d="M5 13l6 6" />
      <path d="M7.5 16.5L4 20" />
    </>
  ),
  shield: (
    <>
      <path d="M12 3l7 3v5c0 5-3.5 8.5-7 10-3.5-1.5-7-5-7-10V6z" />
      <path d="M12 7v10M8.5 11h7" />
    </>
  ),
  daggers: (
    <>
      <path d="M12 2l2.5 3.5V14h-5V5.5z" />
      <path d="M7 14h10" />
      <path d="M12 14v4.5" />
      <circle cx="12" cy="20.5" r="1.5" />
    </>
  ),
  bow: (
    <>
      <path d="M7 3c8 3 8 15 0 18" />
      <path d="M7 3v18" />
      <path d="M3 12h17M17 9l3 3-3 3" />
    </>
  ),
  flame: (
    <path d="M12 3c3 4 6 6 6 11a6 6 0 0 1-12 0c0-3 2-5 3-6 0 3 1 4 2 4 0-3 0-6 1-9z" />
  ),
  spirit: (
    <>
      <circle cx="12" cy="12" r="3" />
      <path d="M12 3a9 9 0 1 1-8.5 6" />
      <circle cx="4.5" cy="6.5" r="1.2" />
    </>
  ),
  cross: <path d="M10 4h4v6h6v4h-6v6h-4v-6H4v-4h6z" />,
  note: (
    <>
      <path d="M9 18V6l10-2v12" />
      <circle cx="6.5" cy="18" r="2.5" />
      <circle cx="16.5" cy="16" r="2.5" />
    </>
  ),
};

/** Couleur par rôle ; un rôle inconnu reste en encre secondaire. */
const ROLE_COLOR: Record<string, string> = {
  tank: "text-pending",
  healer: "text-available",
  dps: "text-danger",
  support: "text-accent",
};

export function ClassIcon({ classId, reference }: { classId: string; reference: ReferenceSet | null }) {
  const c = reference?.classes.find((x) => x.id === classId);
  if (!c?.icon) return null;
  const glyph = GLYPHS[c.icon];
  return (
    <span
      aria-hidden="true"
      data-testid="class-icon"
      className={`inline-flex size-7 shrink-0 items-center justify-center rounded-md bg-surface-sunken ${ROLE_COLOR[c.role] ?? "text-ink-secondary"}`}
    >
      {glyph ? (
        <svg
          viewBox="0 0 24 24"
          className="size-[18px]"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
        >
          {glyph}
        </svg>
      ) : (
        <span className="text-[14px]">{c.icon}</span>
      )}
    </span>
  );
}
