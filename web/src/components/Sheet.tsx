// Feuille du bas (EXPERIENCE.md *Component Patterns*) : `role="dialog"`, `aria-modal`, focus piégé
// puis rendu au déclencheur ; fermeture par Échap, par le fond ou par glissement vers le bas. Pas
// d'animation d'entrée ; seul le retour après un glissement court est animé (coupé par
// `prefers-reduced-motion`). Base de « Langue · Thème » et des confirmations destructives.

import { useEffect, useRef, type KeyboardEvent, type PointerEvent, type ReactNode, type RefObject } from "react";

/** Glissement vers le bas, en pixels, au-delà duquel la feuille se ferme. */
export const SWIPE_CLOSE_PX = 80;

const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

export interface SheetProps {
  /** Id de l'élément qui nomme la feuille. */
  labelledBy: string;
  onClose: () => void;
  /** Bouton qui a ouvert la feuille : il récupère le focus à la fermeture. Passé explicitement,
   * car Safari ne donne pas le focus à un bouton cliqué. */
  trigger: RefObject<HTMLElement | null>;
  /** Élément qui prend le focus à l'ouverture. */
  initialFocus: RefObject<HTMLElement | null>;
  /** Repli du focus à la fermeture si le déclencheur a quitté la page. */
  fallbackFocus?: RefObject<HTMLElement | null>;
  children: ReactNode;
}

export function Sheet({ labelledBy, onClose, trigger, initialFocus, fallbackFocus, children }: SheetProps) {
  const sheet = useRef<HTMLDivElement>(null);
  const drag = useRef<{ id: number; startY: number; dy: number } | null>(null);

  useEffect(() => {
    // Le déclencheur récupère le focus à la fermeture, quelle qu'en soit la cause.
    const back = trigger.current;
    initialFocus.current?.focus();
    return () => {
      // Un déclencheur détaché (bouton disparu après l'action) laisserait le focus sur `body`.
      if (back?.isConnected) back.focus();
      else fallbackFocus?.current?.focus();
    };
  }, [trigger, initialFocus, fallbackFocus]);

  // Échap sur tout le document : le focus peut être sur `body` après un toucher sur du texte.
  const closeRef = useRef(onClose);
  closeRef.current = onClose;
  useEffect(() => {
    const onDocKey = (e: globalThis.KeyboardEvent) => {
      if (e.key === "Escape") closeRef.current();
    };
    document.addEventListener("keydown", onDocKey);
    return () => document.removeEventListener("keydown", onDocKey);
  }, []);

  function onKeyDown(e: KeyboardEvent<HTMLDivElement>) {
    if (e.key !== "Tab" || !sheet.current) return;
    const items = [...sheet.current.querySelectorAll<HTMLElement>(FOCUSABLE)];
    if (items.length === 0) return;
    const first = items[0];
    const last = items[items.length - 1];
    const active = document.activeElement;
    if (e.shiftKey && (active === first || !sheet.current.contains(active))) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && (active === last || !sheet.current.contains(active))) {
      e.preventDefault();
      first.focus();
    }
  }

  function setOffset(px: number) {
    if (sheet.current) sheet.current.style.transform = px > 0 ? `translateY(${px}px)` : "";
  }

  function onPointerDown(e: PointerEvent<HTMLDivElement>) {
    drag.current = { id: e.pointerId, startY: e.clientY, dy: 0 };
    e.currentTarget.setPointerCapture?.(e.pointerId);
    sheet.current?.classList.remove("motion-safe:transition-transform");
  }

  function onPointerMove(e: PointerEvent<HTMLDivElement>) {
    const d = drag.current;
    if (!d || d.id !== e.pointerId) return;
    d.dy = Math.max(0, e.clientY - d.startY);
    setOffset(d.dy);
  }

  function onPointerEnd(e: PointerEvent<HTMLDivElement>) {
    const d = drag.current;
    if (!d || d.id !== e.pointerId) return;
    drag.current = null;
    // Un geste annulé par le système (`pointercancel`) ne ferme jamais la feuille.
    if (e.type === "pointerup" && d.dy > SWIPE_CLOSE_PX) {
      onClose();
      return;
    }
    sheet.current?.classList.add("motion-safe:transition-transform");
    setOffset(0);
  }

  return (
    <div className="fixed inset-0 z-50 flex items-end justify-center">
      <div className="absolute inset-0 bg-backdrop" aria-hidden="true" onClick={onClose} />
      <div
        ref={sheet}
        role="dialog"
        aria-modal="true"
        aria-labelledby={labelledBy}
        onKeyDown={onKeyDown}
        className="relative w-full max-w-content rounded-t-lg border border-b-0 border-border bg-surface-raised px-3 pb-6 shadow-[0_-8px_24px_rgb(0_0_0/0.35)] motion-safe:transition-transform min-[360px]:px-4"
      >
        <div
          data-testid="sheet-handle"
          className="flex h-tap cursor-grab touch-none items-center justify-center"
          onPointerDown={onPointerDown}
          onPointerMove={onPointerMove}
          onPointerUp={onPointerEnd}
          onPointerCancel={onPointerEnd}
        >
          <span className="block h-1 w-10 rounded-full bg-ink-disabled" aria-hidden="true" />
        </div>
        {children}
      </div>
    </div>
  );
}
