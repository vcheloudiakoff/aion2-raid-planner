// Feuille du bas « Langue · Thème » (EXPERIENCE.md) : `role="dialog"`, `aria-modal`, focus piégé
// puis rendu au déclencheur ; fermeture par Échap, bouton Fermer, fond ou glissement vers le bas.
// Pas d'animation d'entrée ; seul le retour après un glissement court est animé (coupé par
// `prefers-reduced-motion`).

import {
  useEffect,
  useId,
  useRef,
  type KeyboardEvent,
  type PointerEvent,
  type ReactNode,
  type RefObject,
} from "react";
import { LANGS, setLang, t, useLang, type Lang } from "../i18n";
import { THEMES, setTheme, useTheme, type Theme } from "../theme";

/** Glissement vers le bas, en pixels, au-delà duquel la feuille se ferme. */
export const SWIPE_CLOSE_PX = 80;

const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

interface Props {
  onClose: () => void;
  /** Bouton qui a ouvert la feuille : il récupère le focus à la fermeture. Passé explicitement,
   * car Safari ne donne pas le focus à un bouton cliqué. */
  trigger: RefObject<HTMLElement | null>;
}

export function SettingsSheet({ onClose, trigger }: Props) {
  const lang = useLang();
  const theme = useTheme();
  const titleId = useId();
  const sheet = useRef<HTMLDivElement>(null);
  const closeButton = useRef<HTMLButtonElement>(null);
  const drag = useRef<{ id: number; startY: number; dy: number } | null>(null);

  useEffect(() => {
    // Le déclencheur récupère le focus à la fermeture, quelle qu'en soit la cause.
    const back = trigger.current;
    closeButton.current?.focus();
    return () => back?.focus();
  }, [trigger]);

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
        aria-labelledby={titleId}
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
        <div className="flex items-center justify-between gap-2">
          <h2 id={titleId} className="text-title">
            {t("settings.title")}
          </h2>
          <button
            ref={closeButton}
            type="button"
            onClick={onClose}
            className="min-h-tap rounded-md border border-border px-4 text-label text-ink-primary"
          >
            {t("settings.close")}
          </button>
        </div>
        <Choices<Lang>
          legend={t("settings.language")}
          values={LANGS}
          current={lang}
          label={(l) => t(l === "fr" ? "settings.lang.fr" : "settings.lang.en")}
          lang={(l) => l}
          onPick={setLang}
        />
        <Choices<Theme>
          legend={t("settings.theme")}
          values={THEMES}
          current={theme}
          label={(v) => t(v === "dark" ? "settings.theme.dark" : "settings.theme.light")}
          onPick={setTheme}
        />
      </div>
    </div>
  );
}

interface ChoicesProps<T extends string> {
  legend: string;
  values: readonly T[];
  current: T;
  label: (v: T) => string;
  lang?: (v: T) => string;
  onPick: (v: T) => void;
}

function Choices<T extends string>({ legend, values, current, label, lang, onPick }: ChoicesProps<T>): ReactNode {
  const id = useId();
  return (
    <div role="group" aria-labelledby={id} className="mt-5">
      <p id={id} className="mb-2 text-label text-ink-secondary">
        {legend}
      </p>
      <div className="grid grid-cols-2 gap-2">
        {values.map((v) => {
          const on = v === current;
          return (
            <button
              key={v}
              type="button"
              lang={lang?.(v)}
              aria-pressed={on}
              onClick={() => onPick(v)}
              className={`min-h-tap rounded-md border px-3 text-label ${
                on ? "border-2 border-accent text-ink-primary" : "border-border text-ink-secondary"
              }`}
            >
              {label(v)}
            </button>
          );
        })}
      </div>
    </div>
  );
}
