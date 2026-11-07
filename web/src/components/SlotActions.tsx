// Actions d'une place occupée de l'onglet Composition (EXPERIENCE.md *Interaction Primitives*) :
// « Retirer », « Au banc » (titulaire seulement) et « Échanger », qui ferme la feuille et attend le
// toucher d'une autre place. Jamais de balayage ni de glisser-déposer.

import { useId, useRef, type RefObject } from "react";
import { t } from "../i18n";
import { Sheet } from "./Sheet";

interface Props {
  /** `Pseudo[SRV]` de l'occupant : il nomme la feuille. */
  name: string;
  /** Faux pour un joueur déjà au banc. */
  canBench: boolean;
  onRemove: () => void;
  onBench: () => void;
  onSwap: () => void;
  onClose: () => void;
  trigger: RefObject<HTMLElement | null>;
  fallbackFocus?: RefObject<HTMLElement | null>;
}

export function SlotActions({ name, canBench, onRemove, onBench, onSwap, onClose, trigger, fallbackFocus }: Props) {
  const ids = useId();
  const heading = useRef<HTMLHeadingElement>(null);
  const button = "min-h-tap rounded-md border border-border px-4 text-label text-ink-primary";
  return (
    <Sheet labelledBy={`${ids}-title`} onClose={onClose} trigger={trigger} initialFocus={heading} fallbackFocus={fallbackFocus}>
      <div data-testid="slot-actions">
        <h2
          id={`${ids}-title`}
          ref={heading}
          tabIndex={-1}
          className="truncate font-mono text-[15px] leading-[1.4] font-medium outline-none"
        >
          {name}
        </h2>
        <div className="mt-4 flex flex-col gap-2">
          <button type="button" data-testid="action-swap" onClick={onSwap} className={button}>
            {t("composition.swap")}
          </button>
          {canBench && (
            <button type="button" data-testid="action-bench" onClick={onBench} className={button}>
              {t("composition.toBench")}
            </button>
          )}
          <button type="button" data-testid="action-remove" onClick={onRemove} className={button}>
            {t("composition.remove")}
          </button>
          <button type="button" onClick={onClose} className={button}>
            {t("composition.close")}
          </button>
        </div>
      </div>
    </Sheet>
  );
}
