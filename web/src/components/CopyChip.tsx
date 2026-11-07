// Puce à copier (EXPERIENCE.md, Component Patterns) : toute la puce est la cible. Un toucher copie
// et affiche « Copié » 1,5 s ; si l'API presse-papiers manque ou échoue, le texte est sélectionné pour
// une copie à la main. Aucun effet de bord serveur. Le texte n'est jamais tronqué au milieu.
// Variante `display` : le nom de salle de l'onglet Salle, seul très gros texte de l'outil (DESIGN.md
// *Typography*) ; sinon `mono`.

import { useEffect, useRef, useState } from "react";
import { t, useLang } from "../i18n";

export const TOAST_MS = 1500;

interface Props {
  value: string;
  /** Nom de ce qu'on copie, pour le lecteur d'écran. */
  label: string;
  /** Appelé après une copie réussie seulement. */
  onCopied?: () => void;
  /** Typographie du texte copié : `mono` par défaut, `display` pour le nom de salle. */
  variant?: "mono" | "display";
}

const TEXT_CLASS = {
  mono: "font-mono text-[15px] leading-[1.4] font-medium",
  display: "text-display",
} as const;

export function CopyChip({ value, label, onCopied, variant = "mono" }: Props) {
  useLang();
  const [toast, setToast] = useState(0);
  const text = useRef<HTMLSpanElement>(null);

  useEffect(() => {
    if (!toast) return;
    const id = setTimeout(() => setToast(0), TOAST_MS);
    return () => clearTimeout(id);
  }, [toast]);

  async function copy() {
    try {
      await navigator.clipboard.writeText(value);
    } catch {
      const node = text.current;
      if (node) window.getSelection()?.selectAllChildren(node);
      return;
    }
    // Un nouveau toucher relance les 1,5 s.
    setToast((n) => n + 1);
    onCopied?.();
  }

  return (
    <>
      <button
        type="button"
        onClick={copy}
        aria-label={t("copy.action", { label })}
        data-variant={variant}
        className={`flex min-h-tap w-full items-center gap-2 rounded-sm bg-surface-sunken px-3 py-2 text-left text-ink-primary ${TEXT_CLASS[variant]}`}
      >
        <span ref={text} className="min-w-0 flex-1 break-all select-all">
          {value}
        </span>
        <svg aria-hidden="true" viewBox="0 0 24 24" className="size-5 shrink-0 fill-none stroke-ink-secondary stroke-2">
          <rect x="8" y="8" width="12" height="12" rx="2" />
          <path d="M16 8V6a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h2" />
        </svg>
      </button>
      {toast > 0 && <CopiedToast />}
    </>
  );
}

/** Toast « Copié », en bas au-dessus du bouton principal ; l'appelant le retire après `TOAST_MS`. */
export function CopiedToast() {
  return (
    <p
      role="status"
      className="fixed bottom-24 left-1/2 z-40 -translate-x-1/2 rounded-md border border-border bg-surface-raised px-4 py-2 text-label text-ink-primary shadow-[0_8px_24px_rgb(0_0_0/0.35)]"
    >
      {t("copy.done")}
    </p>
  );
}
