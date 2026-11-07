// Feuille du bas « Langue · Thème » (EXPERIENCE.md), sur la base commune `Sheet` : dialogue modal,
// focus piégé puis rendu au déclencheur ; fermeture par Échap, bouton Fermer, fond ou glissement.

import { useId, useRef, type ReactNode, type RefObject } from "react";
import { LANGS, setLang, t, useLang, type Lang } from "../i18n";
import { THEMES, setTheme, useTheme, type Theme } from "../theme";
import { Sheet } from "./Sheet";

export { SWIPE_CLOSE_PX } from "./Sheet";

interface Props {
  onClose: () => void;
  /** Bouton qui a ouvert la feuille : il récupère le focus à la fermeture. */
  trigger: RefObject<HTMLElement | null>;
}

export function SettingsSheet({ onClose, trigger }: Props) {
  const lang = useLang();
  const theme = useTheme();
  const titleId = useId();
  const closeButton = useRef<HTMLButtonElement>(null);

  return (
    <Sheet labelledBy={titleId} onClose={onClose} trigger={trigger} initialFocus={closeButton}>
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
    </Sheet>
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
