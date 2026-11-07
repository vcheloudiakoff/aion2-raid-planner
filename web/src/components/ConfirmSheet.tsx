// Confirmation destructive (EXPERIENCE.md *Component Patterns*) : une feuille du bas qui nomme la
// conséquence, un bouton danger qui reprend le verbe et « Annuler ». Le focus va d'abord sur
// « Annuler » : la touche Entrée par réflexe ne détruit rien. Échap, le fond et le glissement
// ferment comme « Annuler », sauf pendant l'envoi.

import { useId, useRef, type RefObject } from "react";
import { Sheet } from "./Sheet";

export interface ConfirmSheetProps {
  /** La conséquence, en une phrase : elle nomme aussi le dialogue. */
  message: string;
  confirmLabel: string;
  cancelLabel: string;
  /** Libellé pendant l'envoi. */
  busyLabel: string;
  busy?: boolean;
  /** Erreur du dernier essai, déjà traduite. */
  error?: string | null;
  onConfirm: () => void;
  onClose: () => void;
  trigger: RefObject<HTMLElement | null>;
  /** Repli du focus si le déclencheur a disparu à la fermeture. */
  fallbackFocus?: RefObject<HTMLElement | null>;
}

export function ConfirmSheet({
  message,
  confirmLabel,
  cancelLabel,
  busyLabel,
  busy = false,
  error,
  onConfirm,
  onClose,
  trigger,
  fallbackFocus,
}: ConfirmSheetProps) {
  const messageId = useId();
  const cancel = useRef<HTMLButtonElement>(null);
  const close = () => {
    if (!busy) onClose();
  };
  return (
    <Sheet labelledBy={messageId} onClose={close} trigger={trigger} initialFocus={cancel} fallbackFocus={fallbackFocus}>
      <p id={messageId} className="text-body">
        {message}
      </p>
      {error && (
        <p role="alert" className="mt-3 text-label text-danger">
          {error}
        </p>
      )}
      <div className="mt-5 flex flex-col gap-2">
        <button
          type="button"
          onClick={onConfirm}
          disabled={busy}
          className="min-h-tap rounded-md border border-danger px-4 text-label text-danger disabled:border-border disabled:text-ink-disabled"
        >
          {busy ? busyLabel : confirmLabel}
        </button>
        <button
          ref={cancel}
          type="button"
          onClick={close}
          disabled={busy}
          className="min-h-tap rounded-md border border-border px-4 text-label text-ink-primary disabled:text-ink-disabled"
        >
          {cancelLabel}
        </button>
      </div>
    </Sheet>
  );
}
